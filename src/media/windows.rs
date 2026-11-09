//! Windows backend: `SystemMediaTransportControls`.
//!
//! SMTC is what draws the media flyout above the volume OSD and what routes
//! the keyboard's media keys. A desktop (Win32) app cannot use
//! `GetForCurrentView` — that needs a UWP CoreWindow — so it goes through the
//! `ISystemMediaTransportControlsInterop` factory and binds to our HWND.
//!
//! Every call here is fail-soft: if SMTC is unavailable the app keeps working,
//! it just loses the OS-level transport surface.

use std::cell::RefCell;
use std::ffi::c_void;

use tao::platform::windows::WindowExtWindows;
use tao::window::Window;
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs, SystemMediaTransportControlsDisplayUpdater,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::core::{HSTRING, Ref, Result};

use super::{NowPlaying, RemoteCommand};

struct Controls {
    controls: SystemMediaTransportControls,
    updater: SystemMediaTransportControlsDisplayUpdater,
    /// Last values pushed, so we only rebuild what actually changed. Setting a
    /// thumbnail in particular kicks off a fetch.
    artwork: String,
    metadata: (String, String, String),
}

// WinRT objects here are apartment-bound. `init` and `update` both run on the
// event loop thread, so a thread local is both correct and cheaper than a lock.
thread_local! {
    static CONTROLS: RefCell<Option<Controls>> = const { RefCell::new(None) };
}

fn ticks(seconds: f64) -> TimeSpan {
    // WinRT TimeSpan is in 100-nanosecond ticks.
    let clamped = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
    };
    TimeSpan {
        Duration: (clamped * 10_000_000.0) as i64,
    }
}

pub fn init(window: &Window) {
    if let Err(error) = setup(window) {
        eprintln!("ytmusic: system media controls unavailable: {error}");
    }
}

fn setup(window: &Window) -> Result<()> {
    let hwnd = HWND(window.hwnd() as *mut c_void);

    let interop = windows::core::factory::<
        SystemMediaTransportControls,
        ISystemMediaTransportControlsInterop,
    >()?;
    let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd) }?;

    controls.SetIsEnabled(true)?;
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsStopEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;

    // Raised on a thread pool thread, not ours — `super::dispatch` is
    // thread-safe for exactly this reason.
    controls.ButtonPressed(&TypedEventHandler::<
        SystemMediaTransportControls,
        SystemMediaTransportControlsButtonPressedEventArgs,
    >::new(|_, args: Ref<_>| {
        let command = match args.ok()?.Button()? {
            SystemMediaTransportControlsButton::Play => Some(RemoteCommand::Play),
            SystemMediaTransportControlsButton::Pause => Some(RemoteCommand::Pause),
            SystemMediaTransportControlsButton::Stop => Some(RemoteCommand::Stop),
            SystemMediaTransportControlsButton::Next => Some(RemoteCommand::Next),
            SystemMediaTransportControlsButton::Previous => Some(RemoteCommand::Previous),
            SystemMediaTransportControlsButton::FastForward => Some(RemoteCommand::SkipForward),
            SystemMediaTransportControlsButton::Rewind => Some(RemoteCommand::SkipBackward),
            _ => None,
        };
        if let Some(command) = command {
            super::dispatch(command, 0.0);
        }
        Ok(())
    }))?;

    // Lets the user scrub the flyout's progress bar.
    controls.PlaybackPositionChangeRequested(&TypedEventHandler::<
        SystemMediaTransportControls,
        PlaybackPositionChangeRequestedEventArgs,
    >::new(|_, args: Ref<_>| {
        let position = args.ok()?.RequestedPlaybackPosition()?;
        super::dispatch(
            RemoteCommand::Seek,
            position.Duration as f64 / 10_000_000.0,
        );
        Ok(())
    }))?;

    let updater = controls.DisplayUpdater()?;
    updater.SetType(MediaPlaybackType::Music)?;

    CONTROLS.with(|slot| {
        *slot.borrow_mut() = Some(Controls {
            controls,
            updater,
            artwork: String::new(),
            metadata: (String::new(), String::new(), String::new()),
        });
    });

    Ok(())
}

pub fn update(now: &NowPlaying<'_>) {
    CONTROLS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else { return };
        if let Err(error) = push(state, now) {
            eprintln!("ytmusic: could not update media controls: {error}");
        }
    });
}

fn push(state: &mut Controls, now: &NowPlaying<'_>) -> Result<()> {
    let metadata = (
        now.title.to_string(),
        now.artist.to_string(),
        now.album.to_string(),
    );

    if metadata != state.metadata {
        let music = state.updater.MusicProperties()?;
        music.SetTitle(&HSTRING::from(now.title))?;
        music.SetArtist(&HSTRING::from(now.artist))?;
        music.SetAlbumTitle(&HSTRING::from(now.album))?;
        state.metadata = metadata;
    }

    if now.artwork_url != state.artwork {
        state.artwork = now.artwork_url.to_string();
        if now.artwork_url.is_empty() {
            state.updater.SetThumbnail(None)?;
        } else {
            let uri = Uri::CreateUri(&HSTRING::from(now.artwork_url))?;
            let thumbnail = RandomAccessStreamReference::CreateFromUri(&uri)?;
            state.updater.SetThumbnail(&thumbnail)?;
        }
    }

    state.updater.Update()?;

    state.controls.SetPlaybackStatus(if now.playing {
        MediaPlaybackStatus::Playing
    } else {
        MediaPlaybackStatus::Paused
    })?;

    let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
    timeline.SetStartTime(ticks(0.0))?;
    timeline.SetMinSeekTime(ticks(0.0))?;
    timeline.SetPosition(ticks(now.elapsed))?;
    timeline.SetMaxSeekTime(ticks(now.duration))?;
    timeline.SetEndTime(ticks(now.duration))?;
    state.controls.UpdateTimelineProperties(&timeline)?;

    Ok(())
}

pub fn clear() {
    CONTROLS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else { return };
        state.artwork.clear();
        state.metadata = (String::new(), String::new(), String::new());
        let _ = state.updater.ClearAll();
        let _ = state
            .controls
            .SetPlaybackStatus(MediaPlaybackStatus::Stopped);
    });
}
