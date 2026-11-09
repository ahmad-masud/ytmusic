//! A native macOS desktop app for YouTube Music.
//!
//! Architecture: one `tao` window hosting one `wry` (WKWebView) view pointed at
//! music.youtube.com. `src/inject.js` runs inside the page and reports what the
//! player is doing; the Rust side turns that into a window title, a menu bar
//! item, and a system Now Playing entry, and sends commands back the other way.
//!
//! The web page is the UI. Everything Rust adds is the part a browser tab
//! cannot do: media keys, Control Center, a menu bar item, and staying alive
//! with the window closed.

mod action;
mod config;
mod icon;
mod menu;
mod nowplaying;
mod state;
mod tray;

use std::error::Error;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{BackgroundThrottlingPolicy, NewWindowResponse, WebView, WebViewBuilder};

use action::Action;
use config::{Config, HOME_URL};
use menu::AppMenu;
use nowplaying::RemoteCommand;
use state::{IpcMessage, PlayerState};
use tray::Tray;

const INJECTED_SCRIPT: &str = include_str!("inject.js");

/// Window moves and resizes arrive in a flood while dragging; coalesce them.
const GEOMETRY_FLUSH: Duration = Duration::from_secs(3);

/// macOS retires a Now Playing session that stops being refreshed, so the
/// change-driven updates need a floor. Measured: the tile survives at this
/// interval and disappears if we only publish on change.
const NOW_PLAYING_HEARTBEAT: Duration = Duration::from_secs(5);
const ZOOM_STEP: f64 = 1.1;

#[derive(Debug)]
enum UserEvent {
    /// A JSON payload from `inject.js`.
    Ipc(String),
    /// A menu id from either the app menu or the tray menu.
    Menu(String),
    /// A media key or Control Center command.
    Remote(RemoteCommand, f64),
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::load();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = build_window(&config, &event_loop)?;

    // Must happen after the event loop exists: both need a live NSApplication.
    let app_menu = AppMenu::build(&config)?;
    let tray = Tray::build(&config)?;

    let webview = build_webview(&config, &window, proxy.clone())?;

    forward_menu_events(proxy.clone());
    if config.native_now_playing {
        let remote = Mutex::new(proxy.clone());
        nowplaying::init(move |command, value| {
            if let Ok(proxy) = remote.lock() {
                let _ = proxy.send_event(UserEvent::Remote(command, value));
            }
        });
    }

    let mut app = App::new(config);
    app.sync_ui(&app_menu, &tray);

    event_loop.run(move |event, _target, control_flow| {
        let mut exit = false;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if app.config.close_to_tray {
                    hide_window(&window);
                    app.config.save();
                } else {
                    exit = true;
                }
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::Moved(_),
                ..
            } => app.capture_geometry(&window),

            // Clicking the Dock icon while the window is hidden.
            Event::Reopen { .. } => show_window(&window),

            Event::UserEvent(UserEvent::Ipc(payload)) => {
                app.handle_ipc(&payload, &window, &webview, &app_menu, &tray);
            }

            Event::UserEvent(UserEvent::Menu(id)) => match Action::from_menu_id(&id) {
                Some(Action::Quit) => exit = true,
                Some(action) => app.perform(action, &window, &webview, &app_menu, &tray),
                None => {}
            },

            Event::UserEvent(UserEvent::Remote(command, value)) => {
                app.handle_remote(command, value, &webview);
            }

            Event::LoopDestroyed => {
                app.config.save();
                nowplaying::clear();
            }

            _ => {}
        }

        *control_flow = if exit {
            ControlFlow::Exit
        } else {
            app.tick_control_flow()
        };
    });
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn build_window<T: 'static>(
    config: &Config,
    event_loop: &tao::event_loop::EventLoop<T>,
) -> Result<Window, Box<dyn Error>> {
    let mut builder = WindowBuilder::new()
        .with_title(config::APP_NAME)
        .with_inner_size(LogicalSize::new(config.window.width, config.window.height))
        .with_min_inner_size(LogicalSize::new(480.0, 420.0))
        .with_always_on_top(config.always_on_top);

    if let (Some(x), Some(y)) = (config.window.x, config.window.y) {
        builder = builder.with_position(LogicalPosition::new(x, y));
    }

    let window = builder.build(event_loop)?;
    if config.window.maximized {
        window.set_maximized(true);
    }
    Ok(window)
}

fn build_webview(
    config: &Config,
    window: &Window,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<WebView, Box<dyn Error>> {
    // A loopback start URL means we are pointed at a local fixture; keep those
    // navigations in the window instead of bouncing them to the browser.
    let start_url = config.start_url().to_ascii_lowercase();
    let allow_loopback = is_loopback(host_of(&start_url));

    let webview = WebViewBuilder::new()
        .with_url(config.start_url())
        .with_user_agent(&config.user_agent)
        .with_initialization_script_for_main_only(INJECTED_SCRIPT, true)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(UserEvent::Ipc(request.into_body()));
        })
        // Playback has to survive a reload and a hidden window.
        .with_autoplay(true)
        .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
        .with_devtools(config.devtools)
        // Zoom is driven from the View menu so it can be persisted.
        .with_hotkeys_zoom(false)
        .with_back_forward_navigation_gestures(true)
        .with_accept_first_mouse(true)
        .with_clipboard(true)
        .with_navigation_handler(move |url| match classify(&url, allow_loopback) {
            Destination::Internal => true,
            Destination::External => {
                open_externally(&url);
                false
            }
        })
        .with_new_window_req_handler(|url, _features| {
            open_externally(&url);
            NewWindowResponse::Deny
        })
        .build(window)?;

    if (config.zoom - 1.0).abs() > f64::EPSILON {
        let _ = webview.zoom(config.zoom);
    }
    Ok(webview)
}

/// muda and tray-icon deliver events on their own channels, which would never
/// wake the tao loop; bounce them through the proxy instead.
fn forward_menu_events(proxy: EventLoopProxy<UserEvent>) {
    let proxy = Mutex::new(proxy);
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        if let Ok(proxy) = proxy.lock() {
            let _ = proxy.send_event(UserEvent::Menu(event.id.0.clone()));
        }
    }));
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    config: Config,
    state: PlayerState,
    /// Set while we own the system Now Playing entry.
    now_playing_active: bool,
    /// When we last pushed to MPNowPlayingInfoCenter.
    last_published: Option<Instant>,
    /// When the debounced geometry write is due.
    pending_save: Option<Instant>,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            config,
            state: PlayerState::default(),
            now_playing_active: false,
            last_published: None,
            pending_save: None,
        }
    }

    fn handle_ipc(
        &mut self,
        payload: &str,
        window: &Window,
        webview: &WebView,
        app_menu: &AppMenu,
        tray: &Tray,
    ) {
        if self.config.debug {
            eprintln!("ytmusic[ipc] <- {payload}");
        }
        match serde_json::from_str::<IpcMessage>(payload) {
            Ok(IpcMessage::State(state)) => self.on_state(state, window, app_menu, tray),
            Ok(IpcMessage::Ready) => {
                // A fresh document resets the zoom factor.
                let _ = webview.zoom(self.config.zoom);
            }
            Ok(IpcMessage::Log(message)) => eprintln!("ytmusic[page]: {message}"),
            Ok(IpcMessage::CopyLink(url)) => nowplaying::copy_to_clipboard(&url),
            Err(err) => eprintln!("ytmusic: unreadable IPC payload ({err}): {payload}"),
        }
    }

    fn on_state(&mut self, next: PlayerState, window: &Window, app_menu: &AppMenu, tray: &Tray) {
        // MPNowPlayingInfoCenter interpolates the Control Center scrubber from
        // the elapsed time and the playback rate, so a per-second republish is
        // mostly wasted work. It cannot be dropped entirely, though: a session
        // that stops refreshing is retired by the system and the tile vanishes.
        // So: publish on anything the system cannot infer, plus a heartbeat.
        let track_changed = next.track_changed(&self.state);
        let transport_changed = next.playing != self.state.playing;
        let seeked = (next.elapsed - self.state.elapsed).abs() > 2.0;
        let heartbeat_due = self
            .last_published
            .is_none_or(|at| at.elapsed() >= NOW_PLAYING_HEARTBEAT);
        let republish = track_changed
            || transport_changed
            || seeked
            || heartbeat_due
            || !self.now_playing_active;

        if next.track_changed(&self.state) {
            window.set_title(&next.window_title());
            if self.config.debug {
                eprintln!(
                    "ytmusic[track] {} | artwork={}",
                    next.window_title(),
                    if next.artwork.is_empty() { "none" } else { &next.artwork }
                );
            }
        }
        self.state = next;

        if self.config.native_now_playing {
            if self.state.has_track() && republish {
                nowplaying::update(
                    &self.state.title,
                    &self.state.artist,
                    &self.state.album,
                    &self.state.artwork,
                    self.state.duration,
                    self.state.elapsed,
                    self.state.playing,
                );
                self.now_playing_active = true;
                self.last_published = Some(Instant::now());
            } else if self.now_playing_active && !self.state.has_track() {
                nowplaying::clear();
                self.now_playing_active = false;
            }
        }

        self.sync_ui(app_menu, tray);
    }

    fn sync_ui(&self, app_menu: &AppMenu, tray: &Tray) {
        app_menu.sync(&self.state);
        tray.sync(&self.state);
    }

    fn perform(
        &mut self,
        action: Action,
        window: &Window,
        webview: &WebView,
        app_menu: &AppMenu,
        tray: &Tray,
    ) {
        if let Some((command, argument)) = action.page_command() {
            call_page(webview, command, argument);
            return;
        }

        match action {
            Action::Reload => drop(webview.reload()),
            Action::HardReload => drop(webview.evaluate_script("location.reload()")),
            Action::Home => drop(webview.load_url(HOME_URL)),
            Action::Back => drop(webview.go_back()),
            Action::Forward => drop(webview.go_forward()),

            Action::ShowWindow => show_window(window),
            Action::HideWindow => {
                hide_window(window);
                self.config.save();
            }

            Action::ToggleAlwaysOnTop => {
                let enabled = !self.config.always_on_top;
                self.config.always_on_top = enabled;
                window.set_always_on_top(enabled);
                app_menu.always_on_top.set_checked(enabled);
                tray.always_on_top.set_checked(enabled);
                self.config.save();
            }

            Action::ZoomIn => self.set_zoom(self.config.zoom * ZOOM_STEP, webview),
            Action::ZoomOut => self.set_zoom(self.config.zoom / ZOOM_STEP, webview),
            Action::ZoomReset => self.set_zoom(1.0, webview),

            Action::DevTools => {
                if webview.is_devtools_open() {
                    webview.close_devtools();
                } else {
                    webview.open_devtools();
                }
            }

            Action::CopyLink => {
                let url = if self.state.url.is_empty() {
                    HOME_URL
                } else {
                    &self.state.url
                };
                nowplaying::copy_to_clipboard(url);
            }
            Action::OpenInBrowser => {
                open_externally(if self.state.url.is_empty() {
                    HOME_URL
                } else {
                    &self.state.url
                });
            }
            Action::RevealConfig => {
                let dir = Config::dir();
                let _ = std::fs::create_dir_all(&dir);
                if let Some(path) = dir.to_str() {
                    reveal_in_finder(path);
                }
            }

            // Handled by the caller, or mapped to a page command above.
            Action::Quit => {}
            _ => {}
        }
    }

    fn handle_remote(&mut self, command: RemoteCommand, value: f64, webview: &WebView) {
        if self.config.debug {
            eprintln!("ytmusic[remote] {command:?} value={value}");
        }
        let interval = if value.abs() < 1.0 { 10.0 } else { value.abs() };
        let (name, argument) = match command {
            RemoteCommand::Play => ("play", None),
            RemoteCommand::Pause => ("pause", None),
            RemoteCommand::Toggle => ("playPause", None),
            RemoteCommand::Next => ("next", None),
            RemoteCommand::Previous => ("previous", None),
            RemoteCommand::Stop => ("stop", None),
            RemoteCommand::Seek => ("seekTo", Some(value)),
            RemoteCommand::SkipForward => ("seekBy", Some(interval)),
            RemoteCommand::SkipBackward => ("seekBy", Some(-interval)),
        };
        call_page(webview, name, argument);
    }

    fn set_zoom(&mut self, zoom: f64, webview: &WebView) {
        self.config.zoom = zoom.clamp(0.4, 3.0);
        if let Err(err) = webview.zoom(self.config.zoom) {
            eprintln!("ytmusic: could not set zoom: {err}");
        }
        self.queue_save();
    }

    fn capture_geometry(&mut self, window: &Window) {
        let scale = window.scale_factor();
        let size = window.inner_size().to_logical::<f64>(scale);
        // Hidden or minimised windows report degenerate sizes; don't persist those.
        if size.width < 200.0 || size.height < 200.0 {
            return;
        }
        self.config.window.width = size.width;
        self.config.window.height = size.height;
        self.config.window.maximized = window.is_maximized();
        if let Ok(position) = window.outer_position() {
            let position = position.to_logical::<f64>(scale);
            self.config.window.x = Some(position.x);
            self.config.window.y = Some(position.y);
        }
        self.queue_save();
    }

    fn queue_save(&mut self) {
        self.pending_save = Some(Instant::now() + GEOMETRY_FLUSH);
    }

    /// Flush a pending config write if it is due, and report how long the loop
    /// may sleep.
    fn tick_control_flow(&mut self) -> ControlFlow {
        match self.pending_save {
            Some(due) if Instant::now() >= due => {
                self.config.save();
                self.pending_save = None;
                ControlFlow::Wait
            }
            Some(due) => ControlFlow::WaitUntil(due),
            None => ControlFlow::Wait,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn call_page(webview: &WebView, command: &str, argument: Option<f64>) {
    let name = serde_json::to_string(command).unwrap_or_else(|_| "\"\"".into());
    let script = match argument {
        Some(value) => format!("window.__ytmNative&&window.__ytmNative.cmd({name},{value})"),
        None => format!("window.__ytmNative&&window.__ytmNative.cmd({name})"),
    };
    if let Err(err) = webview.evaluate_script(&script) {
        eprintln!("ytmusic: could not run `{command}`: {err}");
    }
}

fn show_window(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
    nowplaying::activate();
}

fn hide_window(window: &Window) {
    window.set_visible(false);
}

enum Destination {
    Internal,
    External,
}

/// Hosts that belong to the app. Deliberately broad: YouTube's sign-in and
/// playback paths bounce through a lot of Google properties, and wrongly
/// cancelling one of those breaks login or audio, while wrongly keeping a page
/// in the window is merely untidy.
const INTERNAL_SUFFIXES: &[&str] = &[
    "youtube.com",
    "youtu.be",
    "ytimg.com",
    "ggpht.com",
    "googlevideo.com",
    "gstatic.com",
    "googleusercontent.com",
    "googleapis.com",
    "googlesyndication.com",
    "googleadservices.com",
    "doubleclick.net",
    "google.com",
];

fn classify(url: &str, allow_loopback: bool) -> Destination {
    let url = url.to_ascii_lowercase();

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        // about:, blob:, data: and friends are the page's own business.
        return if url.starts_with("mailto:") || url.starts_with("tel:") {
            Destination::External
        } else {
            Destination::Internal
        };
    }

    let host = host_of(&url);
    if allow_loopback && is_loopback(host) {
        return Destination::Internal;
    }
    // `google.<cctld>` covers google.ca, google.co.uk and the rest.
    let is_google = host == "google" || host.starts_with("google.") || host.contains(".google.");
    let is_known = INTERNAL_SUFFIXES
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")));

    if is_google || is_known {
        Destination::Internal
    } else {
        Destination::External
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]") || host.ends_with(".localhost")
}

fn host_of(url: &str) -> &str {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = after_scheme.split('/').next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    host.split(':').next().unwrap_or(host)
}

fn open_externally(url: &str) {
    // Only hand `open` schemes we meant to support, so a hostile page cannot
    // talk us into launching an arbitrary URL handler.
    let allowed = ["http://", "https://", "mailto:", "tel:"]
        .iter()
        .any(|scheme| url.to_ascii_lowercase().starts_with(scheme));
    if !allowed {
        eprintln!("ytmusic: refusing to open {url}");
        return;
    }
    if let Err(err) = Command::new("/usr/bin/open").args(["--", url]).spawn() {
        eprintln!("ytmusic: could not open {url}: {err}");
    }
}

fn reveal_in_finder(path: &str) {
    if let Err(err) = Command::new("/usr/bin/open").args(["--", path]).spawn() {
        eprintln!("ytmusic: could not reveal {path}: {err}");
    }
}
