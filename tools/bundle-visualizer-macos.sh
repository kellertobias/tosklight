#!/usr/bin/env bash
#
# Assemble the ToskLight Architect macOS application bundle.
#
# Architect is one product with two executables: the Rig Editor an operator launches, and the
# renderer it owns and starts as an accessory. The editor is the bundle's executable, so opening
# the application opens the editor — the renderer is a helper beside it, found by name, and never
# the thing a double-click runs.
#
# The renderer is a plain winit/wgpu process rather than a Tauri application, so nothing generates
# a bundle for it and this assembles the layout by hand instead. Release staging can then treat
# every macOS product the same way.
#
# usage: bundle-visualizer-macos.sh EDITOR_BINARY RENDERER_BINARY OUTPUT_DIR [VERSION]

set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "usage: bundle-visualizer-macos.sh EDITOR_BINARY RENDERER_BINARY OUTPUT_DIR [VERSION]" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EDITOR_BINARY="$1"
RENDERER_BINARY="$2"
OUTPUT_DIR="$3"
VERSION="${4:-${LIGHT_RELEASE_VERSION:-0.1.0}}"
PRODUCT_NAME="ToskLight Architect"
IDENTIFIER="de.tokenet.tosklight.visualizer"
ICON="$ROOT/apps/viz-editor/src-tauri/icons/icon.icns"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: macOS application bundles can only be assembled on macOS" >&2
  exit 1
fi
[[ -f "$EDITOR_BINARY" ]] || { echo "error: no rig editor binary at $EDITOR_BINARY" >&2; exit 1; }
[[ -f "$RENDERER_BINARY" ]] || { echo "error: no renderer binary at $RENDERER_BINARY" >&2; exit 1; }
# The Viz Editor owns the icon set both products share, so a missing icon means the icon set was
# never generated and a silently unbadged bundle would be worse than a failed build.
[[ -f "$ICON" ]] || { echo "error: no Viz application icon at $ICON" >&2; exit 1; }

APP="$OUTPUT_DIR/$PRODUCT_NAME.app"
rm -rf -- "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# The editor is the executable, so launching the application opens the Rig Editor. The renderer
# keeps its own file name: the editor looks for it by that name beside itself, and naming it after
# the product would make the editor find itself instead.
install -m 0755 "$EDITOR_BINARY" "$APP/Contents/MacOS/$PRODUCT_NAME"
install -m 0755 "$RENDERER_BINARY" "$APP/Contents/MacOS/viz-renderer"
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

echo "Architect bundled: $APP"
