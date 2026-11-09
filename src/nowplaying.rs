//! Safe wrapper around `native/nowplaying.m`.
//!
//! Gives us the macOS "Now Playing" tile in Control Center and routes the
//! hardware media keys into the app.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;

/// Commands the system can send us. Keep in sync with `native/nowplaying.m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek,
    Stop,
    SkipForward,
    SkipBackward,
}

impl RemoteCommand {
    fn from_raw(raw: c_int) -> Option<Self> {
        Some(match raw {
            1 => Self::Play,
            2 => Self::Pause,
            3 => Self::Toggle,
            4 => Self::Next,
            5 => Self::Previous,
            6 => Self::Seek,
            7 => Self::Stop,
            8 => Self::SkipForward,
            9 => Self::SkipBackward,
            _ => return None,
        })
    }
}

type Handler = Box<dyn Fn(RemoteCommand, f64) + Send + Sync + 'static>;

static HANDLER: OnceLock<Handler> = OnceLock::new();

unsafe extern "C" {
    fn ytm_native_init(callback: extern "C" fn(c_int, f64));
    fn ytm_native_update(
        title: *const c_char,
        artist: *const c_char,
        album: *const c_char,
        artwork_url: *const c_char,
        duration: f64,
        elapsed: f64,
        playing: c_int,
    );
    fn ytm_native_clear();
    fn ytm_native_copy_text(text: *const c_char);
    fn ytm_native_activate();
}

extern "C" fn trampoline(command: c_int, value: f64) {
    if let (Some(handler), Some(command)) = (HANDLER.get(), RemoteCommand::from_raw(command)) {
        handler(command, value);
    }
}

/// Register the remote-command handler and enable media-key capture.
///
/// The handler is called on the main thread, but it is required to be
/// `Send + Sync` because AppKit makes no promise about that.
pub fn init<F>(handler: F)
where
    F: Fn(RemoteCommand, f64) + Send + Sync + 'static,
{
    if HANDLER.set(Box::new(handler)).is_err() {
        return; // already initialised
    }
    unsafe { ytm_native_init(trampoline) }
}

/// Push the current track into the system Now Playing tile.
pub fn update(
    title: &str,
    artist: &str,
    album: &str,
    artwork_url: &str,
    duration: f64,
    elapsed: f64,
    playing: bool,
) {
    let Ok(title) = CString::new(title) else { return };
    let Ok(artist) = CString::new(artist) else { return };
    let Ok(album) = CString::new(album) else { return };
    let Ok(artwork) = CString::new(artwork_url) else { return };

    unsafe {
        ytm_native_update(
            title.as_ptr(),
            artist.as_ptr(),
            album.as_ptr(),
            artwork.as_ptr(),
            duration,
            elapsed,
            playing as c_int,
        )
    }
}

/// Drop out of the Now Playing tile entirely.
pub fn clear() {
    unsafe { ytm_native_clear() }
}

/// Put `text` on the system clipboard.
pub fn copy_to_clipboard(text: &str) {
    if let Ok(text) = CString::new(text) {
        unsafe { ytm_native_copy_text(text.as_ptr()) }
    }
}

/// Bring the application to the front.
pub fn activate() {
    unsafe { ytm_native_activate() }
}
