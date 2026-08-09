#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$#" -ne 2 ]]; then
  echo "usage: ./tools/convert-media.sh <input-folder> <output-folder>" >&2
  exit 2
fi

cd "$ROOT"
exec cargo run --release -p media-codec --bin media-convert -- "$1" "$2"
