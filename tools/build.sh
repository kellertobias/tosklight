#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"
light_init_artifact_paths "$ROOT"
UI_DIR="$ROOT/apps/light-desktop"
HARDWARE_DIR="$ROOT/apps/light-hardware-controls"
TARGET_DIR="$CARGO_TARGET_DIR"
DATA_DIR="$LIGHT_DATA_DIR"
FIXTURE_LIBRARY_DIR="$ROOT/assets/fixture-library"
MANUAL_REQUIREMENTS="$ROOT/docs/help/.tooling/requirements.txt"
MANUAL_VENV_DIR="$LIGHT_ARTIFACTS_DIR/cache/manual-venv"
MANUAL_PYTHON="$MANUAL_VENV_DIR/bin/python"
CONTROL_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-control-artifacts.json"
HARDWARE_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-hardware-artifacts.json"
DEV_SERVER_LABEL="de.tokenet.tosklight.dev-server"
CODESAFARI_VERSION="1.0.0"

# This script backs the root package.json scripts; run it through npm rather than directly.
usage() {
  cat <<'EOF'
tools/build.sh is invoked by the root package.json scripts:
  npm run open                 Build debug server and app, stop old instances, and open ToskLight
  npm run manual               Build PDF and deployable HTML manuals from docs/help Markdown
  npm run icons:contact-sheets Refresh Help contact-sheet PNGs from assets/icons SVGs
  npm run pages:generate       Assemble the public site: landing page, manual, and code safari
  npm run pages:serve [PORT]   Serve the assembled public site locally
  npm run codesafari           Run the CodeSafari code tour locally
  npm run bundle               Create self-contained server archives for macOS, Windows, Linux AMD64/ARM64
  npm run bundle:install       Also install and open ToskLight.app in ~/Applications
  npm run migrate-artifacts    Move legacy ./light-data to the canonical development runtime directory
  npm run clean:root           Move unexpected root directories into recoverable artifact storage
  npm run clean:artifacts      Remove generated artifacts while preserving runtime and root-cleanup recovery
  npm run artifact-path NAME   Print a resolved artifact path (for CI and tooling)

Direct subcommands: open | manual | icon-contact-sheets | safari | pages | pages-serve [PORT] | codesafari |
  archive [install] | migrate-artifacts | clean-root | clean-artifacts [runtime PATH] | path NAME
EOF
}

build_manual() {
  ensure_manual_dependencies
  "$MANUAL_PYTHON" "$ROOT/tools/generate_icon_contact_sheets.py"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" LIGHT_MANUAL_KEYCAP_DIR="$LIGHT_MANUAL_ROOT/pdf/.manual-keycaps" \
    "$MANUAL_PYTHON" "$ROOT/tools/build_manual.py" --output "$LIGHT_MANUAL_PDF"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" "$MANUAL_PYTHON" "$ROOT/tools/verify_manual.py" "$LIGHT_MANUAL_PDF"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" "$MANUAL_PYTHON" "$ROOT/tools/build_html_manual.py" \
    --site "$LIGHT_MANUAL_HTML_DIR" --archive "$LIGHT_MANUAL_HTML_ARCHIVE"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" "$MANUAL_PYTHON" "$ROOT/tools/verify_html_manual.py" \
    "$LIGHT_MANUAL_HTML_DIR" \
    "$LIGHT_MANUAL_HTML_ARCHIVE"
}

build_icon_contact_sheets() {
  ensure_manual_dependencies
  "$MANUAL_PYTHON" "$ROOT/tools/generate_icon_contact_sheets.py"
}

build_safari() {
  require node
  require npx
  rm -rf "$LIGHT_SAFARI_DIR"
  mkdir -p "$(dirname "$LIGHT_SAFARI_DIR")"
  npx --yes "@tobisk/codesafari@$CODESAFARI_VERSION" export "$ROOT" --out "$LIGHT_SAFARI_DIR"
  [[ -f "$LIGHT_SAFARI_DIR/index.html" ]] || {
    echo "error: CodeSafari export produced no index.html in $LIGHT_SAFARI_DIR" >&2
    exit 1
  }
  cp "$ROOT/tools/codesafari-overrides.css" "$LIGHT_SAFARI_DIR/codesafari-overrides.css"
  node "$ROOT/tools/inject-codesafari-overrides.mjs" "$LIGHT_SAFARI_DIR/index.html"
  echo "Created $LIGHT_SAFARI_DIR"
}

# Assemble the deployable public site. The landing page sits at the root and links
# into the two generated subdirectories; nothing here reaches outside $LIGHT_PAGES_DIR.
build_pages() {
  if [[ "${LIGHT_REUSE_MANUAL:-0}" == "1" ]]; then
    for required in "$LIGHT_MANUAL_PDF" "$LIGHT_MANUAL_HTML_DIR/index.html" "$LIGHT_MANUAL_HTML_ARCHIVE"; do
      [[ -f "$required" ]] || {
        echo "error: LIGHT_REUSE_MANUAL=1 but the manual artifact is missing: $required" >&2
        exit 1
      }
    done
  else
    build_manual
  fi
  build_safari

  rm -rf "$LIGHT_PAGES_DIR"
  mkdir -p "$LIGHT_PAGES_DIR"
  cp -R "$LIGHT_MANUAL_HTML_DIR/." "$LIGHT_PAGES_DIR/manual"
  cp -R "$LIGHT_SAFARI_DIR/." "$LIGHT_PAGES_DIR/safari"
  cp "$LIGHT_MANUAL_PDF" "$LIGHT_PAGES_DIR/tosklight-manual.pdf"
  cp -R "$ROOT/docs/site/." "$LIGHT_PAGES_DIR/"
  node "$ROOT/tools/semantic-test-docs/cli.mjs" --write \
    --output-dir "$LIGHT_PAGES_DIR/semantic-tests"
  # Same application icon the operator manual renders in its hero and sidebar.
  cp "$ROOT/apps/light-desktop/src-tauri/icons/icon.png" "$LIGHT_PAGES_DIR/icon.png"
  # GitHub Pages otherwise runs the output through Jekyll and drops _-prefixed assets.
  touch "$LIGHT_PAGES_DIR/.nojekyll"

  node "$ROOT/tools/render-landing-page.mjs" "$LIGHT_PAGES_DIR/index.html"

  for required in \
    index.html \
    manual/index.html \
    safari/index.html \
    performance/status.json \
    performance/index.html \
    semantic-tests/semantic-test-catalog.html \
    semantic-tests/semantic-test-catalog.v1.json
  do
    [[ -f "$LIGHT_PAGES_DIR/$required" ]] || {
      echo "error: assembled site is missing $required" >&2
      exit 1
    }
  done
  echo "Created $LIGHT_PAGES_DIR"
}

# Live CodeSafari server for browsing the code tour during development.
run_safari_dev() {
  require npx
  npx --yes "@tobisk/codesafari@$CODESAFARI_VERSION" dev "$ROOT"
}

# Serve the already-assembled public site from a static file server.
serve_pages() {
  require node
  [[ -f "$LIGHT_PAGES_DIR/index.html" ]] || {
    echo "error: no assembled site at $LIGHT_PAGES_DIR; run 'npm run pages:generate' first" >&2
    exit 1
  }
  node "$ROOT/tools/serve-pages.mjs" "$LIGHT_PAGES_DIR" "${1:-8080}"
}

ensure_manual_dependencies() {
  require python3
  [[ -f "$MANUAL_REQUIREMENTS" ]] || {
    echo "error: manual requirements not found: $MANUAL_REQUIREMENTS" >&2
    exit 1
  }

  if [[ ! -x "$MANUAL_PYTHON" ]]; then
    echo "Creating isolated manual build environment..."
    mkdir -p "$(dirname "$MANUAL_VENV_DIR")"
    python3 -m venv "$MANUAL_VENV_DIR"
  fi

  if ! manual_requirements_satisfied; then
    echo "Installing pinned manual build dependencies..."
    "$MANUAL_PYTHON" -m pip install --disable-pip-version-check --requirement "$MANUAL_REQUIREMENTS"
  fi
}

manual_requirements_satisfied() {
  local installed package requirement version

  while IFS= read -r requirement; do
    [[ -z "$requirement" || "$requirement" == \#* ]] && continue
    package="${requirement%%==*}"
    version="${requirement#*==}"
    [[ "$package" != "$requirement" && -n "$package" && -n "$version" ]] || return 1
    installed="$("$MANUAL_PYTHON" -c \
      'from importlib.metadata import version; import sys; print(version(sys.argv[1]))' \
      "$package" 2>/dev/null)" || return 1
    [[ "$installed" == "$version" ]] || return 1
  done < "$MANUAL_REQUIREMENTS"

  "$MANUAL_PYTHON" -m pip check >/dev/null 2>&1
}

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command not found: $1" >&2
    exit 1
  }
}

stop_running() {
  echo "Stopping running Light instances..."
  launchctl remove "$DEV_SERVER_LABEL" 2>/dev/null || true
  pkill -x light-headless 2>/dev/null || true
  pkill -x light-desktop 2>/dev/null || true
  pkill -x ToskLight 2>/dev/null || true
  pkill -x light-hardware-controls 2>/dev/null || true
  pkill -x Light 2>/dev/null || true
  pkill -f "$ROOT/node_modules/.bin/vite" 2>/dev/null || true
  pkill -f "$TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-headless" 2>/dev/null || true
  pkill -f "$TARGET_DIR/debug/light-headless" 2>/dev/null || true
  pkill -f "$TARGET_DIR/debug/light-desktop" 2>/dev/null || true
  pkill -f "$TARGET_DIR/release/light-desktop" 2>/dev/null || true
}

wait_for_endpoint() {
  local attempts=0
  while (( attempts < 100 )); do
    if curl -fsS http://127.0.0.1:5000/api/v2/readiness >/dev/null 2>&1; then return 0; fi
    sleep 0.1
    attempts=$((attempts + 1))
  done
  echo "error: timed out waiting for Light headless; see $DATA_DIR/light-headless.log" >&2
  return 1
}

wait_for_launchd_server() {
  # Matches dev.sh's 60s readiness window; a debug-build server on grown desk data
  # needs well over the former 10s before it binds.
  local attempts=0 details pid command
  while (( attempts < 600 )); do
    details="$(launchctl print "gui/$(id -u)/$DEV_SERVER_LABEL" 2>/dev/null || true)"
    pid="$(sed -n 's/^[[:space:]]*pid = \([0-9][0-9]*\)$/\1/p' <<<"$details" | head -1)"
    if [[ -n "$pid" ]]; then
      command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
      if [[ "$command" == "$TARGET_DIR/debug/light-headless --data-dir $DATA_DIR"* ]] && \
        curl -fsS http://127.0.0.1:5000/api/v2/readiness >/dev/null 2>&1; then
        return 0
      fi
    fi
    sleep 0.1
    attempts=$((attempts + 1))
  done
  echo "error: canonical Light headless process did not own readiness; see $DATA_DIR/light-headless.log" >&2
  launchctl remove "$DEV_SERVER_LABEL" 2>/dev/null || true
  return 1
}

wait_for_server() {
  local pid="$1"
  local attempts=0
  while (( attempts < 100 )); do
    if curl -fsS http://127.0.0.1:5000/api/v2/readiness >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "error: Light headless exited during startup; see $TARGET_DIR/light-headless.log" >&2
      return 1
    fi
    sleep 0.1
    attempts=$((attempts + 1))
  done
  echo "error: timed out waiting for Light headless; see $TARGET_DIR/light-headless.log" >&2
  return 1
}

start_server() {
  local binary="$1"
  mkdir -p "$DATA_DIR" "$TARGET_DIR"
  nohup "$binary" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR" >"$TARGET_DIR/light-headless.log" 2>&1 &
  local pid=$!
  echo "$pid" >"$TARGET_DIR/light-headless.pid"
  wait_for_server "$pid"
}

build_debug_and_open() {
  require cargo
  require npm
  require curl
  require open

  build_icon_contact_sheets
  light_check_runtime_migration
  stop_running
  write_tauri_configs
  echo "Installing workspace dependencies..."
  (cd "$ROOT" && npm ci)
  echo "Building control UI assets for the Light headless bundle..."
  (cd "$UI_DIR" && npm run build)
  echo "Building Light headless for the app bundle..."
  cargo build --manifest-path "$ROOT/Cargo.toml" -p light-headless --bin light-headless
  echo "Building debug Tauri app..."
  (cd "$HARDWARE_DIR" && npm run tauri:build -- --debug --bundles app --config "$HARDWARE_TAURI_CONFIG")
  (cd "$UI_DIR" && npm run tauri:build -- --debug --bundles app --config "$CONTROL_TAURI_CONFIG")
  cp "$TARGET_DIR/debug/light-headless" "$TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-headless"
  echo "Starting development Light headless service..."
  launchctl submit -l "$DEV_SERVER_LABEL" -o "$DATA_DIR/light-headless.log" -e "$DATA_DIR/light-headless.log" -- "$TARGET_DIR/debug/light-headless" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR"
  wait_for_launchd_server
  open "$TARGET_DIR/debug/bundle/macos/ToskLight.app"
  echo "ToskLight is open. Server log: $DATA_DIR/light-headless.log"
}

archive_release() {
  local install="${1:-false}"
  local version app_path hardware_app_path artifact_dir app_zip hardware_app_zip universal_server

  require cargo
  require npm
  require ditto
  require zip
  require lipo
  require rustup
  require cargo-zigbuild
  require zig

  build_icon_contact_sheets
  version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
  artifact_dir="$LIGHT_RELEASE_DIR"
  app_zip="$artifact_dir/tosklight-$version-macos-$(uname -m).zip"
  hardware_app_zip="$artifact_dir/tosklight-hardware-controls-$version-macos-$(uname -m).zip"

  write_tauri_configs
  echo "Installing workspace dependencies..."
  (cd "$ROOT" && npm ci)
  echo "Building release UI for standalone Light headless..."
  (cd "$UI_DIR" && npm run build)
  ensure_rust_target aarch64-apple-darwin
  ensure_rust_target x86_64-apple-darwin
  ensure_rust_target x86_64-pc-windows-gnu
  ensure_rust_target x86_64-unknown-linux-musl
  ensure_rust_target aarch64-unknown-linux-musl

  echo "Building self-contained macOS universal Light headless..."
  cargo build --manifest-path "$ROOT/Cargo.toml" --release --target aarch64-apple-darwin -p light-headless --bin light-headless
  cargo build --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-apple-darwin -p light-headless --bin light-headless
  universal_server="$TARGET_DIR/release/light-headless"
  mkdir -p "$(dirname "$universal_server")"
  lipo -create \
    "$TARGET_DIR/aarch64-apple-darwin/release/light-headless" \
    "$TARGET_DIR/x86_64-apple-darwin/release/light-headless" \
    -output "$universal_server"

  echo "Building self-contained Windows Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-pc-windows-gnu -p light-headless --bin light-headless
  echo "Building self-contained Linux AMD64 Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --no-default-features --target x86_64-unknown-linux-musl -p light-headless --bin light-headless
  echo "Building self-contained Linux ARM64 Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --no-default-features --target aarch64-unknown-linux-musl -p light-headless --bin light-headless

  echo "Building release Tauri app..."
  (cd "$HARDWARE_DIR" && npm run tauri:build -- --bundles app --config "$HARDWARE_TAURI_CONFIG")
  (cd "$UI_DIR" && npm run tauri:build -- --bundles app --config "$CONTROL_TAURI_CONFIG")

  app_path="$TARGET_DIR/release/bundle/macos/ToskLight.app"
  hardware_app_path="$TARGET_DIR/release/bundle/macos/ToskLight Hardware Controls.app"
  cp "$universal_server" "$app_path/Contents/MacOS/light-headless"
  mkdir -p "$artifact_dir"
  archive_binary "$universal_server" "light-headless" "$artifact_dir/light-headless-$version-macos-universal.zip"
  archive_binary "$TARGET_DIR/x86_64-pc-windows-gnu/release/light-headless.exe" "light-headless.exe" "$artifact_dir/light-headless-$version-windows-amd64.zip"
  archive_binary "$TARGET_DIR/x86_64-unknown-linux-musl/release/light-headless" "light-headless" "$artifact_dir/light-headless-$version-linux-amd64.zip"
  archive_binary "$TARGET_DIR/aarch64-unknown-linux-musl/release/light-headless" "light-headless" "$artifact_dir/light-headless-$version-linux-arm64.zip"
  rm -f "$app_zip"
  ditto -c -k --sequesterRsrc --keepParent "$app_path" "$app_zip"
  echo "Created $app_zip"
  rm -f "$hardware_app_zip"
  ditto -c -k --sequesterRsrc --keepParent "$hardware_app_path" "$hardware_app_zip"
  echo "Created $hardware_app_zip"

  if [[ "$install" == true ]]; then
    require open
    stop_running
    mkdir -p "$HOME/Applications"
    rm -rf "$HOME/Applications/ToskLight.app"
    ditto "$app_path" "$HOME/Applications/ToskLight.app"
    rm -rf "$HOME/Applications/ToskLight Hardware Controls.app"
    ditto "$hardware_app_path" "$HOME/Applications/ToskLight Hardware Controls.app"
    open "$HOME/Applications/ToskLight.app"
    echo "Installed and opened $HOME/Applications/ToskLight.app"
  fi
}

ensure_rust_target() {
  local target="$1"
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "error: missing Rust target $target; install it with: rustup target add $target" >&2
    exit 1
  fi
}

archive_binary() {
  local binary="$1"
  local binary_name="$2"
  local archive="$3"
  local archive_name
  local staging

  [[ -f "$binary" ]] || { echo "error: expected build output not found: $binary" >&2; exit 1; }
  [[ -d "$FIXTURE_LIBRARY_DIR" ]] || { echo "error: fixture library not found: $FIXTURE_LIBRARY_DIR" >&2; exit 1; }
  archive_name="${archive##*/}"
  archive_name="${archive_name%.zip}"
  mkdir -p "$LIGHT_TMP_DIR"
  staging="$(mktemp -d "$LIGHT_TMP_DIR/archive.XXXXXX")"
  mkdir -p "$staging/$archive_name"
  cp "$binary" "$staging/$archive_name/$binary_name"
  cp -R "$FIXTURE_LIBRARY_DIR" "$staging/$archive_name/fixture-library"
  rm -f "$archive"
  (cd "$staging" && zip -q -r "$archive" "$archive_name")
  rm -rf "$staging"
  echo "Created $archive"
}

write_tauri_configs() {
  require node
  node "$ROOT/tools/write-tauri-artifact-config.mjs" control "$CONTROL_TAURI_CONFIG"
  node "$ROOT/tools/write-tauri-artifact-config.mjs" hardware "$HARDWARE_TAURI_CONFIG"
}

print_artifact_path() {
  case "$1" in
    root) printf '%s\n' "$LIGHT_ARTIFACTS_DIR" ;;
    cargo) printf '%s\n' "$CARGO_TARGET_DIR" ;;
    manual-pdf) printf '%s\n' "$LIGHT_MANUAL_PDF" ;;
    manual-html) printf '%s\n' "$LIGHT_MANUAL_HTML_ARCHIVE" ;;
    manual-html-dir) printf '%s\n' "$LIGHT_MANUAL_HTML_DIR" ;;
    icon-contact-sheets) printf '%s\n' "$LIGHT_ICON_CONTACT_SHEETS_DIR" ;;
    pages) printf '%s\n' "$LIGHT_PAGES_DIR" ;;
    safari) printf '%s\n' "$LIGHT_SAFARI_DIR" ;;
    release) printf '%s\n' "$LIGHT_RELEASE_DIR" ;;
    runtime) printf '%s\n' "$LIGHT_RUNTIME_DATA_DIR" ;;
    tmp) printf '%s\n' "$LIGHT_TMP_DIR" ;;
    test-results) printf '%s\n' "$LIGHT_TEST_RESULTS_DIR" ;;
    playwright-report) printf '%s\n' "$LIGHT_PLAYWRIGHT_REPORT_DIR" ;;
    visual-inspection) printf '%s\n' "$LIGHT_VISUAL_INSPECTION_DIR" ;;
    storybook) printf '%s\n' "$LIGHT_STORYBOOK_UI_DIR" ;;
    *) echo "error: unknown artifact path: $1" >&2; return 2 ;;
  esac
}

case "${1:-}" in
  icon-contact-sheets)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_icon_contact_sheets
    ;;
  manual)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_manual
    ;;
  safari)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_safari
    ;;
  pages)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_pages
    ;;
  codesafari)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    run_safari_dev
    ;;
  pages-serve)
    [[ $# -le 2 ]] || { usage >&2; exit 2; }
    serve_pages "${2:-}"
    ;;
  open)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_debug_and_open
    ;;
  archive)
    case "${2:-}" in
      "") archive_release false ;;
      install) [[ $# -eq 2 ]] || { usage >&2; exit 2; }; archive_release true ;;
      *) usage >&2; exit 2 ;;
    esac
    ;;
  migrate-artifacts)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    light_migrate_runtime
    ;;
  clean-root)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    light_clean_repository_root
    ;;
  clean|clean-artifacts)
    case "${2:-}" in
      "") [[ $# -eq 1 ]] || { usage >&2; exit 2; }; light_clean_reproducible ;;
      runtime) [[ $# -eq 3 ]] || { usage >&2; exit 2; }; light_clean_runtime "$3" ;;
      *) usage >&2; exit 2 ;;
    esac
    ;;
  path)
    [[ $# -eq 2 ]] || { usage >&2; exit 2; }
    print_artifact_path "$2"
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
