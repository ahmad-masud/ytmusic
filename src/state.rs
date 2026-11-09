//! The player state we scrape out of the page, and the IPC envelope it
//! arrives in.

use serde::Deserialize;

/// A snapshot of what YouTube Music is currently doing.
///
/// Produced once a second (and immediately on play/pause/seek) by
/// `src/inject.js` and posted over wry's IPC channel.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlayerState {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork: String,
    /// Seconds.
    pub duration: f64,
    /// Seconds.
    pub elapsed: f64,
    pub playing: bool,
    /// `1` liked, `-1` disliked, `0` neither.
    pub liked: i8,
    /// Whether a `<video>` element exists yet (false on the splash/login page).
    pub has_player: bool,
    pub url: String,
}

impl PlayerState {
    pub fn has_track(&self) -> bool {
        !self.title.is_empty()
    }

    /// `true` when something other than the playhead moved, i.e. when the
    /// system Now Playing tile needs a full refresh rather than a nudge.
    pub fn track_changed(&self, previous: &Self) -> bool {
        self.title != previous.title
            || self.artist != previous.artist
            || self.album != previous.album
            || self.artwork != previous.artwork
    }

    /// `Song — Artist`, for the window title.
    pub fn window_title(&self) -> String {
        match (self.title.as_str(), self.artist.as_str()) {
            ("", _) => "YouTube Music".to_string(),
            (title, "") => title.to_string(),
            (title, artist) => format!("{title} — {artist}"),
        }
    }

    /// A compact label for the tray, truncated on a character boundary.
    pub fn tray_label(&self, max_chars: usize) -> String {
        let label = self.window_title();
        if label.chars().count() <= max_chars {
            return label;
        }
        let truncated: String = label.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated.trim_end())
    }

    pub fn progress_label(&self) -> String {
        format!(
            "{} / {}",
            format_duration(self.elapsed),
            format_duration(self.duration)
        )
    }
}

pub fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total = seconds.round() as u64;
    let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// Messages the injected script can send us.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum IpcMessage {
    /// A player snapshot.
    State(PlayerState),
    /// The injected script finished installing on a page.
    Ready,
    /// Something went wrong in the page; forwarded to stderr.
    Log(String),
    /// User asked to copy the current track's link.
    CopyLink(String),
}
