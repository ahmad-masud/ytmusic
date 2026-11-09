//! The macOS application menu.

use std::cell::RefCell;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::action::Action;
use crate::config::Config;

const CMD: Modifiers = Modifiers::META;
const CMD_SHIFT: Modifiers = Modifiers::META.union(Modifiers::SHIFT);
const CMD_ALT: Modifiers = Modifiers::META.union(Modifiers::ALT);

fn key(mods: Modifiers, code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(mods, code))
}

fn item(action: Action, label: &str, accelerator: Option<Accelerator>) -> MenuItem {
    MenuItem::with_id(action.id(), label, true, accelerator)
}

/// The parts of the player state that change what a menu item says. Snapshots
/// of this are compared so a 1 Hz state update does not turn into a dozen
/// Objective-C calls per second for labels that did not change.
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
    pub play_pause: MenuItem,
    pub like: MenuItem,
    pub dislike: MenuItem,
    pub always_on_top: CheckMenuItem,
    last: RefCell<Option<Rendered>>,
}

impl AppMenu {
    pub fn build(config: &Config) -> muda::Result<Self> {
        let root = Menu::new();

        // --- Application -----------------------------------------------------
        let about = PredefinedMenuItem::about(
            Some("About YouTube Music"),
            Some(AboutMetadata {
                name: Some("YouTube Music".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some(
                    "An unofficial desktop wrapper for music.youtube.com.\n\
                     Not affiliated with Google or YouTube."
                        .into(),
                ),
                ..Default::default()
            }),
        );
        let app_menu = Submenu::with_items(
            "YouTube Music",
            true,
            &[
                &about,
                &PredefinedMenuItem::separator(),
                &item(Action::RevealConfig, "Open Settings Folder…", None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &item(Action::Quit, "Quit YouTube Music", key(CMD, Code::KeyQ)),
            ],
        )?;

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
        let play_pause = item(Action::PlayPause, "Play", key(CMD, Code::KeyP));
        let like = item(Action::Like, "Like", key(CMD_SHIFT, Code::KeyL));
        let dislike = item(Action::Dislike, "Dislike", key(CMD_SHIFT, Code::KeyD));
        let playback_menu = Submenu::with_items(
            "Playback",
            true,
            &[
                &play_pause,
                &PredefinedMenuItem::separator(),
                &item(Action::Next, "Next Track", key(CMD_SHIFT, Code::ArrowRight)),
                &item(
                    Action::Previous,
                    "Previous Track",
                    key(CMD_SHIFT, Code::ArrowLeft),
                ),
                &PredefinedMenuItem::separator(),
                &item(
                    Action::SeekForward,
                    "Skip Forward 10s",
                    key(CMD_SHIFT, Code::Period),
                ),
                &item(
                    Action::SeekBackward,
                    "Skip Back 10s",
                    key(CMD_SHIFT, Code::Comma),
                ),
                &PredefinedMenuItem::separator(),
                &item(Action::VolumeUp, "Volume Up", key(CMD_SHIFT, Code::ArrowUp)),
                &item(
                    Action::VolumeDown,
                    "Volume Down",
                    key(CMD_SHIFT, Code::ArrowDown),
                ),
                &item(Action::ToggleMute, "Mute", key(CMD_SHIFT, Code::KeyM)),
                &PredefinedMenuItem::separator(),
                &like,
                &dislike,
                &item(Action::Shuffle, "Shuffle", key(CMD_SHIFT, Code::KeyS)),
                &item(Action::Repeat, "Cycle Repeat Mode", None),
                &PredefinedMenuItem::separator(),
                &item(Action::CopyLink, "Copy Song Link", key(CMD_SHIFT, Code::KeyC)),
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
        let mut view_items: Vec<&dyn muda::IsMenuItem> = Vec::new();
        let reload = item(Action::Reload, "Reload", key(CMD, Code::KeyR));
        let hard_reload = item(
            Action::HardReload,
            "Reload Ignoring Cache",
            key(CMD_SHIFT, Code::KeyR),
        );
        let back = item(Action::Back, "Back", key(CMD, Code::BracketLeft));
        let forward = item(Action::Forward, "Forward", key(CMD, Code::BracketRight));
        let home = item(Action::Home, "Home", key(CMD_SHIFT, Code::KeyH));
        let search = item(Action::Search, "Search…", key(CMD, Code::KeyF));
        let zoom_in = item(Action::ZoomIn, "Zoom In", key(CMD, Code::Equal));
        let zoom_out = item(Action::ZoomOut, "Zoom Out", key(CMD, Code::Minus));
        let zoom_reset = item(Action::ZoomReset, "Actual Size", key(CMD, Code::Digit0));
        let fullscreen = PredefinedMenuItem::fullscreen(None);
        let devtools = item(
            Action::DevTools,
            "Toggle Developer Tools",
            key(CMD_ALT, Code::KeyI),
        );
        let separators: Vec<PredefinedMenuItem> =
            (0..4).map(|_| PredefinedMenuItem::separator()).collect();

        view_items.push(&reload);
        view_items.push(&hard_reload);
        view_items.push(&separators[0]);
        view_items.push(&back);
        view_items.push(&forward);
        view_items.push(&home);
        view_items.push(&search);
        view_items.push(&separators[1]);
        view_items.push(&zoom_in);
        view_items.push(&zoom_out);
        view_items.push(&zoom_reset);
        view_items.push(&separators[2]);
        view_items.push(&fullscreen);
        view_items.push(&always_on_top);
        if config.devtools {
            view_items.push(&separators[3]);
            view_items.push(&devtools);
        }
        let view_menu = Submenu::with_items("View", true, &view_items)?;

        // --- Window ----------------------------------------------------------
        let window_menu = Submenu::with_items(
            "Window",
            true,
            &[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::zoom(None),
                // Deliberately *not* `close_window`: closing the window should
                // hide it and keep the music going.
                &item(Action::HideWindow, "Close Window", key(CMD, Code::KeyW)),
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

        Ok(Self {
            _root: root,
            play_pause,
            like,
            dislike,
            always_on_top,
            last: RefCell::new(None),
        })
    }

    pub fn sync(&self, state: &crate::state::PlayerState) {
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
            .set_text(if state.playing { "Pause" } else { "Play" });
        self.play_pause.set_enabled(state.has_player);
        self.like
            .set_text(if state.liked == 1 { "Remove Like" } else { "Like" });
        self.dislike.set_text(if state.liked == -1 {
            "Remove Dislike"
        } else {
            "Dislike"
        });
        self.like.set_enabled(state.has_track());
        self.dislike.set_enabled(state.has_track());
    }
}
