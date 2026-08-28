#!/usr/bin/env bash
# Wrap the winit Media server in a Finder-launchable macOS application bundle.
#
# The bundle is a regular application. It was an accessory one (`LSUIElement`) while its outputs
# were assumed to be surfaces nobody would ever touch, but an accessory application cannot open a
# full-screen Space: `toggleFullScreen:` fails, the green button does nothing, and an operator is
# left with a window that will not go full screen. Its outputs are ordinary windows an operator
# maximizes, so it is an ordinary application. The menu bar item the runtime creates stays.
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: bundle-media-macos.sh BINARY OUTPUT_DIR [VERSION]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$1"
OUTPUT_DIR="$2"
VERSION="${3:-${LIGHT_RELEASE_VERSION:-0.1.0}}"
PRODUCT_NAME="ToskLight Media"
APP="$OUTPUT_DIR/$PRODUCT_NAME.app"
SOURCE_ICON="$ROOT/assets/branding/ToskLight Pixel.png"

[[ "$(uname -s)" == "Darwin" ]] || { echo "error: Media.app can only be built on macOS" >&2; exit 1; }
[[ -f "$BINARY" ]] || { echo "error: no Media binary at $BINARY" >&2; exit 1; }
[[ -f "$SOURCE_ICON" ]] || { echo "error: no Media icon at $SOURCE_ICON" >&2; exit 1; }

source "$ROOT/tools/artifact-paths.sh"
light_init_artifact_paths "$ROOT"
mkdir -p "$OUTPUT_DIR" "$LIGHT_TMP_DIR"
icon_root="$(mktemp -d "$LIGHT_TMP_DIR/media-icon.XXXXXX")"
trap 'rm -rf -- "$icon_root"' EXIT
icons="$icon_root/icons"
(cd "$ROOT" && npm exec -- tauri icon "$SOURCE_ICON" --output "$icons" >/dev/null)
[[ -f "$icons/icon.icns" ]] || { echo "error: Tauri did not generate the Media icns" >&2; exit 1; }

rm -rf -- "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m 0755 "$BINARY" "$APP/Contents/MacOS/$PRODUCT_NAME"
install -m 0644 "$icons/icon.icns" "$APP/Contents/Resources/icon.icns"

cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>$PRODUCT_NAME</string>
  <key>CFBundleExecutable</key><string>$PRODUCT_NAME</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundleIdentifier</key><string>de.tokenet.tosklight.media</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$PRODUCT_NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>10.13</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSLocalNetworkUsageDescription</key><string>ToskLight Media receives Art-Net, sACN, and CITP and serves its administration interface on the local network.</string>
</dict></plist>
PLIST
touch "$APP"
echo "Media application bundled: $APP"
