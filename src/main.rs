//! A native desktop app for YouTube Music, on macOS and Windows.
//!
//! Architecture: one `tao` window hosting one `wry` webview — WKWebView on
//! macOS, WebView2 on Windows — pointed at music.youtube.com.
//! `src/inject.js` runs inside the page and reports what the player is doing;
//! the Rust side turns that into a window title, a tray item, and a system
//! media session, and sends commands back the other way.
//!
//! The web page is the UI. Everything Rust adds is the part a browser tab
//! cannot do: media keys, the OS now-playing surface, a tray item, and staying
//! alive with the window closed.
//!
//! Platform-specific code is confined to `media/`, `shell.rs`, and the `cfg`
//! branches in `menu.rs`, `tray.rs` and `icon.rs`.

// Don't pop a console window alongside the GUI on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod action;
mod config;
mod icon;
mod media;
mod menu;
mod shell;
mod state;
mod tray;

use std::error::Error;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{BackgroundThrottlingPolicy, NewWindowResponse, WebView, WebViewBuilder};

use action::Action;
use config::{Config, HOME_URL};
use media::{NowPlaying, RemoteCommand};
use menu::AppMenu;
use state::{IpcMessage, PlayerState};
use tray::Tray;

const INJECTED_SCRIPT: &str = include_str!("inject.js");

/// Window moves and resizes arrive in a flood while dragging; coalesce them.
const GEOMETRY_FLUSH: Duration = Duration::from_secs(3);

/// The OS retires a media session that stops being refreshed, so the
/// change-driven updates need a floor. Measured on macOS: the tile survives at
/// this interval and disappears if we only publish on change.
const MEDIA_HEARTBEAT: Duration = Duration::from_secs(5);

const ZOOM_STEP: f64 = 1.1;

#[derive(Debug)]
enum UserEvent {
    /// A JSON payload from `inject.js`.
    Ipc(String),
    /// A menu id from either the app menu or the tray menu.
    Menu(String),
    /// A media key or system transport command.
    Remote(RemoteCommand, f64),
    /// The tray icon was clicked (Windows; macOS opens the menu instead).
    TrayClick,
}

fn main() -> Result<(), Box<dyn Error>> {
    attach_console();
    let config = Config::load();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = build_window(&config, &event_loop)?;

    // After the window: the Windows menu bar attaches to its HWND, and both
    // platforms need a live event loop first.
    let app_menu = AppMenu::build(&config, &window)?;
    let tray = Tray::build(&config)?;

    let webview = build_webview(&config, &window, proxy.clone())?;

    forward_menu_events(proxy.clone());
    forward_tray_events(proxy.clone());

    if config.native_now_playing {
        // After the webview, so COM is already initialised on this thread by
        // WebView2 before we ask WinRT for the transport controls.
        let remote = Mutex::new(proxy.clone());
        media::init(&window, move |command, value| {
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
                    window.set_visible(false);
                    app.config.save();
                } else {
                    exit = true;
                }
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::Moved(_),
                ..
            } => app.capture_geometry(&window),

            // Clicking the Dock icon on macOS while the window is hidden.
            Event::Reopen { .. } => shell::bring_to_front(&window),

            Event::UserEvent(UserEvent::TrayClick) => shell::bring_to_front(&window),

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
                media::clear();
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

/// The release build is a GUI subsystem binary, so it has no console and
/// `eprintln!` goes nowhere. When run from a terminal with YTMUSIC_DEBUG set,
/// borrow the parent's console so the diagnostics are actually visible.
#[cfg(target_os = "windows")]
fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    if std::env::var("YTMUSIC_DEBUG").is_ok() {
        let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_console() {}

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

    // macOS takes the icon from the bundle's .icns; Windows needs it set here
    // for the title bar, taskbar and Alt-Tab.
    #[cfg(target_os = "windows")]
    {
        builder = builder.with_window_icon(icon::window_icon());
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

    let mut builder = WebViewBuilder::new()
        .with_url(config.start_url())
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
                shell::open_externally(&url);
                false
            }
        })
        .with_new_window_req_handler(|url, _features| {
            shell::open_externally(&url);
            NewWindowResponse::Deny
        });

    // Empty means "use the engine's own user agent", which is what we want on
    // Windows: WebView2 already identifies as Edge, which YouTube serves well.
    if !config.user_agent.trim().is_empty() {
        builder = builder.with_user_agent(&config.user_agent);
    }

    let webview = builder.build(window)?;

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

fn forward_tray_events(proxy: EventLoopProxy<UserEvent>) {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    let proxy = Mutex::new(proxy);
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        // On macOS the left click opens the menu, so only Windows reaches here
        // with something to do.
        let restore = matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        );
        if restore
            && let Ok(proxy) = proxy.lock()
        {
            let _ = proxy.send_event(UserEvent::TrayClick);
        }
    }));
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    config: Config,
    state: PlayerState,
    /// Set while we own the system media session.
    media_active: bool,
    /// When we last published to the OS media session.
    last_published: Option<Instant>,
    /// When the debounced geometry write is due.
    pending_save: Option<Instant>,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            config,
            state: PlayerState::default(),
            media_active: false,
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
            Ok(IpcMessage::CopyLink(url)) => shell::copy_to_clipboard(&url),
            Err(err) => eprintln!("ytmusic: unreadable IPC payload ({err}): {payload}"),
        }
    }

    fn on_state(&mut self, next: PlayerState, window: &Window, app_menu: &AppMenu, tray: &Tray) {
        // Both platforms interpolate the scrubber from elapsed + playback rate,
        // so a per-second republish is mostly wasted work. It cannot be dropped
        // entirely though: a session that stops refreshing is retired by the OS
        // and the tile vanishes. So: publish on anything it cannot infer, plus
        // a heartbeat.
        let track_changed = next.track_changed(&self.state);
        let transport_changed = next.playing != self.state.playing;
        let seeked = (next.elapsed - self.state.elapsed).abs() > 2.0;
        let heartbeat_due = self
            .last_published
            .is_none_or(|at| at.elapsed() >= MEDIA_HEARTBEAT);
        let republish =
            track_changed || transport_changed || seeked || heartbeat_due || !self.media_active;

        if track_changed {
            window.set_title(&next.window_title());
            if self.config.debug {
                eprintln!(
                    "ytmusic[track] {} | artwork={}",
                    next.window_title(),
                    if next.artwork.is_empty() {
                        "none"
                    } else {
                        &next.artwork
                    }
                );
            }
        }
        self.state = next;

        if self.config.native_now_playing {
            if self.state.has_track() {
                if republish {
                    media::update(&NowPlaying {
                        title: &self.state.title,
                        artist: &self.state.artist,
                        album: &self.state.album,
                        artwork_url: &self.state.artwork,
                        duration: self.state.duration,
                        elapsed: self.state.elapsed,
                        playing: self.state.playing,
                    });
                    self.media_active = true;
                    self.last_published = Some(Instant::now());
                }
            } else if self.media_active {
                media::clear();
                self.media_active = false;
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

            Action::ShowWindow => shell::bring_to_front(window),
            Action::HideWindow => {
                window.set_visible(false);
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

            Action::CopyLink => shell::copy_to_clipboard(self.current_url()),
            Action::OpenInBrowser => shell::open_externally(self.current_url()),
            Action::RevealConfig => {
                let dir = Config::dir();
                let _ = std::fs::create_dir_all(&dir);
                shell::reveal_path(&dir);
            }

            // Handled by the caller, or mapped to a page command above.
            Action::Quit => {}
            _ => {}
        }
    }

    fn current_url(&self) -> &str {
        if self.state.url.is_empty() {
            HOME_URL
        } else {
            &self.state.url
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
