//! System media integration: the OS "now playing" surface and the hardware
//! media keys.
//!
//! Both platforms expose the same shape — tell the OS what is playing, and get
//! told when the user presses a transport button — behind wildly different
//! APIs:
//!
//!   * macOS: `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter`, reached
//!     through a small Objective-C shim in `native/nowplaying.m`.
//!   * Windows: `SystemMediaTransportControls` (WinRT), bound to the app's
//!     window via the `ISystemMediaTransportControlsInterop` factory.
//!
//! Everything above this module is platform-agnostic.

use std::sync::OnceLock;

use tao::window::Window;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

/// A transport command coming *from* the operating system.
///
/// Not every platform raises every variant: SMTC resolves the play/pause key
/// to `Play` or `Pause` itself, so `Toggle` is macOS-only.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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

/// What the OS should display.
#[derive(Debug, Clone, Copy, Default)]
pub struct NowPlaying<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub artwork_url: &'a str,
    /// Seconds.
    pub duration: f64,
    /// Seconds.
    pub elapsed: f64,
    pub playing: bool,
}

type Handler = Box<dyn Fn(RemoteCommand, f64) + Send + Sync + 'static>;

static HANDLER: OnceLock<Handler> = OnceLock::new();

/// Register the transport handler and hand the OS its controls.
///
/// The handler must be `Send + Sync`: macOS invokes it on the main thread, but
/// Windows raises `ButtonPressed` on a system thread pool thread.
pub fn init<F>(window: &Window, handler: F)
where
    F: Fn(RemoteCommand, f64) + Send + Sync + 'static,
{
    if HANDLER.set(Box::new(handler)).is_err() {
        return; // already initialised
    }
    imp::init(window);
}

/// Called by the platform backends when the OS sends a transport command.
pub(crate) fn dispatch(command: RemoteCommand, value: f64) {
    if let Some(handler) = HANDLER.get() {
        handler(command, value);
    }
}

/// Publish the current track.
pub fn update(now: &NowPlaying<'_>) {
    imp::update(now);
}

/// Give up the OS media session.
pub fn clear() {
    imp::clear();
}
