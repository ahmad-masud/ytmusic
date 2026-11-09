//! Small OS integrations that are not about media: opening links in the real
//! browser, revealing a folder, the clipboard, and raising the window.

use std::path::Path;
use std::process::Command;

use tao::window::Window;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn ytm_native_activate();
}

/// Hand a URL to the user's default browser.
///
/// Only schemes we meant to support are passed through, so a hostile page
/// cannot talk us into launching an arbitrary protocol handler.
pub fn open_externally(url: &str) {
    let lowered = url.to_ascii_lowercase();
    let allowed = ["http://", "https://", "mailto:", "tel:"]
        .iter()
        .any(|scheme| lowered.starts_with(scheme));
    if !allowed {
        eprintln!("ytmusic: refusing to open {url}");
        return;
    }
    launch(url);
}

/// Open a folder in Finder / File Explorer.
pub fn reveal_path(path: &Path) {
    let Some(path) = path.to_str() else {
        eprintln!("ytmusic: path is not valid UTF-8: {path:?}");
        return;
    };
    #[cfg(target_os = "windows")]
    {
        if let Err(err) = Command::new("explorer.exe").arg(path).spawn() {
            eprintln!("ytmusic: could not reveal {path}: {err}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    launch(path);
}

#[cfg(target_os = "macos")]
fn launch(target: &str) {
    // `--` so a target beginning with `-` is not read as a flag.
    if let Err(err) = Command::new("/usr/bin/open").args(["--", target]).spawn() {
        eprintln!("ytmusic: could not open {target}: {err}");
    }
}

#[cfg(target_os = "windows")]
fn launch(target: &str) {
    use std::ffi::c_void;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::PCWSTR;

    // ShellExecuteW rather than `cmd /c start`: no command line to quote, so
    // nothing in the URL can be read as shell syntax.
    let verb = wide("open");
    let file = wide(target);
    let result = unsafe {
        ShellExecuteW(
            Some(HWND(std::ptr::null_mut::<c_void>())),
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value <= 32 on failure.
    if result.0 as isize <= 32 {
        eprintln!("ytmusic: could not open {target}");
    }
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Put `text` on the system clipboard.
pub fn copy_to_clipboard(text: &str) {
    let result = arboard::Clipboard::new().and_then(|mut board| board.set_text(text.to_owned()));
    if let Err(err) = result {
        eprintln!("ytmusic: could not copy to clipboard: {err}");
    }
}

/// Show the window and put the app in front.
pub fn bring_to_front(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
    // tao's set_focus raises the window, but on macOS the *application* also
    // has to be activated or the window comes up behind the current app.
    #[cfg(target_os = "macos")]
    unsafe {
        ytm_native_activate()
    };
}
