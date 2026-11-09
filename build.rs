fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

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
