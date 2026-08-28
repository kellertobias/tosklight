#!/usr/bin/env bash
# Apply a fresh ad-hoc signature after downloading the macOS release bundle.
set -euo pipefail

BUNDLE_DIR="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
[[ "$(uname -s)" == "Darwin" ]] || { echo "error: this script is only for macOS" >&2; exit 1; }
[[ -d "$BUNDLE_DIR" ]] || { echo "error: no bundle directory at $BUNDLE_DIR" >&2; exit 1; }

found=0
for app in \
  "$BUNDLE_DIR/tosklight-desk-macos_arm64.app" \
  "$BUNDLE_DIR/tosklight-architect-macos_arm64.app" \
  "$BUNDLE_DIR/tosklight-media-macos_arm64.app"; do
  [[ -d "$app" ]] || continue
  found=1
  echo "Ad-hoc signing $(basename "$app")"
  codesign --force --deep --sign - --timestamp=none "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
done

[[ "$found" == 1 ]] || { echo "error: no ToskLight macOS applications found in $BUNDLE_DIR" >&2; exit 1; }
echo "All ToskLight applications have valid local ad-hoc signatures."
