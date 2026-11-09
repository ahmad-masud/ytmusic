//! The menu bar item: a template-rendered glyph, the current track, and the
//! playback controls you want without raising the window.

use std::cell::RefCell;

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::action::Action;
use crate::config::Config;
use crate::icon;
use crate::state::PlayerState;

/// How much of the track name to show next to the menu bar icon.
const TITLE_CHARS: usize = 28;

fn item(action: Action, label: &str) -> MenuItem {
    MenuItem::with_id(action.tray_id(), label, true, None)
}

/// See `menu::Rendered`: everything except the progress line, which is the one
/// thing that genuinely changes every second.
#[derive(PartialEq, Eq, Clone)]
struct Rendered {
    title: String,
    playing: bool,
    has_player: bool,
    has_track: bool,
    liked: i8,
}

pub struct Tray {
    icon: TrayIcon,
    now_playing: MenuItem,
    progress: MenuItem,
    play_pause: MenuItem,
    like: MenuItem,
    dislike: MenuItem,
    pub always_on_top: CheckMenuItem,
    show_title: bool,
    last: RefCell<Option<Rendered>>,
}

impl Tray {
    pub fn build(config: &Config) -> Result<Self, Box<dyn std::error::Error>> {
        let now_playing = MenuItem::with_id("tray:now-playing", "Not playing", false, None);
        let progress = MenuItem::with_id("tray:progress", "—", false, None);
        let play_pause = item(Action::PlayPause, "Play");
        let next = item(Action::Next, "Next Track");
        let previous = item(Action::Previous, "Previous Track");
        let like = item(Action::Like, "Like");
        let dislike = item(Action::Dislike, "Dislike");
        let copy_link = item(Action::CopyLink, "Copy Song Link");
        let show = item(Action::ShowWindow, "Open YouTube Music");
        let always_on_top = CheckMenuItem::with_id(
            Action::ToggleAlwaysOnTop.tray_id(),
            "Always on Top",
            true,
            config.always_on_top,
            None,
        );
        let quit = item(Action::Quit, "Quit YouTube Music");

        let menu = Menu::new();
        menu.append_items(&[
            &now_playing,
            &progress,
            &PredefinedMenuItem::separator(),
            &play_pause,
            &next,
            &previous,
            &PredefinedMenuItem::separator(),
            &like,
            &dislike,
            &copy_link,
            &PredefinedMenuItem::separator(),
            &show,
            &always_on_top,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;

        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("YouTube Music");

        // macOS wants a template image and opens the menu on left click.
        // Windows wants a full-colour icon, and left click should restore the
        // window while right click opens the menu.
        #[cfg(target_os = "macos")]
        let builder = builder
            .with_icon_templated(icon::tray_icon()?)
            .with_menu_on_left_click(true);
        #[cfg(not(target_os = "macos"))]
        let builder = builder
            .with_icon(icon::tray_icon()?)
            .with_menu_on_left_click(false);

        let icon = builder.build()?;

        Ok(Self {
            icon,
            now_playing,
            progress,
            play_pause,
            like,
            dislike,
            always_on_top,
            show_title: config.tray_title,
            last: RefCell::new(None),
        })
    }

    pub fn sync(&self, state: &PlayerState) {
        // The progress line is the only thing that genuinely moves every tick.
        self.progress.set_text(if state.has_track() {
            state.progress_label()
        } else {
            "—".to_string()
        });

        let next = Rendered {
            title: state.window_title(),
            playing: state.playing,
            has_player: state.has_player,
            has_track: state.has_track(),
            liked: state.liked,
        };
        if self.last.borrow().as_ref() == Some(&next) {
            return;
        }

        self.now_playing.set_text(if next.has_track {
            next.title.as_str()
        } else {
            "Not playing"
        });
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

        // The menu bar can show the track next to the icon; the Windows
        // notification area has no such thing, so it goes in the tooltip.
        #[cfg(target_os = "macos")]
        self.icon.set_title(if self.show_title && next.has_track {
            Some(state.tray_label(TITLE_CHARS))
        } else {
            None
        });
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (&self.show_title, TITLE_CHARS);
            let tooltip = if next.has_track {
                format!("YouTube Music — {}", next.title)
            } else {
                "YouTube Music".to_string()
            };
            let _ = self.icon.set_tooltip(Some(tooltip));
        }

        *self.last.borrow_mut() = Some(next);
    }
}
