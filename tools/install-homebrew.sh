#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--help" ]]; then
  echo "usage: bash install-homebrew.sh"
  echo "Install ToskLight Launcher, Control, Architect (with visualizer), and Pixel from GitHub Releases."
  echo "Requires Homebrew and an Apple Silicon Mac."
  exit 0
fi
if [[ "$#" != 0 ]]; then
  echo "error: unsupported argument; use --help for usage" >&2
  exit 2
fi
if [[ "$(uname -s)" != Darwin || "$(uname -m)" != arm64 ]]; then
  echo "error: the ToskLight macOS release requires an Apple Silicon Mac." >&2
  echo "Run Terminal natively (outside Rosetta) on an Apple Silicon Mac." >&2
  exit 1
fi
if ! command -v brew >/dev/null 2>&1; then
  echo "error: install Homebrew from https://brew.sh, then run this script again." >&2
  exit 1
fi

package=kellertobias/tosklight/tosklight
echo "Updating the ToskLight Homebrew tap from GitHub..."
brew tap kellertobias/tosklight https://github.com/kellertobias/tosklight.git
brew update
brew trust --cask "$package"
echo "Installing Launcher, Control, Architect with its visualizer, Pixel, and FFmpeg; signing and verifying the apps..."
if brew list --cask "$package" >/dev/null 2>&1; then
  echo "Refreshing the existing installation so local signing runs even at the same version..."
  brew reinstall --cask "$package"
else
  brew install --cask "$package"
fi
echo "ToskLight is installed with verified signatures. Open ToskLight Launcher from Applications to choose Control, Architect, or Pixel."
