#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"
source "$ROOT/tools/cargo-command-lock.sh"
light_init_artifact_paths "$ROOT"
UI_DIR="$ROOT/apps/light-desktop"
HARDWARE_DIR="$ROOT/apps/light-hardware-controls"
TARGET_DIR="$CARGO_TARGET_DIR"
DATA_DIR="$LIGHT_DATA_DIR"
FIXTURE_LIBRARY_DIR="$ROOT/assets/fixture-library"
MANUAL_REQUIREMENTS="$ROOT/docs/help/.tooling/requirements.txt"
MANUAL_VENV_DIR="$LIGHT_ARTIFACTS_DIR/cache/manual-venv"
MANUAL_PYTHON="$MANUAL_VENV_DIR/bin/python"
MANUAL_CONFIG="$ROOT/docs/help/manual.config.json"
MANUAL_RENDERER_PACKAGE="${LIGHT_MANUAL_RENDERER_PACKAGE:-@tobisk/markdown-manuals@1.3.4}"
CONTROL_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-control-artifacts.json"
HARDWARE_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-hardware-artifacts.json"
DEV_SERVER_LABEL="de.tokenet.tosklight.dev-server"
CODESAFARI_VERSION="1.0.0"

# This script backs the root package.json scripts; run it through npm rather than directly.
usage() {
  cat <<'EOF'
tools/build.sh is invoked by the root package.json scripts:
  npm run open                 Open the latest debug ToskLight build without rebuilding
  npm run build:open           Build debug server and app, stop old instances, and open ToskLight
  npm run open:hardware-controls
                               Open the latest Hardware Controls build without rebuilding
  npm run build:hardware-controls:open
                               Build and open Hardware Controls
  npm run open:media [ARGS...] Open the latest Media Server build on its monitors
  npm run build:media:open     Build and open the Media Server
  npm run build:media          Build the Media Server only
  npm run open:viz [ARGS...]   Open the latest visualizer build with the latest Viz editor
  npm run build:viz:open       Build and open the visualizer with the Viz editor
  npm run build:viz            Build the standalone visualizer only
  npm run open:viz-editor      Open the latest Viz rig-planning editor build
  npm run build:viz-editor:open
                               Build and open the Viz rig-planning editor
  npm run build:viz-editor     Build the Viz rig-planning editor only
  npm run demo-show            Generate the canonical demo show from the shipped fixture packages
  npm run demo-capture         Render demo-show frames with the native core, with no window
  npm run manual               Build PDF and deployable HTML manuals from docs/help Markdown
  npm run icons:contact-sheets Refresh Help contact-sheet PNGs from assets/icons SVGs
  npm run models               Rebuild assets/models GLBs with Blender and check the import contract
  npm run models:verify        Check the shipped assets/models GLBs without rebuilding them
  npm run models:render        Render one PNG per model and regenerate the help catalogue
  npm run models:open          Rebuild the models and open the whole set as one .blend in Blender
  npm run pages:generate       Assemble the public site: landing page, manual, Storybook, and code safari
  npm run pages:serve [PORT]   Serve the assembled public site locally
  npm run codesafari           Run the CodeSafari code tour locally
  npm run bundle               Create self-contained server archives for macOS, Windows, Linux AMD64/ARM64
  npm run bundle:install       Also install and open ToskLight.app in ~/Applications
  npm run migrate-artifacts    Move legacy ./light-data to the canonical development runtime directory
  npm run clean:root           Move unexpected root directories into recoverable artifact storage
  npm run clean:artifacts      Remove generated artifacts while preserving runtime and root-cleanup recovery
  npm run clean:cargo-incremental
                               Remove only stale Cargo incremental objects
  npm run artifact-path NAME   Print a resolved artifact path (for CI and tooling)

Direct subcommands: open | build-open | open-hardware-controls | build-hardware-controls-open | open-media [ARGS...] | build-media-open [ARGS...] | build-media | open-viz [ARGS...] | build-viz-open [ARGS...] | build-viz | open-viz-editor [ARGS...] | build-viz-editor-open [ARGS...] | build-viz-editor | demo-show | demo-capture | manual | icon-contact-sheets | models [verify|render|open] | safari | pages | pages-serve [PORT] | codesafari |
  archive [install] | migrate-artifacts | clean-root | clean-cargo-incremental | clean-artifacts [runtime PATH] | path NAME
EOF
}

build_manual() {
  ensure_manual_dependencies
  generate_attribute_reference
  "$MANUAL_PYTHON" "$ROOT/tools/generate_icon_contact_sheets.py"
  run_manual_renderer build \
    --source "$ROOT/docs/help" \
    --config "$MANUAL_CONFIG" \
    --allowed-env-vars LIGHT_MANUAL_VERSION \
    --html-dir "$LIGHT_MANUAL_HTML_DIR" \
    --html-archive "$LIGHT_MANUAL_HTML_ARCHIVE" \
    --pdf "$LIGHT_MANUAL_PDF"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" "$MANUAL_PYTHON" "$ROOT/tools/verify_manual.py" "$LIGHT_MANUAL_PDF"
  PYTHONPATH="${LIGHT_MANUAL_PYTHONPATH:-}" "$MANUAL_PYTHON" "$ROOT/tools/verify_html_manual.py" \
    "$LIGHT_MANUAL_HTML_DIR" \
    "$LIGHT_MANUAL_HTML_ARCHIVE"
}

generate_attribute_reference() {
  light_with_cargo_command_lock "generate attribute reference" \
    cargo run --locked --quiet --package light-core --example generate_attribute_reference -- \
      "$ROOT/docs/help/99-Appendix/02-default-attributes.md"
}

run_manual_renderer() {
  require npx
  require node
  LIGHT_MANUAL_VERSION="${LIGHT_MANUAL_VERSION:-$(node -p "require(process.argv[1]).version" "$ROOT/package.json")}" \
    npx --yes --package "$MANUAL_RENDERER_PACKAGE" markdown-manual "$@"
}

build_icon_contact_sheets() {
  ensure_manual_dependencies
  "$MANUAL_PYTHON" "$ROOT/tools/generate_icon_contact_sheets.py"
}

# The fixture, truss and stage GLBs of docs/engineering/fixture-and-stage-model-brief.md.
# The models are tracked assets, so this only runs when they are being changed.
build_stage_models() {
  require blender
  require python3
  blender --background --factory-startup --python "$ROOT/tools/build_stage_models.py" -- \
    --output "$ROOT/assets/models"
  verify_stage_models
}

verify_stage_models() {
  require python3
  python3 "$ROOT/tools/verify_stage_models.py" --models "$ROOT/assets/models" --quiet
}

# One PNG per shipped model, plus the generated help catalogue that shows them. The renders
# come from importing the .glb, so this doubles as a round-trip check of what actually ships.
render_stage_models() {
  require blender
  ensure_manual_dependencies
  blender --background --factory-startup --python "$ROOT/tools/render_stage_models.py" -- \
    --models "$ROOT/assets/models" \
    --images "$ROOT/docs/help/assets/models" \
    --page "$ROOT/docs/help/99-Appendix/01-model-catalogue.md"
  python3 "$ROOT/tools/generate_model_catalogue.py" \
    --models "$ROOT/assets/models" \
    --images "$ROOT/docs/help/assets/models" \
    --page "$ROOT/docs/help/99-Appendix/01-model-catalogue.md"
  "$MANUAL_PYTHON" "$ROOT/tools/optimise_model_images.py" "$ROOT/docs/help/assets/models"
}

# A .blend of the whole set laid out, to look at the geometry the builders produce. It is a
# scratch review file, not a source of truth: tools/stage_models/ is where the models are edited.
open_stage_models() {
  require blender
  local review="$LIGHT_TMP_DIR/stage-models-review.blend"
  rm -f "$review"
  blender --background --factory-startup --python "$ROOT/tools/build_stage_models.py" -- \
    --output "$ROOT/assets/models" --blend "$review"
  [[ -f "$review" ]] || {
    echo "error: the review file was not written; see the Blender output above" >&2
    exit 1
  }
  verify_stage_models
  echo "Opening $review"
  blender "$review" >/dev/null 2>&1 &
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
  if [[ ! -f "$LIGHT_STORYBOOK_UI_DIR/index.html" ]]; then
    npm run storybook:build
  fi
  build_safari

  rm -rf "$LIGHT_PAGES_DIR"
  mkdir -p "$LIGHT_PAGES_DIR"
  cp -R "$LIGHT_MANUAL_HTML_DIR/." "$LIGHT_PAGES_DIR/manual"
  cp -R "$LIGHT_STORYBOOK_UI_DIR/." "$LIGHT_PAGES_DIR/storybook"
  cp -R "$LIGHT_SAFARI_DIR/." "$LIGHT_PAGES_DIR/safari"
  cp "$LIGHT_MANUAL_PDF" "$LIGHT_PAGES_DIR/tosklight-manual.pdf"
  cp "$ROOT/LICENSE" "$LIGHT_PAGES_DIR/license.txt"
  node "$ROOT/tools/generate-third-party-licenses.mjs" "$LIGHT_PAGES_DIR/third-party-licenses.html"
  cp -R "$ROOT/docs/site/." "$LIGHT_PAGES_DIR/"
  node "$ROOT/tools/semantic-test-docs/cli.mjs" --write \
    --output-dir "$LIGHT_PAGES_DIR/semantic-tests"
  # Publish the full-resolution application artwork, including the approved Apple-style effects.
  cp "$ROOT/assets/branding/ToskLight Control.png" "$LIGHT_PAGES_DIR/icon.png"
  # GitHub Pages otherwise runs the output through Jekyll and drops _-prefixed assets.
  touch "$LIGHT_PAGES_DIR/.nojekyll"

  node "$ROOT/tools/render-landing-page.mjs" "$LIGHT_PAGES_DIR/index.html"

  for required in \
    index.html \
    manual/index.html \
    storybook/index.html \
    safari/index.html \
    performance/status.json \
    performance/index.html \
    semantic-tests/semantic-test-catalog.html \
    semantic-tests/semantic-test-catalog.v1.json \
    license.txt \
    third-party-licenses.html
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
  configure_manual_native_dependencies
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

configure_manual_native_dependencies() {
  local cairo_prefix

  [[ "$(uname -s)" == "Darwin" ]] || return 0
  command -v brew >/dev/null 2>&1 || return 0
  cairo_prefix="$(brew --prefix cairo 2>/dev/null)" || return 0
  [[ -d "$cairo_prefix/lib" ]] || return 0
  export DYLD_LIBRARY_PATH="$cairo_prefix/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
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

require_built_file() {
  local artifact="$1"
  local build_command="$2"
  [[ -x "$artifact" ]] || {
    echo "error: no runnable build found at $artifact" >&2
    echo "Build it first with: $build_command" >&2
    exit 1
  }
}

require_built_app() {
  local artifact="$1"
  local build_command="$2"
  [[ -d "$artifact" ]] || {
    echo "error: no application build found at $artifact" >&2
    echo "Build it first with: $build_command" >&2
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

open_debug_app() {
  require curl
  require open

  local app="$TARGET_DIR/debug/bundle/macos/ToskLight.app"
  local server="$TARGET_DIR/debug/light-headless"
  require_built_app "$app" "npm run build:open"
  require_built_file "$server" "npm run build:open"
  light_check_runtime_migration

  if curl -fsS http://127.0.0.1:5000/api/v2/readiness >/dev/null 2>&1; then
    echo "Using the Light server already answering on http://127.0.0.1:5000"
  else
    echo "Starting the latest built Light headless service..."
    launchctl remove "$DEV_SERVER_LABEL" 2>/dev/null || true
    launchctl submit -l "$DEV_SERVER_LABEL" -o "$DATA_DIR/light-headless.log" -e "$DATA_DIR/light-headless.log" -- "$server" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR"
    wait_for_launchd_server
  fi
  open "$app"
  echo "ToskLight is open. Server log: $DATA_DIR/light-headless.log"
}

build_debug_and_open() {
  require node
  require cargo
  require npm
  require curl
  require open

  light_check_runtime_migration
  stop_running
  write_tauri_configs
  node "$ROOT/tools/ensure-workspace-dependencies.mjs"
  node "$ROOT/tools/ensure-control-frontend.mjs"
  light_with_cargo_command_lock "npm run build:open" build_debug_app_bundle
  cp "$TARGET_DIR/debug/light-headless" "$TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-headless"
  # The visualizer ships inside the desk, beside it, because the desk supervises it as a helper
  # rather than launching a second application. It is built here rather than assumed present: a
  # desk whose Open Visualizer cannot find its helper is a menu item that only ever fails.
  cargo build --manifest-path "$ROOT/Cargo.toml" -p viz-renderer --bin viz-renderer
  cp "$TARGET_DIR/debug/viz-renderer" "$TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS/viz-renderer"
  echo "Starting development Light headless service..."
  launchctl submit -l "$DEV_SERVER_LABEL" -o "$DATA_DIR/light-headless.log" -e "$DATA_DIR/light-headless.log" -- "$TARGET_DIR/debug/light-headless" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR"
  wait_for_launchd_server
  open "$TARGET_DIR/debug/bundle/macos/ToskLight.app"
  echo "ToskLight is open. Server log: $DATA_DIR/light-headless.log"
}

open_hardware_controls() {
  require open
  local app="$TARGET_DIR/debug/bundle/macos/ToskLight Hardware Controls.app"
  require_built_app "$app" "npm run build:hardware-controls:open"
  open "$app"
  echo "ToskLight Hardware Controls is open."
}

build_hardware_controls_and_open() {
  require node
  require npm
  write_tauri_configs
  node "$ROOT/tools/ensure-workspace-dependencies.mjs"
  echo "Building Hardware Controls frontend..."
  (cd "$HARDWARE_DIR" && npm run build)
  echo "Building debug Hardware Controls app..."
  (cd "$HARDWARE_DIR" && npm run tauri:build -- --debug --bundles app --config "$HARDWARE_TAURI_CONFIG")
  open_hardware_controls
}

build_debug_app_bundle() {
  echo "Building Light headless for the app bundle..."
  cargo build --manifest-path "$ROOT/Cargo.toml" -p light-headless --bin light-headless
  echo "Building debug Tauri app..."
  (cd "$UI_DIR" && npm run tauri:build -- --debug --bundles app --config "$CONTROL_TAURI_CONFIG")
}

archive_release_locked() {
  local install="${1:-false}"
  local version app_path hardware_app_path artifact_dir app_zip hardware_app_zip universal_server
  local universal_renderer

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

  # The desk looks for the renderer beside its own executable and starts nothing if it is absent,
  # so a bundle without it opens with "This screen cannot draw a Stage" on every Stage pane.
  echo "Building self-contained macOS universal visualizer..."
  cargo build --manifest-path "$ROOT/Cargo.toml" --release --target aarch64-apple-darwin -p viz-renderer
  cargo build --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-apple-darwin -p viz-renderer
  universal_renderer="$TARGET_DIR/release/viz-renderer"
  lipo -create \
    "$TARGET_DIR/aarch64-apple-darwin/release/viz-renderer" \
    "$TARGET_DIR/x86_64-apple-darwin/release/viz-renderer" \
    -output "$universal_renderer"

  echo "Building self-contained Windows Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --target x86_64-pc-windows-gnu -p light-headless --bin light-headless
  echo "Building self-contained Linux AMD64 Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --no-default-features --target x86_64-unknown-linux-musl -p light-headless --bin light-headless
  echo "Building self-contained Linux ARM64 Light headless..."
  cargo zigbuild --manifest-path "$ROOT/Cargo.toml" --release --no-default-features --target aarch64-unknown-linux-musl -p light-headless --bin light-headless

  echo "Building release Tauri app..."
  (cd "$HARDWARE_DIR" && npm run build)
  (cd "$HARDWARE_DIR" && npm run tauri:build -- --bundles app --config "$HARDWARE_TAURI_CONFIG")
  (cd "$UI_DIR" && npm run tauri:build -- --bundles app --config "$CONTROL_TAURI_CONFIG")

  app_path="$TARGET_DIR/release/bundle/macos/ToskLight.app"
  hardware_app_path="$TARGET_DIR/release/bundle/macos/ToskLight Hardware Controls.app"
  cp "$universal_server" "$app_path/Contents/MacOS/light-headless"
  cp "$universal_renderer" "$app_path/Contents/MacOS/viz-renderer"
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

archive_release() {
  light_with_cargo_command_lock "npm run bundle" archive_release_locked "$@"
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
    demo-show) printf '%s\n' "$LIGHT_DEMO_SHOW_DIR" ;;
    pages) printf '%s\n' "$LIGHT_PAGES_DIR" ;;
    safari) printf '%s\n' "$LIGHT_SAFARI_DIR" ;;
    release) printf '%s\n' "$LIGHT_RELEASE_DIR" ;;
    performance) printf '%s\n' "$LIGHT_PERFORMANCE_DIR" ;;
    runtime) printf '%s\n' "$LIGHT_RUNTIME_DATA_DIR" ;;
    extensions) printf '%s\n' "$LIGHT_RUNTIME_EXTENSIONS_DIR" ;;
    tmp) printf '%s\n' "$LIGHT_TMP_DIR" ;;
    test-results) printf '%s\n' "$LIGHT_TEST_RESULTS_DIR" ;;
    playwright-report) printf '%s\n' "$LIGHT_PLAYWRIGHT_REPORT_DIR" ;;
    visual-inspection) printf '%s\n' "$LIGHT_VISUAL_INSPECTION_DIR" ;;
    storybook) printf '%s\n' "$LIGHT_STORYBOOK_UI_DIR" ;;
    *) echo "error: unknown artifact path: $1" >&2; return 2 ;;
  esac
}

# Stage the canonical demo show the Desk embeds for the editor and visualizer build.
#
# The tracked asset is generated by the operator-level demo generator and is the single product
# template. Staging copies it; it must never grow a second rig definition here.
build_demo_show() {
  echo "Staging the canonical demo show..."
  mkdir -p "$LIGHT_DEMO_SHOW_DIR"
  cp "$ROOT/assets/demo.show" "$LIGHT_DEMO_SHOW_DIR/demo-show.show"
}

# Frames of the demo show, rendered by the native core with no window.
#
# This is what the demo video is composited from. It opens no WebView and needs no display, so it
# runs the same on a build machine as it does here.
capture_demo() {
  require cargo
  build_demo_show
  echo "Capturing the demo show..."
  cargo build --release --manifest-path "$ROOT/Cargo.toml" -p viz-capture --bin viz-capture
  "$TARGET_DIR/release/viz-capture" \
    --show "$LIGHT_DEMO_SHOW_DIR/demo-show.show" \
    --output "$LIGHT_DEMO_SHOW_DIR/frames" \
    "$@"
}

# The Viz editor is the planning window for a standalone visualizer session: the desk's patch
# sheet over a show file, with no desk running. Like the visualizer, it is a separate product and
# opening ToskLight never builds it.
build_viz_editor() {
  require cargo
  require npm
  # The editor packages the demo as a resource, so it has to exist before the bundle is assembled.
  build_demo_show
  echo "Building the Viz editor..."
  (cd "$ROOT/apps/viz-editor" && npm run build)
  # `custom-protocol` is what makes this a real application rather than a development one: without
  # it Tauri embeds no frontend and the window opens on the dev-server URL, which is a white page
  # unless `npm run dev` happens to be running. The Tauri CLI passes it for `tauri build`; this is
  # a plain cargo build, so it passes it here.
  cargo build --release --manifest-path "$ROOT/Cargo.toml" -p viz-editor \
    --features custom-protocol
  echo "Viz editor built: $TARGET_DIR/release/viz-editor"
}

open_viz_editor() {
  require_built_file "$TARGET_DIR/release/viz-editor" "npm run build:viz-editor:open"
  [[ -f "$LIGHT_DEMO_SHOW_DIR/demo-show.show" ]] || {
    echo "error: no demo show build found at $LIGHT_DEMO_SHOW_DIR/demo-show.show" >&2
    echo "Build it first with: npm run build:viz-editor:open" >&2
    exit 1
  }
  echo "Opening the Viz editor."
  # A development build is not a bundle, so it has no resource directory to find the packaged demo
  # in. The generated artefact is the same file a release packages, so it is named directly.
  TOSKLIGHT_VIZ_DEMO_SHOW="$LIGHT_DEMO_SHOW_DIR/demo-show.show" \
    "$TARGET_DIR/release/viz-editor" "$@"
}

build_viz_editor_and_open() {
  build_viz_editor
  open_viz_editor "$@"
}

# The Media Server is a separate product with its own build. Building or opening ToskLight never
# builds it, and it never has to be present for the desk to run.
build_media() {
  require cargo
  require npm
  # The administration interface is compiled into the executable, so it is built first. The
  # helper skips the build when nothing that feeds it has changed.
  node "$ROOT/tools/ensure-media-frontend.mjs"
  echo "Building the Media Server..."
  cargo build --release --manifest-path "$ROOT/Cargo.toml" -p media-server
  echo "Media Server built: $TARGET_DIR/release/media-server"
  # The same bundle the release builds, so the product an operator develops against is the product
  # that ships: with its icon, and as the accessory application its menu bar item belongs to.
  if [[ "$(uname -s)" == "Darwin" ]]; then
    bash "$ROOT/tools/bundle-media-macos.sh" \
      "$TARGET_DIR/release/media-server" "$TARGET_DIR/release/bundle/macos"
  fi
}

# Where a development Media Server keeps its configuration and library.
media_data_dir() {
  printf '%s\n' "$LIGHT_RUNTIME_DATA_DIR/media"
}

# A first run has no configuration, and the shipped defaults are off-screen — correct for a
# server, useless for an operator who typed `open:media` expecting to see something. So a
# development configuration is seeded once, bound to the primary display. It is never overwritten
# afterwards: whatever the operator has changed is theirs.
seed_media_configuration() {
  local data_dir configuration
  data_dir="$(media_data_dir)"
  configuration="$data_dir/media-server.json"
  [[ -f "$configuration" ]] && return 0

  mkdir -p "$data_dir/library"
  echo "Seeding a development Media configuration at $configuration"
  cat >"$configuration" <<JSON
{
  "version": 1,
  "configuration": {
    "instanceId": "development",
    "library": { "root": "$data_dir/library", "targetCodec": "h264" },
    "outputs": [
      {
        "id": "6b1f0c2a-1111-4a2b-8c3d-000000000001",
        "name": "Main",
        "target": { "kind": "monitor", "monitor": { "by": "index", "value": 0 }, "fullscreen": false },
        "resolution": { "width": 1280, "height": 720 },
        "presentation": "display-synchronized",
        "personality": "two-layers",
        "protocol": "art-net",
        "universe": 9,
        "startAddress": 177
      }
    ]
  }
}
JSON
}

open_media() {
  require_built_file "$TARGET_DIR/release/media-server" "npm run build:media:open"
  seed_media_configuration
  local data_dir executable
  data_dir="$(media_data_dir)"
  executable="$(media_executable)"
  echo "Opening the Media Server. Quit it from its menu bar item."
  echo "Configuration: $data_dir/media-server.json"
  MEDIA_CONFIG="$data_dir/media-server.json" \
  MEDIA_LOG="${MEDIA_LOG:-info}" \
    "$executable" "$@"
}

# On macOS the executable has to be launched from inside the bundle to be an application at all:
# outside one it has no icon, and `LSUIElement` never applies, so there is no menu bar item to quit
# it from. Everywhere else the bare binary is the product.
media_executable() {
  local bundled="$TARGET_DIR/release/bundle/macos/ToskLight Media.app/Contents/MacOS/ToskLight Media"
  if [[ "$(uname -s)" == "Darwin" && -x "$bundled" ]]; then
    printf '%s\n' "$bundled"
  else
    printf '%s\n' "$TARGET_DIR/release/media-server"
  fi
}

build_media_and_open() {
  build_media
  open_media "$@"
}

# The visualizer is a separate product with its own build. Building or opening ToskLight never
# builds it, and it never has to be present for the desk to run.
build_visualizer() {
  require cargo
  echo "Building the standalone visualizer..."
  cargo build --release --manifest-path "$ROOT/Cargo.toml" -p viz-renderer
  echo "Visualizer built: $TARGET_DIR/release/viz-renderer"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    # Architect is one product with two executables, so the bundle needs both. Building only the
    # renderer here would produce a bundle that opens the visualizer and cannot reach the editor
    # at all, which is not the product the release assembles.
    build_viz_editor
    bash "$ROOT/tools/bundle-visualizer-macos.sh" \
      "$TARGET_DIR/release/viz-editor" \
      "$TARGET_DIR/release/viz-renderer" \
      "$TARGET_DIR/release/bundle/macos"
  fi
}

# On macOS the executable has to be launched from inside the bundle to inherit its icon, name and
# menu; everywhere else the bare binary is the product.
visualizer_executable() {
  local bundled="$TARGET_DIR/release/bundle/macos/ToskLight Architect.app/Contents/MacOS/viz-renderer"
  if [[ "$(uname -s)" == "Darwin" && -x "$bundled" ]]; then
    printf '%s\n' "$bundled"
  else
    printf '%s\n' "$TARGET_DIR/release/viz-renderer"
  fi
}

# The visualizer is a client of the desk, so it starts the development server only when nothing is
# already answering, and never stops a desk the operator is using.
ensure_desk_server_for_visualizer() {
  if curl -fsS http://127.0.0.1:5000/api/v2/readiness >/dev/null 2>&1; then
    echo "Using the desk already answering on http://127.0.0.1:5000"
    return 0
  fi
  echo "No desk is answering on http://127.0.0.1:5000; starting the latest built development server..."
  light_check_runtime_migration
  require_built_file "$TARGET_DIR/debug/light-headless" "npm run build:open"
  launchctl remove "$DEV_SERVER_LABEL" 2>/dev/null || true
  launchctl submit -l "$DEV_SERVER_LABEL" -o "$DATA_DIR/light-headless.log" -e "$DATA_DIR/light-headless.log" -- "$TARGET_DIR/debug/light-headless" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR"
  wait_for_launchd_server
  echo "Server log: $DATA_DIR/light-headless.log"
}

# Whether this launch was told to look at a desk, a show file or the built-in scene. With none of
# them the visualizer opens its planning window, and the rig comes from there rather than a desk.
visualizer_names_a_source() {
  local argument
  for argument in "$@"; do
    case "$argument" in
      --server|--port|--show|--demo) return 0 ;;
    esac
  done
  return 1
}

# A standalone visualizer can open a show file after launch, including from its planning session.
# Build the private server before exporting its path so the native UI never advertises a helper
# that does not exist yet.
build_visualizer_headless() {
  echo "Building the standalone visualizer's private show server..."
  cargo build --release --manifest-path "$ROOT/Cargo.toml" -p light-headless --bin light-headless
}

open_visualizer() {
  require curl
  require_built_file "$(visualizer_executable)" "npm run build:viz:open"
  require_built_file "$TARGET_DIR/release/viz-editor" "npm run build:viz:open"
  require_built_file "$TARGET_DIR/release/light-headless" "npm run build:viz:open"
  [[ -f "$LIGHT_DEMO_SHOW_DIR/demo-show.show" ]] || {
    echo "error: no demo show build found at $LIGHT_DEMO_SHOW_DIR/demo-show.show" >&2
    echo "Build it first with: npm run build:viz:open" >&2
    exit 1
  }
  if visualizer_names_a_source "$@"; then
    # A named desk is the running one. A named show uses the private server built above.
    if ! printf '%s\n' "$@" | grep -qx -- "--show"; then
      ensure_desk_server_for_visualizer
    fi
  else
    echo "Opening the Viz editor. Use Open Viz there to open its renderer."
    local library="${LIGHT_FIXTURE_LIBRARY:-$DATA_DIR/fixtures.sqlite}"
    if [[ ! -f "$library" ]]; then
      echo "No fixture library at $library yet; the editor's fixture browser will be empty until the desk has run once."
    fi
    TOSKLIGHT_VIZ_RENDERER="$(visualizer_executable)" \
    TOSKLIGHT_VIZ_HEADLESS="$TARGET_DIR/release/light-headless" \
    TOSKLIGHT_VIZ_DEMO_SHOW="$LIGHT_DEMO_SHOW_DIR/demo-show.show" \
    LIGHT_FIXTURE_LIBRARY="$library" \
      "$TARGET_DIR/release/viz-editor"
    return
  fi
  echo "Opening the visualizer. Press Command+, for Quick Settings."
  # In a development tree the two binaries sit beside each other in the target directory rather
  # than inside one installed bundle, so the visualizer is told where the editor is. The editor
  # patches from this checkout's own fixture library, which the development desk installs the
  # shipped packages into; without one the fixture browser is simply empty.
  local library="${LIGHT_FIXTURE_LIBRARY:-$DATA_DIR/fixtures.sqlite}"
  if [[ ! -f "$library" ]]; then
    echo "No fixture library at $library yet; the editor's fixture browser will be empty until the desk has run once."
  fi
  TOSKLIGHT_VIZ_EDITOR="$TARGET_DIR/release/viz-editor" \
  TOSKLIGHT_VIZ_HEADLESS="$TARGET_DIR/release/light-headless" \
  TOSKLIGHT_VIZ_DEMO_SHOW="$LIGHT_DEMO_SHOW_DIR/demo-show.show" \
  LIGHT_FIXTURE_LIBRARY="$library" \
    "$(visualizer_executable)" "$@"
}

build_visualizer_and_open() {
  # Build every helper before starting a desk, so a compile error cannot leave a new service behind.
  build_visualizer
  build_viz_editor
  build_visualizer_headless
  open_visualizer "$@"
}

case "${1:-}" in
  open-hardware-controls)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    open_hardware_controls
    ;;
  build-hardware-controls-open)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_hardware_controls_and_open
    ;;
  open-media)
    shift
    open_media "$@"
    ;;
  build-media-open)
    shift
    build_media_and_open "$@"
    ;;
  build-media)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_media
    ;;
  open-viz)
    shift
    open_visualizer "$@"
    ;;
  build-viz-open)
    shift
    build_visualizer_and_open "$@"
    ;;
  open-viz-editor)
    shift
    open_viz_editor "$@"
    ;;
  build-viz-editor-open)
    shift
    build_viz_editor_and_open "$@"
    ;;
  build-viz-editor)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_viz_editor
    ;;
  build-viz)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_visualizer
    ;;
  demo-show)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_demo_show
    ;;
  demo-capture)
    shift
    capture_demo "$@"
    ;;
  icon-contact-sheets)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_icon_contact_sheets
    ;;
  manual)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    build_manual
    ;;
  models)
    case "${2:-}" in
      "") build_stage_models ;;
      verify) [[ $# -eq 2 ]] || { usage >&2; exit 2; }; verify_stage_models ;;
      render) [[ $# -eq 2 ]] || { usage >&2; exit 2; }; render_stage_models ;;
      open) [[ $# -eq 2 ]] || { usage >&2; exit 2; }; open_stage_models ;;
      *) usage >&2; exit 2 ;;
    esac
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
    open_debug_app
    ;;
  build-open)
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
  clean-cargo-incremental)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    light_with_cargo_command_lock "npm run clean:cargo-incremental" light_clean_cargo_incremental
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
