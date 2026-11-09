//! On-disk preferences and window geometry.
//!
//! Lives at `~/Library/Application Support/YouTube Music/config.json` and is
//! plain JSON on purpose: there is no preferences UI, so hand-editing is the
//! intended way to change the less common knobs.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "YouTube Music";

fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}
pub const HOME_URL: &str = "https://music.youtube.com/";

/// Pretend to be Safari. WKWebView's stock user agent omits the `Safari`
/// token, which makes YouTube serve a degraded page and makes Google's sign-in
/// flow refuse the "embedded browser". Overridable because Google occasionally
/// moves the goalposts.
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
     AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window: WindowConfig,
    /// Webview zoom factor, `1.0` is 100%.
    pub zoom: f64,
    /// Keep the window above other apps.
    pub always_on_top: bool,
    /// Closing the window hides it (and keeps playing) instead of quitting.
    pub close_to_tray: bool,
    /// Show the track name next to the menu bar icon.
    pub tray_title: bool,
    /// Publish to Control Center / capture media keys via MediaPlayer.framework.
    /// Turn this off if it ever fights with WebKit's own media session.
    pub native_now_playing: bool,
    /// Web inspector support (`View ▸ Toggle Developer Tools`). Off by default:
    /// an inspectable webview costs roughly 60 MB of GPU-process memory, and
    /// almost nobody opens it.
    pub devtools: bool,
    pub user_agent: String,
    pub start_url: String,
    /// Set by `YTMUSIC_DEBUG=1`; never written to disk.
    #[serde(skip)]
    pub debug: bool,
    /// Set by `YTMUSIC_START_URL`; never written to disk, so pointing the app
    /// at a fixture for one run does not become the permanent home page.
    #[serde(skip)]
    pub start_url_override: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 820.0,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            zoom: 1.0,
            always_on_top: false,
            close_to_tray: true,
            tray_title: true,
            native_now_playing: true,
            devtools: false,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            start_url: HOME_URL.to_string(),
            debug: false,
            start_url_override: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::path();
        let mut config = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Self>(&raw) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("ytmusic: ignoring unreadable config at {path:?}: {err}");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };

        // Environment overrides, for debugging against a fixture page.
        config.debug = matches!(std::env::var("YTMUSIC_DEBUG").as_deref(), Ok("1" | "true"));
        config.start_url_override = std::env::var("YTMUSIC_START_URL").ok();

        config.sanitised()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            eprintln!("ytmusic: could not create {parent:?}: {err}");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(err) = fs::write(&path, json) {
                    eprintln!("ytmusic: could not write {path:?}: {err}");
                }
            }
            Err(err) => eprintln!("ytmusic: could not serialise config: {err}"),
        }
    }

    /// Clamp values that would produce an unusable window.
    fn sanitised(mut self) -> Self {
        self.window.width = self.window.width.clamp(420.0, 10_000.0);
        self.window.height = self.window.height.clamp(360.0, 10_000.0);
        self.zoom = self.zoom.clamp(0.4, 3.0);
        if self.user_agent.trim().is_empty() {
            self.user_agent = DEFAULT_USER_AGENT.to_string();
        }
        if !is_web_url(&self.start_url) {
            self.start_url = HOME_URL.to_string();
        }
        if !self.start_url_override.as_deref().is_none_or(is_web_url) {
            self.start_url_override = None;
        }
        self
    }

    /// The URL to open on launch: the environment override if there is one,
    /// otherwise the saved home page.
    pub fn start_url(&self) -> &str {
        self.start_url_override.as_deref().unwrap_or(&self.start_url)
    }

    pub fn dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }
}
