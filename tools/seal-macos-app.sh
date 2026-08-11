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

# Rust's linker gives each Mach-O an ad-hoc executable signature, but that does not seal the
# enclosing application resources. Gatekeeper treats that half-signed state as damaged instead
# of offering the normal approval path for an unsigned development build. Sign the completed
# bundle, including every nested helper, only after its final resources have been staged.
codesign --force --deep --sign - --timestamp=none "$app"
codesign --verify --deep --strict --verbose=2 "$app"
