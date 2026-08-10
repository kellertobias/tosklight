#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SLUG="${1:-}"
VERSION="${2:-}"
COMPONENTS="${3:-}"
OUTPUT="${4:-}"

if [[ -z "$SLUG" || -z "$VERSION" || -z "$COMPONENTS" || -z "$OUTPUT" ]]; then
  echo "usage: assemble-release-bundle.sh SLUG VERSION COMPONENTS_DIR OUTPUT_DIR" >&2
  exit 2
fi

source "$ROOT/tools/artifact-paths.sh"
light_init_artifact_paths "$ROOT"
mkdir -p "$OUTPUT" "$LIGHT_TMP_DIR"
COMPONENTS="$(cd "$COMPONENTS" && pwd)"
OUTPUT="$(cd "$OUTPUT" && pwd)"
stage_root="$(mktemp -d "$LIGHT_TMP_DIR/release-bundle.XXXXXX")"
trap 'rm -rf -- "$stage_root"' EXIT

asset_slug="${SLUG//-/_}"
bundle="$stage_root/tosklight-bundle-$asset_slug"
mkdir -p "$bundle"

extract_archive() {
  local archive="$1"
  local destination="$2"
  mkdir -p "$destination"
  if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
    7z x -y "-o$destination" "$archive" >/dev/null
  elif [[ "$SLUG" == "macos-arm64" ]]; then
    ditto -x -k "$archive" "$destination"
  else
    unzip -q "$archive" -d "$destination"
  fi
}

single_directory() {
  local directory="$1"
  find "$directory" -mindepth 1 -maxdepth 1 -type d -print -quit
}

headless_unpack="$stage_root/headless"
extract_archive "$COMPONENTS/light-headless-$VERSION-$SLUG.zip" "$headless_unpack"
headless="$bundle/tosklight-headless-$asset_slug"
mv "$(single_directory "$headless_unpack")" "$headless"
if [[ -f "$headless/light-headless.exe" ]]; then
  mv "$headless/light-headless.exe" "$headless/tosklight-headless-$asset_slug.exe"
else
  mv "$headless/light-headless" "$headless/tosklight-headless-$asset_slug"
fi

media_unpack="$stage_root/media"
extract_archive "$COMPONENTS/tosklight-media-$VERSION-$SLUG.zip" "$media_unpack"
media="$bundle/tosklight-media-$asset_slug"
mv "$(single_directory "$media_unpack")" "$media"
if [[ -f "$media/media-server.exe" ]]; then
  mv "$media/media-server.exe" "$media/tosklight-media-$asset_slug.exe"
else
  mv "$media/media-server" "$media/tosklight-media-$asset_slug"
fi

case "$SLUG" in
  macos-arm64)
    desk_unpack="$stage_root/desk"
    extract_archive "$COMPONENTS/tosklight-$VERSION-macos-arm64.zip" "$desk_unpack"
    mv "$desk_unpack/ToskLight.app" "$bundle/tosklight-desk-$asset_slug.app"

    previz_unpack="$stage_root/previz"
    extract_archive "$COMPONENTS/tosklight-viz-$VERSION-macos-arm64.zip" "$previz_unpack"
    mv "$previz_unpack/ToskLight Viz Editor.app" "$bundle/tosklight-previz-$asset_slug.app"
    ;;
  windows-amd64)
    cp "$COMPONENTS/tosklight-$VERSION-windows-amd64-setup.exe" \
      "$bundle/tosklight-desk-$asset_slug-setup.exe"
    previz_unpack="$stage_root/previz"
    extract_archive \
      "$COMPONENTS/tosklight-visualizer-$VERSION-windows-amd64.zip" "$previz_unpack"
    previz="$bundle/tosklight-previz-$asset_slug"
    mv "$(single_directory "$previz_unpack")" "$previz"
    mv "$previz/viz-editor.exe" "$previz/tosklight-previz-$asset_slug.exe"
    cp -R "$headless/fixture-library" "$previz/fixture-library"
    mkdir -p "$previz/demo-show"
    cp "$COMPONENTS/demo.show" "$previz/demo-show/demo.show"
    ;;
  linux-amd64)
    cp "$COMPONENTS/tosklight-$VERSION-linux-amd64.AppImage" \
      "$bundle/tosklight-desk-$asset_slug.AppImage"
    cp "$COMPONENTS/tosklight-$VERSION-linux-amd64.deb" \
      "$bundle/tosklight-desk-$asset_slug.deb"
    previz_unpack="$stage_root/previz"
    extract_archive \
      "$COMPONENTS/tosklight-visualizer-$VERSION-linux-amd64.zip" "$previz_unpack"
    previz="$bundle/tosklight-previz-$asset_slug"
    mv "$(single_directory "$previz_unpack")" "$previz"
    mv "$previz/viz-editor" "$previz/tosklight-previz-$asset_slug"
    cp -R "$headless/fixture-library" "$previz/fixture-library"
    mkdir -p "$previz/demo-show"
    cp "$COMPONENTS/demo.show" "$previz/demo-show/demo.show"
    ;;
  linux-arm64) ;;
  *)
    echo "error: unsupported release bundle slug: $SLUG" >&2
    exit 2
    ;;
esac

archive="$OUTPUT/tosklight-bundle-$asset_slug.zip"
if [[ "$SLUG" == "macos-arm64" ]]; then
  ditto -c -k --sequesterRsrc --keepParent "$bundle" "$archive"
elif [[ "${RUNNER_OS:-}" == "Windows" ]]; then
  (cd "$stage_root" && 7z a -tzip "$archive" "$(basename "$bundle")" >/dev/null)
else
  (cd "$stage_root" && zip -q -r "$archive" "$(basename "$bundle")")
fi
