//! Every user-triggerable action, and the stable string ids used as menu ids.
//!
//! Both the app menu and the tray menu emit these; the tray prefixes its ids
//! so the two menus can carry the same action without colliding.

pub const TRAY_PREFIX: &str = "tray:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Playback
    PlayPause,
    Next,
    Previous,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    ToggleMute,
    Like,
    Dislike,
    Shuffle,
    Repeat,
    // Navigation
    Reload,
    HardReload,
    Home,
    Back,
    Forward,
    Search,
    // Window
    ShowWindow,
    HideWindow,
    ToggleAlwaysOnTop,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    DevTools,
    // Misc
    CopyLink,
    OpenInBrowser,
    RevealConfig,
    Quit,
}

impl Action {
    pub const fn id(self) -> &'static str {
        match self {
            Self::PlayPause => "play-pause",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::SeekForward => "seek-forward",
            Self::SeekBackward => "seek-backward",
            Self::VolumeUp => "volume-up",
            Self::VolumeDown => "volume-down",
            Self::ToggleMute => "toggle-mute",
            Self::Like => "like",
            Self::Dislike => "dislike",
            Self::Shuffle => "shuffle",
            Self::Repeat => "repeat",
            Self::Reload => "reload",
            Self::HardReload => "hard-reload",
            Self::Home => "home",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Search => "search",
            Self::ShowWindow => "show-window",
            Self::HideWindow => "hide-window",
            Self::ToggleAlwaysOnTop => "always-on-top",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::ZoomReset => "zoom-reset",
            Self::DevTools => "devtools",
            Self::CopyLink => "copy-link",
            Self::OpenInBrowser => "open-in-browser",
            Self::RevealConfig => "reveal-config",
            Self::Quit => "quit",
        }
    }

    /// Menu id for the tray copy of this action.
    pub fn tray_id(self) -> String {
        format!("{TRAY_PREFIX}{}", self.id())
    }

    pub fn from_menu_id(id: &str) -> Option<Self> {
        let id = id.strip_prefix(TRAY_PREFIX).unwrap_or(id);
        const ALL: &[Action] = &[
            Action::PlayPause,
            Action::Next,
            Action::Previous,
            Action::SeekForward,
            Action::SeekBackward,
            Action::VolumeUp,
            Action::VolumeDown,
            Action::ToggleMute,
            Action::Like,
            Action::Dislike,
            Action::Shuffle,
            Action::Repeat,
            Action::Reload,
            Action::HardReload,
            Action::Home,
            Action::Back,
            Action::Forward,
            Action::Search,
            Action::ShowWindow,
            Action::HideWindow,
            Action::ToggleAlwaysOnTop,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::ZoomReset,
            Action::DevTools,
            Action::CopyLink,
            Action::OpenInBrowser,
            Action::RevealConfig,
            Action::Quit,
        ];
        ALL.iter().copied().find(|action| action.id() == id)
    }

    /// The `window.__ytmNative.cmd` call this action maps to, if any.
    pub const fn page_command(self) -> Option<(&'static str, Option<f64>)> {
        Some(match self {
            Self::PlayPause => ("playPause", None),
            Self::Next => ("next", None),
            Self::Previous => ("previous", None),
            Self::SeekForward => ("seekBy", Some(10.0)),
            Self::SeekBackward => ("seekBy", Some(-10.0)),
            Self::VolumeUp => ("volumeBy", Some(0.1)),
            Self::VolumeDown => ("volumeBy", Some(-0.1)),
            Self::ToggleMute => ("toggleMute", None),
            Self::Like => ("like", None),
            Self::Dislike => ("dislike", None),
            Self::Shuffle => ("shuffle", None),
            Self::Repeat => ("repeat", None),
            Self::Search => ("search", None),
            _ => return None,
        })
    }
}
