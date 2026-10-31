#!/usr/bin/env bash
set -euo pipefail

app="${1:-}"
if [[ -z "$app" ]]; then
  echo "usage: seal-macos-app.sh APPLICATION.app" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: macOS application bundles can only be sealed on macOS" >&2
  exit 2
fi
if [[ ! -d "$app" || ! -f "$app/Contents/Info.plist" ]]; then
  echo "error: not a macOS application bundle: $app" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
identity="${LIGHT_MACOS_SIGNING_IDENTITY:--}"
options=(--force --sign "$identity")
if [[ "$identity" == - ]]; then
  options+=(--timestamp=none)
else
  [[ "$identity" == 'Developer ID Application: '* ]] || {
    echo 'error: release signing requires a Developer ID Application identity' >&2; exit 1;
  }
  options+=(--timestamp --options runtime --entitlements "$ROOT/tools/macos-audio.entitlements.plist")
  if [[ -n "${LIGHT_MACOS_KEYCHAIN:-}" ]]; then options+=(--keychain "$LIGHT_MACOS_KEYCHAIN"); fi
fi
# Sign code before its enclosing bundle. Resources must already be final at this point.
while IFS= read -r -d '' binary; do
  if file -b "$binary" | grep -q 'Mach-O'; then
    codesign "${options[@]}" "$binary"
  fi
done < <(find "$app/Contents" -type f -print0)
while IFS= read -r -d '' nested; do
  codesign "${options[@]}" "$nested"
done < <(find "$app/Contents" -depth -type d \( -name '*.framework' -o -name '*.app' -o -name '*.xpc' \) -print0)
codesign "${options[@]}" "$app"
codesign --verify --deep --strict --verbose=2 "$app"
