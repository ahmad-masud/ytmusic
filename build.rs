//! Per-platform build steps.
//!
//! macOS: compile the Objective-C shim that reaches MediaPlayer.framework.
//! Windows: embed the icon and version info into the executable.

fn main() {
    match std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default().as_str() {
        "macos" => build_macos_shim(),
        "windows" => embed_windows_resources(),
        _ => {}
    }
}

fn build_macos_shim() {
    println!("cargo:rerun-if-changed=native/nowplaying.m");

    cc::Build::new()
        .file("native/nowplaying.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .warnings(false)
        .compile("ytm_native");

    for framework in ["Foundation", "AppKit", "MediaPlayer"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

/// Embedding resources needs `rc.exe` from the Windows SDK, so it only happens
/// when the host is Windows. Cross-compiling from macOS or Linux still produces
/// a working binary; it just has no icon in Explorer.
#[cfg(windows)]
fn embed_windows_resources() {
    println!("cargo:rerun-if-changed=packaging/AppIcon.ico");

    let mut resources = winresource::WindowsResource::new();
    resources.set_icon("packaging/AppIcon.ico");
    resources.set("ProductName", "YouTube Music");
    resources.set("FileDescription", "YouTube Music");
    resources.set("CompanyName", "Unofficial. Not affiliated with Google or YouTube.");
    resources.set("LegalCopyright", "MIT licensed. YouTube Music is a trademark of Google LLC.");

    if let Err(error) = resources.compile() {
        println!("cargo:warning=could not embed Windows resources: {error}");
    }
}

#[cfg(not(windows))]
fn embed_windows_resources() {
    println!(
        "cargo:warning=cross-compiling for Windows: skipping the icon resource (needs rc.exe)"
    );
}
