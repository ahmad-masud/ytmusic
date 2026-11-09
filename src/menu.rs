//! The application menu.
//!
//! macOS gets a real menu bar owned by the application; Windows gets a window
//! menu bar attached to the HWND. The playback, view and edit submenus are
//! shared — only the platform shell around them differs, along with the
//! modifier key (`CMD_OR_CTRL` is Command on macOS and Control elsewhere).

use std::cell::RefCell;

use muda::accelerator::{Accelerator, CMD_OR_CTRL, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tao::window::Window;

use crate::action::Action;
use crate::config::Config;
use crate::state::PlayerState;

fn key(mods: Modifiers, code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(mods, code))
}

fn item(action: Action, label: &str, accelerator: Option<Accelerator>) -> MenuItem {
    MenuItem::with_id(action.id(), label, true, accelerator)
}

fn about_metadata() -> AboutMetadata {
    AboutMetadata {
        name: Some("YouTube Music".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        comments: Some(
            "An unofficial desktop wrapper for music.youtube.com.\n\
             Not affiliated with Google or YouTube."
                .into(),
        ),
        ..Default::default()
    }
}

/// The parts of the player state that change what a menu item says. Snapshots
/// of this are compared so a 1 Hz state update does not turn into a dozen
/// platform calls per second for labels that did not change.
#[derive(PartialEq, Eq, Clone, Copy)]
struct Rendered {
    playing: bool,
    has_player: bool,
    has_track: bool,
    liked: i8,
}

/// Handles for the items whose labels or check state we update at runtime.
pub struct AppMenu {
    /// Kept alive for as long as the app runs; dropping it tears down the menu.
    _root: Menu,
    play_pause: MenuItem,
    like: MenuItem,
    dislike: MenuItem,
    pub always_on_top: CheckMenuItem,
    last: RefCell<Option<Rendered>>,
}

impl AppMenu {
    pub fn build(config: &Config, window: &Window) -> muda::Result<Self> {
        let _ = window; // only the Windows build needs the HWND
        let cmd = CMD_OR_CTRL;
        let cmd_shift = cmd | Modifiers::SHIFT;
        let cmd_alt = cmd | Modifiers::ALT;

        // --- Edit ------------------------------------------------------------
        let edit_menu = Submenu::with_items(
            "Edit",
            true,
            &[
                &PredefinedMenuItem::undo(None),
                &PredefinedMenuItem::redo(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::cut(None),
                &PredefinedMenuItem::copy(None),
                &PredefinedMenuItem::paste(None),
                &PredefinedMenuItem::select_all(None),
            ],
        )?;

        // --- Playback --------------------------------------------------------
        let play_pause = item(Action::PlayPause, "Play", key(cmd, Code::KeyP));
        let like = item(Action::Like, "Like", key(cmd_shift, Code::KeyL));
        let dislike = item(Action::Dislike, "Dislike", key(cmd_shift, Code::KeyD));
        let playback_menu = Submenu::with_items(
            "Playback",
            true,
            &[
                &play_pause,
                &PredefinedMenuItem::separator(),
                &item(Action::Next, "Next Track", key(cmd_shift, Code::ArrowRight)),
                &item(
                    Action::Previous,
                    "Previous Track",
                    key(cmd_shift, Code::ArrowLeft),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Action::SeekForward,
                    "Skip Forward 10s",
                    key(cmd_shift, Code::Period),
                ),
                &item(
                    Action::SeekBackward,
                    "Skip Back 10s",
                    key(cmd_shift, Code::Comma),
                ),
                &PredefinedMenuItem::separator(),
                &item(Action::VolumeUp, "Volume Up", key(cmd_shift, Code::ArrowUp)),
                &item(
                    Action::VolumeDown,
                    "Volume Down",
                    key(cmd_shift, Code::ArrowDown),
                ),
                &item(Action::ToggleMute, "Mute", key(cmd_shift, Code::KeyM)),
                &PredefinedMenuItem::separator(),
                &like,
                &dislike,
                &item(Action::Shuffle, "Shuffle", key(cmd_shift, Code::KeyS)),
                &item(Action::Repeat, "Cycle Repeat Mode", None),
                &PredefinedMenuItem::separator(),
                &item(Action::CopyLink, "Copy Song Link", key(cmd_shift, Code::KeyC)),
                &item(Action::OpenInBrowser, "Open Song in Browser", None),
            ],
        )?;

        // --- View ------------------------------------------------------------
        let always_on_top = CheckMenuItem::with_id(
            Action::ToggleAlwaysOnTop.id(),
            "Always on Top",
            true,
            config.always_on_top,
            None,
        );
        let reload = item(Action::Reload, "Reload", key(cmd, Code::KeyR));
        let hard_reload = item(
            Action::HardReload,
            "Reload Ignoring Cache",
            key(cmd_shift, Code::KeyR),
        );
        let back = item(Action::Back, "Back", key(cmd, Code::BracketLeft));
        let forward = item(Action::Forward, "Forward", key(cmd, Code::BracketRight));
        let home = item(Action::Home, "Home", key(cmd_shift, Code::KeyH));
        let search = item(Action::Search, "Search…", key(cmd, Code::KeyF));
        let zoom_in = item(Action::ZoomIn, "Zoom In", key(cmd, Code::Equal));
        let zoom_out = item(Action::ZoomOut, "Zoom Out", key(cmd, Code::Minus));
        let zoom_reset = item(Action::ZoomReset, "Actual Size", key(cmd, Code::Digit0));
        // macOS-native menu action; Windows has no predefined equivalent.
        #[cfg(target_os = "macos")]
        let fullscreen = PredefinedMenuItem::fullscreen(None);
        let devtools = item(
            Action::DevTools,
            "Toggle Developer Tools",
            key(cmd_alt, Code::KeyI),
        );
        let separators: Vec<PredefinedMenuItem> =
            (0..4).map(|_| PredefinedMenuItem::separator()).collect();

        let mut view_items: Vec<&dyn muda::IsMenuItem> = vec![
            &reload,
            &hard_reload,
            &separators[0],
            &back,
            &forward,
            &home,
            &search,
            &separators[1],
            &zoom_in,
            &zoom_out,
            &zoom_reset,
            &separators[2],
        ];
        #[cfg(target_os = "macos")]
        view_items.push(&fullscreen);
        view_items.push(&always_on_top);
        if config.devtools {
            view_items.push(&separators[3]);
            view_items.push(&devtools);
        }
        let view_menu = Submenu::with_items("View", true, &view_items)?;

        let root = Menu::new();

        #[cfg(target_os = "macos")]
        {
            let app_menu = Submenu::with_items(
                "YouTube Music",
                true,
                &[
                    &PredefinedMenuItem::about(
                        Some("About YouTube Music"),
                        Some(about_metadata()),
                    ),
                    &PredefinedMenuItem::separator(),
                    &item(Action::RevealConfig, "Open Settings Folder…", None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::services(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(None),
                    &PredefinedMenuItem::hide_others(None),
                    &PredefinedMenuItem::show_all(None),
                    &PredefinedMenuItem::separator(),
                    &item(Action::Quit, "Quit YouTube Music", key(cmd, Code::KeyQ)),
                ],
            )?;
            let window_menu = Submenu::with_items(
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(None),
                    &PredefinedMenuItem::zoom(None),
                    // Deliberately not `close_window`: closing the window should
                    // hide it and keep the music going.
                    &item(Action::HideWindow, "Close Window", key(cmd, Code::KeyW)),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::bring_all_to_front(None),
                ],
            )?;
            root.append_items(&[
                &app_menu,
                &edit_menu,
                &playback_menu,
                &view_menu,
                &window_menu,
            ])?;
            root.init_for_nsapp();
        }

        #[cfg(target_os = "windows")]
        {
            let file_menu = Submenu::with_items(
                "File",
                true,
                &[
                    &item(Action::HideWindow, "Close Window", key(cmd, Code::KeyW)),
                    &item(Action::RevealConfig, "Open Settings Folder…", None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::minimize(None),
                    &PredefinedMenuItem::separator(),
                    &item(Action::Quit, "Exit", key(cmd, Code::KeyQ)),
                ],
            )?;
            let help_menu = Submenu::with_items(
                "Help",
                true,
                &[&PredefinedMenuItem::about(
                    Some("About YouTube Music"),
                    Some(about_metadata()),
                )],
            )?;
            root.append_items(&[
                &file_menu,
                &edit_menu,
                &playback_menu,
                &view_menu,
                &help_menu,
            ])?;

            use tao::platform::windows::WindowExtWindows;
            // Safe: the HWND comes straight from the window we own and is alive
            // for as long as this menu is.
            unsafe { root.init_for_hwnd(window.hwnd())? };
        }

        Ok(Self {
            _root: root,
            play_pause,
            like,
            dislike,
            always_on_top,
            last: RefCell::new(None),
        })
    }

    pub fn sync(&self, state: &PlayerState) {
        let next = Rendered {
            playing: state.playing,
            has_player: state.has_player,
            has_track: state.has_track(),
            liked: state.liked,
        };
        if *self.last.borrow() == Some(next) {
            return;
        }
        *self.last.borrow_mut() = Some(next);

        self.play_pause
            .set_text(if next.playing { "Pause" } else { "Play" });
        self.play_pause.set_enabled(next.has_player);
        self.like
            .set_text(if next.liked == 1 { "Remove Like" } else { "Like" });
        self.dislike.set_text(if next.liked == -1 {
            "Remove Dislike"
        } else {
            "Dislike"
        });
        self.like.set_enabled(next.has_track);
        self.dislike.set_enabled(next.has_track);
    }
}
