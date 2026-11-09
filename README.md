# YouTube Music

A small native desktop app for [music.youtube.com](https://music.youtube.com) on
**macOS and Windows**: the site in a real OS window — WKWebView on macOS,
WebView2 on Windows — plus the things a browser tab cannot do. Media keys, the
system now-playing surface, a tray item, and playback that survives closing the
window.

Written in Rust with [`tao`](https://github.com/tauri-apps/tao) +
[`wry`](https://github.com/tauri-apps/wry). The only platform-specific code is
the system media layer, which has no cross-platform equivalent: an Objective-C
shim for MediaPlayer.framework on macOS, WinRT `SystemMediaTransportControls`
on Windows.

**1.4 MB on macOS, 1.1 MB on Windows.** No Electron, no Node, no bundler, no
npm install.

---

## Install

Both builds are on the [Releases](https://github.com/ahmad-masud/ytmusic/releases)
page.

### macOS

Universal (Apple Silicon + Intel), macOS 11 or later. Unzip and drag
**YouTube Music.app** into `/Applications`.

It is ad-hoc signed rather than notarised — there is no $99/year Apple
Developer certificate behind it — so Gatekeeper blocks the first launch. Open
the app once, then go to **System Settings ▸ Privacy & Security ▸ Open Anyway**.
Or `xattr -dr com.apple.quarantine "/Applications/YouTube Music.app"`.

### Windows

x64, Windows 10 1809 or later. Unzip and run **YouTube Music.exe** — there is
no installer and nothing to register. The MSVC runtime is linked statically, so
you do not need the Visual C++ Redistributable.

It needs the **WebView2 runtime**, which ships with Windows 11 and reaches most
Windows 10 machines through Edge. If the window comes up blank, install the
[Evergreen WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
and try again.

The .exe is unsigned, so SmartScreen will show "Windows protected your PC" on
first run. Click **More info ▸ Run anyway**. Code signing certificates start
around $200/year; this project does not have one.

### Build from source

This skips the Gatekeeper step entirely, because a locally compiled binary is
never quarantined.

On macOS:

```sh
./scripts/bundle.sh                # -> dist/YouTube Music.app
./scripts/bundle.sh --universal    # arm64 + x86_64, for releases
./scripts/bundle.sh /Applications  # or install it straight away
```

On Windows:

```powershell
cargo build --release              # -> target\release\ytmusic.exe
```

Requires a Rust toolchain, plus the Xcode command line tools on macOS. There
are no other dependencies: `cargo build` compiles the Objective-C shim through
the `cc` crate on macOS and embeds the icon through `winresource` on Windows,
and both icons are checked in.

The bundle is **ad-hoc signed**, which matters more than it sounds: WKWebView
keys its cookie jar to the bundle identifier, so without a stable identity you
would be signed out of YouTube Music on every rebuild, and
`MediaPlayer.framework` refuses to hand an unbundled binary the Now Playing
slot. Running `target/release/ytmusic` directly works, but you lose both.

### Homebrew

Not in the official `homebrew/cask` tap, and it cannot be. Three separate
blockers, from [Acceptable Casks](https://docs.brew.sh/Acceptable-Casks) and the
[Package Acceptance Policy](https://docs.brew.sh/Package-Acceptance-Policy):

| Requirement | Status |
|---|---|
| Artefacts "must pass Homebrew's Gatekeeper checks and must not require ... Gatekeeper to be disabled or bypassed" | Ad-hoc signature fails on a default config. Needs a Developer ID certificate and notarisation. |
| Notability: 90 forks, 90 watchers or 225 stars for a self-submission | Not yet |
| Repository at least 30 days old | Not yet |

A personal tap has none of those gates — the policy says software that does not
meet the official criteria "can generally be maintained in a third-party tap".
A ready-to-use cask lives in [`packaging/homebrew/ytmusic.rb`](packaging/homebrew/ytmusic.rb).

Worth knowing: Homebrew 7 removed `--no-quarantine`, so even a tap install is
quarantined and still prompts on first launch. Only notarisation, or building
from source, actually removes that step.

winget has the same shape of problem on the Windows side: the manifest would be
accepted, but an unsigned installer means SmartScreen still intervenes.

## The icon

`packaging/ytmusic-mark.png` is YouTube Music's own 1024×1024 product icon,
taken from Google's CDN — the same asset their web manifest serves to Chrome
when you install music.youtube.com as an app. `make-icon.swift` composites it
onto a white rounded-square plate at macOS proportions, which is the treatment
their real mobile icon already uses.

The tray glyph is the same mark reduced to its geometry and drawn at runtime in
`src/icon.rs`: a filled disc with the ring and play triangle knocked out of it.
The two platforms want different things from that one description — the macOS
menu bar takes *template* images, where only the alpha channel is used and
AppKit recolours it per theme, so the knockouts are holes; the Windows
notification area and taskbar take full-colour icons, so the knockouts are
painted white and the disc is YouTube red. `packaging/AppIcon.ico` carries the
mark at seven sizes for the Windows executable.

It is Google's trademark. Fine on your own machine; if you redistribute this,
delete `packaging/ytmusic-mark.png` and the icon script falls back to a generic
music-note glyph that ships no one else's branding.

## What it does

| | macOS | Windows |
|---|---|---|
| **Media keys** | F7/F8/F9, Touch Bar, headphone and Bluetooth remotes, via `MPRemoteCommandCenter` | Keyboard media keys via `SystemMediaTransportControls` |
| **Now playing** | Title, artist, album, artwork and a scrubbable progress bar in Control Center and on the lock screen | The same, in the media flyout above the volume OSD |
| **Tray** | Menu bar item showing the current track, with playback controls | Notification area icon; track in the tooltip, controls on right click, left click restores the window |
| **Close ≠ quit** | ⌘W hides the window and keeps playing; the Dock icon brings it back | Ctrl+W hides it; the tray icon brings it back |
| **Native menus** | Real application menu bar | Window menu bar on the HWND |
| **Remembers** | Window size and position, zoom level, always-on-top | Same |
| **External links** | Anything that is not Google or YouTube opens in your default browser | Same, via `ShellExecuteW` |

### Shortcuts

⌘ on macOS, **Ctrl** on Windows.

| | | | |
|---|---|---|---|
| ⌘P | Play / Pause | ⌘R | Reload |
| ⇧⌘→ | Next track | ⇧⌘R | Reload ignoring cache |
| ⇧⌘← | Previous track | ⌘[ / ⌘] | Back / Forward |
| ⇧⌘. | Skip forward 10s | ⇧⌘H | Home |
| ⇧⌘, | Skip back 10s | ⌘F | Focus search |
| ⇧⌘↑ / ⇧⌘↓ | Volume | ⌘+ / ⌘- / ⌘0 | Zoom |
| ⇧⌘M | Mute | ⌥⌘I | Developer tools |
| ⇧⌘L / ⇧⌘D | Like / Dislike | ⌘W | Hide window |
| ⇧⌘S | Shuffle | ⌘Q | Quit |
| ⇧⌘C | Copy song link | | |

## Settings

Written on quit and read on launch. There is no settings UI; edit the file.

| | |
|---|---|
| macOS | `~/Library/Application Support/YouTube Music/config.json` |
| Windows | `%APPDATA%\YouTube Music\config.json` |

| Key | Default | |
|---|---|---|
| `close_to_tray` | `true` | ⌘W hides instead of quitting |
| `tray_title` | `true` | Show the track name in the menu bar |
| `native_now_playing` | `true` | Publish to Control Center and capture media keys |
| `always_on_top` | `false` | Float above other windows |
| `zoom` | `1.0` | Webview zoom factor |
| `devtools` | `false` | Web inspector; costs ~60 MB of GPU-process memory |
| `user_agent` | Safari 18.6 / empty | See below |
| `start_url` | `https://music.youtube.com/` | |

### About the user agent

On macOS this matters: WKWebView's stock user agent has no `Safari` token, which
makes YouTube serve a degraded page and makes Google's sign-in refuse the
"embedded browser". The app presents itself as Safari instead. If Google ever
tightens this and sign-in starts failing, bump the `Version/` number in
`user_agent` to match the Safari on your machine.

On Windows there is nothing to fix, so the default is empty and the engine's own
user agent is left alone: WebView2 already identifies as Edge, which YouTube
Music serves its full desktop experience to.

## Development

```sh
cargo build && cargo clippy
```

There is an offline fixture that stands in for YouTube Music, so the IPC bridge,
the window title, the menu bar item and the Now Playing tile can be exercised
without signing in:

```sh
(cd tests/fixture && python3 -m http.server 8765) &
YTMUSIC_DEBUG=1 YTMUSIC_START_URL=http://localhost:8765/player.html \
  "dist/YouTube Music.app/Contents/MacOS/ytmusic"
```

`YTMUSIC_DEBUG=1` logs every message crossing the bridge. Neither variable is
written back to `config.json`.

### Layout

```
src/main.rs          event loop, window, webview, action dispatch
src/inject.js        runs inside the page: scrapes player state, exposes commands
src/media/mod.rs     platform-agnostic now-playing + media key API
src/media/macos.rs     -> native/nowplaying.m (MPNowPlayingInfoCenter)
src/media/windows.rs   -> SystemMediaTransportControls (WinRT)
src/shell.rs         opening links, revealing folders, clipboard, raising the window
src/menu.rs          application menu; NSMenu on macOS, HWND menu bar on Windows
src/tray.rs          menu bar / notification area item
src/icon.rs          the tray and window icons, rasterised at runtime
src/config.rs        preferences and window geometry
packaging/           Info.plist, both icons, the icon generator, Homebrew cask
```

The split worth knowing: **`inject.js` is the only file that knows anything
about YouTube Music's DOM.** It reads `navigator.mediaSession` first and falls
back to scraping the player bar, and every selector has alternatives. When
YouTube ships a redesign, that is the file to fix — the Rust side only ever sees
a `PlayerState` struct.

## Performance

Measured on an M-series Mac, app settled on the signed-out home page,
`devtools: false`. The Windows build has not been profiled; WebView2 is
Chromium rather than WebKit, so expect different numbers there.

| | Memory (physical footprint) | CPU, 60 s idle |
|---|---|---|
| `ytmusic` — everything I wrote | **32 MB** | **0.08%** of one core |
| WebKit GPU + Networking + aux | 59 MB | — |
| WebContent — YouTube Music itself | **491 MB** | 1.38% of one core |

The shape of that table is the whole story: **the native shell is ~5% of the
memory and ~5% of the CPU. The web app is everything else**, and it is the same
WebKit running the same JavaScript as a Safari tab, so there is no version of
this that is dramatically cheaper. Page memory peaks near 565 MB while loading
and WebKit reclaims it to ~460–490 MB; it is stable there, not climbing. Window
size barely moves it.

What this architecture does buy you is the 1.6 MB download instead of ~150 MB,
one WebKit instead of a second browser engine resident alongside Safari, and a
cold start in well under a second.

Three things were measured and fixed while writing this:

- **9,473 DOM nodes walked per second.** The shadow-DOM fallback in
  `inject.js` re-ran a full tree walk on every tick for any selector that
  matched nothing — which is every tick, before you sign in. Negative results
  are now cached with a 10 s backoff: ~9.5k node visits per second became ~9.5k
  per ten seconds, and a snapshot went from 2–4 ms to under 1 ms.
- **Menu churn.** The app menu and tray rewrote ~9 item labels a second whether
  or not anything changed. They now diff first; only the progress line, which
  genuinely moves, is written every tick.
- **Now Playing publishing.** Republishing `MPNowPlayingInfoCenter` every second
  is wasteful — the system interpolates the scrubber from elapsed + rate. But
  publishing *only* on change turns out to break it: macOS retires a session
  that stops refreshing and the Control Center tile silently disappears. There
  is a floor, so there is now a 5 s heartbeat alongside the change-driven
  updates. Verified both ways.

## Limitations

- **The Windows build has not been hand-tested on Windows hardware.** It is
  built, linted and packaged on a Windows runner in CI, and the binary is
  verified to be a GUI-subsystem PE with no Visual C++ Redistributable
  dependency — but nobody has yet watched it play a song. The
  `SystemMediaTransportControls` integration in particular is typed against the
  real WinRT API and compiles, which is not the same as working. Please open an
  issue if something is off.

- **Sign-in may be refused.** Google sometimes blocks logins from embedded
  browsers. Pretending to be Safari gets through today; if it stops, adjust
  `user_agent`.
- **The page is YouTube's.** No ad blocking, no premium-feature workarounds —
  this is a window around the real site, and it behaves exactly like the site.
- **Selectors rot.** Next/previous/like go through YouTube Music's own buttons,
  so a redesign can break them until `inject.js` is updated. Play/pause and all
  the metadata use standard web APIs and are much more stable.

## Notice

Unofficial. Not affiliated with, endorsed by, or connected to Google or YouTube.
Your use of the site through this window is still governed by YouTube's terms.
The YouTube Music name and mark belong to Google; the code is MIT licensed, the
icon is not mine to license.
