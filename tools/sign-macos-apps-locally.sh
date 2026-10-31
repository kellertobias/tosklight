#!/usr/bin/env bash
# Optional local ad-hoc signing for downloaded ToskLight testing builds.
# This script is self-contained and deliberately leaves download quarantine intact.
set -euo pipefail

fail() { echo "error: $*" >&2; exit 1; }
[[ "$(uname -s)" == "Darwin" ]] || fail "this script is only for macOS"
command -v codesign >/dev/null 2>&1 || fail "codesign is unavailable. Install Apple's Command Line Tools with: xcode-select --install"
[[ $# -le 1 ]] || fail "usage: sign-macos-apps-locally.sh [APPLICATION.app | DIRECTORY]"
input="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
[[ ! -L "$input" ]] || fail "refusing a symbolic link: $input"
[[ -d "$input" ]] || fail "no application or directory at $input"
root="$(cd "$input" && pwd -P)"
apps=()
if [[ "$input" == *.app || "$input" == *.app/ ]]; then
  apps+=("$root")
else
  while IFS= read -r -d '' app; do apps+=("$app"); done < <(find "$root" -mindepth 1 -maxdepth 1 -name '*.app' -print0)
fi
[[ ${#apps[@]} -gt 0 ]] || fail "no applications found in $root"

# Validate every candidate before changing any bundle. Known identifiers also cover individually
# downloaded applications and the renamed applications in the complete suite archive.
for app in "${apps[@]}"; do
  [[ ! -L "$app" ]] || fail "refusing a symbolic link: $app"
  [[ "$app" == "$root" || "$app" == "$root/"* ]] || fail "application is outside the selected directory: $app"
  [[ -z "$(find "$app" -type l -print -quit)" ]] || fail "refusing a bundle containing symbolic links: $app"
  identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist" 2>/dev/null)" || fail "cannot read the bundle identifier: $app"
  case "$identifier" in
    de.tokenet.light|de.tokenet.tosklight.visualizer|de.tokenet.tosklight.media|de.tokenet.tosklight.hardware-controls) ;;
    *) fail "not a recognized ToskLight application ($identifier): $app" ;;
  esac
  signature="$(codesign --display --verbose=4 "$app" 2>&1 || true)"
  if [[ "$signature" == *'Authority=Developer ID Application:'* ]]; then
    codesign --verify --deep --strict --verbose=2 "$app" || fail "the Developer ID signature is invalid; download a fresh release: $app"
  fi
done

for app in "${apps[@]}"; do
  signature="$(codesign --display --verbose=4 "$app" 2>&1 || true)"
  if [[ "$signature" == *'Authority=Developer ID Application:'* ]]; then
    echo "Keeping the valid Developer ID signature: $(basename "$app")"
    continue
  fi
  echo "Ad-hoc signing $(basename "$app")"
  # Sign helper executables and libraries before their enclosing bundle. Do not rely on --deep
  # signing to discover executable files outside Apple's standard nested-code locations.
  while IFS= read -r -d '' executable; do
    if [[ "$(/usr/bin/file -b "$executable")" == *Mach-O* ]]; then
      codesign --force --sign - --timestamp=none "$executable"
    fi
  done < <(find "$app" -depth -type f -print0)
  while IFS= read -r -d '' nested; do
    [[ "$nested" == "$app" ]] || codesign --force --sign - --timestamp=none "$nested"
  done < <(find "$app" -depth -type d \( -name '*.app' -o -name '*.framework' -o -name '*.xpc' \) -print0)
  codesign --force --sign - --timestamp=none "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
done

echo "ToskLight signatures verified. Download quarantine is unchanged."
echo "For an ad-hoc testing build: try opening the app, then use System Settings > Privacy & Security > Open Anyway if macOS blocks it."
