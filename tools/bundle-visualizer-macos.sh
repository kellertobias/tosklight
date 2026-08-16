#!/usr/bin/env bash
#
# Wrap the standalone visualizer binary in a macOS application bundle.
#
# The visualizer is a plain winit/wgpu process, not a Tauri application, so nothing generates a
# bundle for it. Without one macOS gives it the generic executable icon in the Dock and the
# window's application menu is titled after the binary. This produces the same layout Tauri emits
# for the other applications, so release staging can treat every macOS product the same way.
#
# usage: bundle-visualizer-macos.sh BINARY OUTPUT_DIR [VERSION]

set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: bundle-visualizer-macos.sh BINARY OUTPUT_DIR [VERSION]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$1"
OUTPUT_DIR="$2"
VERSION="${3:-${LIGHT_RELEASE_VERSION:-0.1.0}}"
PRODUCT_NAME="ToskLight PreViz"
IDENTIFIER="de.tokenet.tosklight.visualizer"
ICON="$ROOT/apps/viz-editor/src-tauri/icons/icon.icns"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: macOS application bundles can only be assembled on macOS" >&2
  exit 1
fi
[[ -f "$BINARY" ]] || { echo "error: no visualizer binary at $BINARY" >&2; exit 1; }
# The Viz Editor owns the icon set both products share, so a missing icon means the icon set was
# never generated and a silently unbadged bundle would be worse than a failed build.
[[ -f "$ICON" ]] || { echo "error: no Viz application icon at $ICON" >&2; exit 1; }

APP="$OUTPUT_DIR/$PRODUCT_NAME.app"
rm -rf -- "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

install -m 0755 "$BINARY" "$APP/Contents/MacOS/$PRODUCT_NAME"
install -m 0644 "$ICON" "$APP/Contents/Resources/icon.icns"

cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>$PRODUCT_NAME</string>
	<key>CFBundleExecutable</key>
	<string>$PRODUCT_NAME</string>
	<key>CFBundleIconFile</key>
	<string>icon.icns</string>
	<key>CFBundleIdentifier</key>
	<string>$IDENTIFIER</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>$PRODUCT_NAME</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.13</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key>
	<true/>
	<key>NSLocalNetworkUsageDescription</key>
	<string>The visualizer reads the rig from a ToskLight desk and receives Art-Net and sACN on the local network.</string>
</dict>
</plist>
PLIST

# Nothing here is signed, so the freshly written bundle keeps whatever quarantine state the build
# left behind. Touching it makes Finder pick the icon up instead of showing a stale cached one.
touch "$APP"

echo "Visualizer bundled: $APP"
