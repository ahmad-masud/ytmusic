# YouTube Music for macOS

A small native desktop app for [music.youtube.com](https://music.youtube.com): a
`WKWebView` in a real Mac window, plus the things a browser tab cannot do —
media keys, the Control Center **Now Playing** tile, a menu bar item, and
playback that survives closing the window.

Written in Rust with [`tao`](https://github.com/tauri-apps/tao) +
[`wry`](https://github.com/tauri-apps/wry), with a small Objective-C shim for
the two AppKit frameworks that have no usable Rust bindings.

**1.6 MB app bundle.** No Electron, no Node, no bundler, no npm install.

---

## Install

### Download

Grab the universal build from [Releases](https://github.com/ahmad-masud/ytmusic/releases),
unzip, and drag **YouTube Music.app** into `/Applications`.

It is ad-hoc signed rather than notarised — there is no $99/year Apple
Developer certificate behind it — so Gatekeeper blocks the first launch. Open
the app once, then go to **System Settings ▸ Privacy & Security ▸ Open Anyway**.
Or `xattr -dr com.apple.quarantine "/Applications/YouTube Music.app"`.

### Build from source

This skips the Gatekeeper step entirely, because a locally compiled binary is
never quarantined.

```sh
./scripts/bundle.sh                # -> dist/YouTube Music.app
./scripts/bundle.sh --universal    # arm64 + x86_64, for releases
./scripts/bundle.sh /Applications  # or install it straight away
```

Requires a Rust toolchain and the Xcode command line tools. There are no other
dependencies: `cargo build` compiles the Objective-C shim through the `cc`
crate, and the icon is checked in.

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

## The icon

`packaging/ytmusic-mark.png` is YouTube Music's own 1024×1024 product icon,
taken from Google's CDN — the same asset their web manifest serves to Chrome
when you install music.youtube.com as an app. `make-icon.swift` composites it
onto a white rounded-square plate at macOS proportions, which is the treatment
their real mobile icon already uses.

The menu bar glyph is the same mark reduced to a silhouette and drawn in
`src/icon.rs`: a filled disc with the ring and play triangle knocked out of it.
The menu bar takes *template* images — AppKit uses only the alpha channel and
recolours it per theme — so a full-colour logo would be wrong there, and
unreadable at 18pt besides.

It is Google's trademark. Fine on your own machine; if you redistribute this,
delete `packaging/ytmusic-mark.png` and the icon script falls back to a generic
music-note glyph that ships no one else's branding.

## What it does

| | |
|---|---|
| **Media keys** | F7/F8/F9, Touch Bar, headphone and Bluetooth remotes, via `MPRemoteCommandCenter` |
| **Now Playing** | Title, artist, album, artwork and a scrubbable progress bar in Control Center and on the lock screen |
| **Menu bar** | Current track, play/pause, skip, like, always-on-top, quit |
| **Close ≠ quit** | ⌘W hides the window and keeps playing; the Dock icon or the menu bar item brings it back |
| **Native menus** | A real macOS menu bar with the shortcuts below |
| **Remembers** | Window size and position, zoom level, always-on-top |
| **External links** | Anything that is not Google or YouTube opens in your default browser |

### Shortcuts

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

`~/Library/Application Support/YouTube Music/config.json`, written on quit and
read on launch. There is no settings UI; edit the file.

| Key | Default | |
|---|---|---|
| `close_to_tray` | `true` | ⌘W hides instead of quitting |
| `tray_title` | `true` | Show the track name in the menu bar |
| `native_now_playing` | `true` | Publish to Control Center and capture media keys |
| `always_on_top` | `false` | Float above other windows |
| `zoom` | `1.0` | Webview zoom factor |
| `devtools` | `false` | Web inspector; costs ~60 MB of GPU-process memory |
| `user_agent` | Safari 18.6 | See below |
| `start_url` | `https://music.youtube.com/` | |

### About the user agent

WKWebView's stock user agent has no `Safari` token, which makes YouTube serve a
degraded page and makes Google's sign-in refuse the "embedded browser". The app
presents itself as Safari instead. If Google ever tightens this and sign-in
starts failing, bump the `Version/` number in `user_agent` to match the Safari
on your machine.

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
src/main.rs        event loop, window, webview, action dispatch
src/inject.js      runs inside the page: scrapes player state, exposes commands
src/nowplaying.rs  safe wrapper over the Objective-C shim
native/nowplaying.m  MPNowPlayingInfoCenter + MPRemoteCommandCenter
src/menu.rs        macOS application menu
src/tray.rs        menu bar item
src/icon.rs        the menu bar glyph, rasterised at runtime
src/config.rs      preferences and window geometry
packaging/         Info.plist and the icon generator
```

The split worth knowing: **`inject.js` is the only file that knows anything
about YouTube Music's DOM.** It reads `navigator.mediaSession` first and falls
back to scraping the player bar, and every selector has alternatives. When
YouTube ships a redesign, that is the file to fix — the Rust side only ever sees
a `PlayerState` struct.

## Performance

Measured on an M-series Mac, app settled on the signed-out home page,
`devtools: false`.

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
