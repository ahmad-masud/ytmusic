#!/usr/bin/env bash
#
# Builds `dist/YouTube Music.app`.
#
# A real .app bundle is not optional here: WKWebView only persists cookies
# (i.e. your sign-in) per bundle identifier, and MediaPlayer.framework will not
# hand an unbundled binary the Now Playing slot or the media keys.
#
#   ./scripts/bundle.sh                     # host architecture, into ./dist
#   ./scripts/bundle.sh --universal         # arm64 + x86_64, for releases
#   ./scripts/bundle.sh /Applications       # install it straight away
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="YouTube Music"
BINARY="ytmusic"
UNIVERSAL=0
DEST="$ROOT/dist"

for arg in "$@"; do
	case "$arg" in
	--universal) UNIVERSAL=1 ;;
	-*)
		echo "unknown option: $arg" >&2
		exit 1
		;;
	*) DEST="$arg" ;;
	esac
done

APP="$DEST/$APP_NAME.app"

if [[ $UNIVERSAL == 1 ]]; then
	echo "==> Building universal binary (arm64 + x86_64)"
	cargo build --release --manifest-path "$ROOT/Cargo.toml" --target aarch64-apple-darwin
	cargo build --release --manifest-path "$ROOT/Cargo.toml" --target x86_64-apple-darwin
	BUILT="$ROOT/target/$BINARY-universal"
	lipo -create -output "$BUILT" \
		"$ROOT/target/aarch64-apple-darwin/release/$BINARY" \
		"$ROOT/target/x86_64-apple-darwin/release/$BINARY"
else
	echo "==> Building release binary"
	cargo build --release --manifest-path "$ROOT/Cargo.toml"
	BUILT="$ROOT/target/release/$BINARY"
fi

echo "==> Preparing icon"
ICNS="$ROOT/packaging/AppIcon.icns"
MARK="$ROOT/packaging/ytmusic-mark.png"
if [[ ! -f "$ICNS" ]]; then
	if command -v swift >/dev/null 2>&1; then
		# The mark is optional: without it the script draws a generic glyph.
		swift "$ROOT/packaging/make-icon.swift" "$ROOT/packaging/AppIcon.iconset" \
			${MARK:+$([[ -f "$MARK" ]] && echo "$MARK")}
		iconutil -c icns "$ROOT/packaging/AppIcon.iconset" -o "$ICNS"
	else
		echo "    swift not found; bundling without an icon"
	fi
fi

echo "==> Assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BUILT" "$APP/Contents/MacOS/$BINARY"
cp "$ROOT/packaging/Info.plist" "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"
[[ -f "$ICNS" ]] && cp "$ICNS" "$APP/Contents/Resources/AppIcon.icns"

# Ad-hoc signing gives the bundle a stable code identity across rebuilds, so
# macOS keeps its keychain/TCC decisions and does not re-prompt every launch.
# It is NOT a Developer ID signature: downloaded copies still trip Gatekeeper.
echo "==> Signing (ad-hoc)"
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --strict "$APP" && echo "    signature OK"

xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

echo
echo "Built $APP ($(du -sh "$APP" | cut -f1))"
echo "Architectures: $(lipo -archs "$APP/Contents/MacOS/$BINARY")"
echo "Run it with:  open \"$APP\""
