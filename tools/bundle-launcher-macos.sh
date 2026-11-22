#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# -ge 1 && $# -le 3 ]] || { echo 'usage: bundle-launcher-macos.sh OUTPUT_DIR [VERSION] [arm64|x86_64]' >&2; exit 2; }
[[ "$(uname -s)" == Darwin ]] || { echo 'error: the launcher requires macOS' >&2; exit 2; }
OUTPUT="$1"
VERSION="${2:-${LIGHT_RELEASE_VERSION:-0.1.0}}"
ARCH="${3:-$(uname -m)}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][A-Za-z0-9.-]+)?$ ]] || { echo 'error: invalid launcher version' >&2; exit 2; }
[[ "$ARCH" == arm64 || "$ARCH" == x86_64 ]] || { echo 'error: launcher architecture must be arm64 or x86_64' >&2; exit 2; }
source "$ROOT/tools/artifact-paths.sh"
light_init_artifact_paths "$ROOT"
mkdir -p "$OUTPUT" "$LIGHT_ARTIFACTS_DIR/cache/swift-modules"
APP="$OUTPUT/ToskLight Launcher.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
echo "Building ToskLight Launcher ($ARCH)…"
xcrun swiftc -O -swift-version 5 -target "$ARCH-apple-macosx11.0" \
  -module-cache-path "$LIGHT_ARTIFACTS_DIR/cache/swift-modules" \
  -framework AppKit "$ROOT/apps/macos-launcher/main.swift" "$ROOT/apps/macos-launcher/SuiteUpdates.swift" \
  -o "$APP/Contents/MacOS/ToskLight Launcher"
xcrun swiftc -O -swift-version 5 -parse-as-library -target "$ARCH-apple-macosx11.0" \
  -module-cache-path "$LIGHT_ARTIFACTS_DIR/cache/swift-modules" \
  -framework AppKit "$ROOT/apps/macos-launcher/InstallUpdate.swift" \
  -o "$APP/Contents/MacOS/tosklight-suite-update"
cp "$ROOT/apps/macos-launcher/products.json" "$APP/Contents/Resources/products.json"
cp "$ROOT/apps/light-desktop/src-tauri/icons/icon.icns" "$APP/Contents/Resources/icon.icns"
cp "$ROOT/LICENSE" "$APP/Contents/Resources/LICENSE"
cp "$ROOT/tools/sign-macos-apps-locally.sh" "$APP/Contents/Resources/sign-macos-apps-locally.sh"
for product in Control Architect Pixel; do
  name="$(printf '%s' "$product" | tr '[:upper:]' '[:lower:]')"
  cp "$ROOT/assets/branding/ToskLight $product.png" "$APP/Contents/Resources/$name.png"
done
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>de.tokenet.tosklight.launcher</string>
  <key>CFBundleName</key><string>ToskLight Launcher</string>
  <key>CFBundleDisplayName</key><string>ToskLight Launcher</string>
  <key>CFBundleExecutable</key><string>ToskLight Launcher</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
bash "$ROOT/tools/seal-macos-app.sh" "$APP"
echo "Built $APP"
