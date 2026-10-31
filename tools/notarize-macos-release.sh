#!/usr/bin/env bash
# Notarize the complete download, then staple and repack every app-bearing component archive.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
light_init_artifact_paths "$ROOT"
[[ "${LIGHT_MACOS_SIGNING_MODE:-adhoc}" == developer-id ]] || { echo 'Ad-hoc release: notarization is not configured.'; exit 0; }
directory="${1:?usage: notarize-macos-release.sh RELEASE_DIRECTORY}"
directory="$(cd "$directory" && pwd)"
: "${APPLE_ID:?}" "${APPLE_TEAM_ID:?}" "${APPLE_APP_SPECIFIC_PASSWORD:?}"
auth=(--apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD")
mkdir -p "$LIGHT_TMP_DIR"
stage="$(mktemp -d "$LIGHT_TMP_DIR/notarization.XXXXXX")"
trap 'rm -rf -- "$stage"' EXIT
archive="$directory/tosklight-bundle-macos_arm64.zip"
[[ -f "$archive" ]] || { echo 'error: complete macOS release archive is missing' >&2; exit 1; }
receipt="$directory/macos-notarization.json"
xcrun notarytool submit "$archive" "${auth[@]}" --wait --timeout 60m --output-format json > "$receipt"
if ! python3 - "$receipt" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
if r.get('status') != 'Accepted':
    print('error: Apple notarization was not accepted:', r.get('status'), file=sys.stderr)
    sys.exit(1)
PY
then
  submission="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$receipt")"
  xcrun notarytool log "$submission" "${auth[@]}" "$directory/macos-notarization-log.json" || true
  exit 1
fi
for archive in "$directory"/*macos-arm64.zip "$directory/tosklight-bundle-macos_arm64.zip"; do
  [[ -f "$archive" ]] || continue
  unpack="$stage/unpacked"
  mkdir "$unpack"
  ditto -x -k "$archive" "$unpack"
  found=0
  while IFS= read -r -d '' app; do
    found=1
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    codesign --verify --deep --strict "$app"
    spctl --assess --type execute --verbose=2 "$app"
  done < <(find "$unpack" -type d -name '*.app' -prune -print0)
  if [[ "$found" == 1 ]]; then
    roots=("$unpack"/*)
    [[ "${#roots[@]}" == 1 ]] || { echo 'error: archive must contain one release root' >&2; exit 1; }
    ditto -c -k --sequesterRsrc --keepParent "${roots[0]}" "$stage/stapled.zip"
    mv "$stage/stapled.zip" "$archive"
  fi
  rm -rf -- "$unpack"
done
