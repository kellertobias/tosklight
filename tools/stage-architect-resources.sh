#!/usr/bin/env bash
# Stage the canonical local Architect resources before the application is sealed.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: stage-architect-resources.sh APP DEMO_SHOW FIXTURE_LIBRARY" >&2
  exit 2
fi

APP="$1"
DEMO_SHOW="$2"
FIXTURE_LIBRARY="$3"
RESOURCES="$APP/Contents/Resources"

# Validate every required input before changing any bundle resource.
[[ -d "$RESOURCES" ]] || { echo "error: no Architect application resources at $RESOURCES" >&2; exit 1; }
[[ -s "$DEMO_SHOW" ]] || { echo "error: no generated demo show at $DEMO_SHOW" >&2; exit 1; }
[[ -d "$FIXTURE_LIBRARY" ]] || { echo "error: no canonical fixture library at $FIXTURE_LIBRARY" >&2; exit 1; }
[[ -n "$(find "$FIXTURE_LIBRARY" -type f -name '*.toskfixture' -print -quit)" ]] || {
  echo "error: no canonical fixture packages in $FIXTURE_LIBRARY" >&2
  exit 1
}

mkdir -p "$RESOURCES/demo-show"
install -m 0644 "$DEMO_SHOW" "$RESOURCES/demo-show/demo-show.show"
# The local assembler creates a fresh app. Replace only this owned resource tree
# when staging again, so removed packages cannot linger beside the canonical set.
rm -rf -- "$RESOURCES/fixture-library"
mkdir -p "$RESOURCES/fixture-library"
cp -R "$FIXTURE_LIBRARY/." "$RESOURCES/fixture-library/"
