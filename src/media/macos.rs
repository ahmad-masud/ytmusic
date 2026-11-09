//! macOS backend: `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`, via
//! the Objective-C shim in `native/nowplaying.m`.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use tao::window::Window;

use super::{NowPlaying, RemoteCommand};

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
}

/// Keep in sync with the `YtmCommand` enum in `native/nowplaying.m`.
fn command_from_raw(raw: c_int) -> Option<RemoteCommand> {
    Some(match raw {
        1 => RemoteCommand::Play,
        2 => RemoteCommand::Pause,
        3 => RemoteCommand::Toggle,
        4 => RemoteCommand::Next,
        5 => RemoteCommand::Previous,
        6 => RemoteCommand::Seek,
        7 => RemoteCommand::Stop,
        8 => RemoteCommand::SkipForward,
        9 => RemoteCommand::SkipBackward,
        _ => return None,
    })
}

extern "C" fn trampoline(command: c_int, value: f64) {
    if let Some(command) = command_from_raw(command) {
        super::dispatch(command, value);
    }
}

pub fn init(_window: &Window) {
    unsafe { ytm_native_init(trampoline) }
}

pub fn update(now: &NowPlaying<'_>) {
    let Ok(title) = CString::new(now.title) else {
        return;
    };
    let Ok(artist) = CString::new(now.artist) else {
        return;
    };
    let Ok(album) = CString::new(now.album) else {
        return;
    };
    let Ok(artwork) = CString::new(now.artwork_url) else {
        return;
    };

    unsafe {
        ytm_native_update(
            title.as_ptr(),
            artist.as_ptr(),
            album.as_ptr(),
            artwork.as_ptr(),
            now.duration,
            now.elapsed,
            now.playing as c_int,
        )
    }
}

pub fn clear() {
    unsafe { ytm_native_clear() }
}
