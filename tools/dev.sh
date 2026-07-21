#!/usr/bin/env bash

# Development runner. Invoked through the root package.json scripts:
#   npm run dev          -> both (server with hot reload + UI in the browser)
#   npm run dev:server   -> server only, hot-reloaded on .rs changes
#   npm run dev:ui       -> control UI only, in the browser
#   npm run dev:app      -> server + the Tauri desktop app (the former ./dev)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"
light_init_artifact_paths "$ROOT"
UI_DIR="$ROOT/apps/control-ui"
DATA_DIR="$LIGHT_DATA_DIR"
FIXTURE_LIBRARY_DIR="$ROOT/assets/fixture-library"
TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-control-artifacts.json"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "error: $1 is required" >&2; exit 1; }
}

ensure_cargo_watch() {
  if ! command -v cargo-watch >/dev/null 2>&1; then
    echo "cargo-watch is not installed; installing it for hot reload..."
    cargo install cargo-watch --locked
  fi
}

# Foreground server. With hot reload, cargo-watch restarts it on any .rs change.
run_server() {
  local reload="${1:-true}"
  require cargo
  light_check_runtime_migration
  mkdir -p "$DATA_DIR"
  local args=(--manifest-path "$ROOT/Cargo.toml" -p light-server --bin light-server --
    --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR")
  if [[ "$reload" == true ]]; then
    ensure_cargo_watch
    echo "Starting Light server with hot reload (restarts on .rs changes)..."
    exec cargo watch --why -x "run ${args[*]}"
  fi
  echo "Starting Light server..."
  exec cargo run "${args[@]}"
}

# Background server used by the combined modes; blocks until readiness or exit.
start_background_server() {
  local reload="$1"
  ( run_server "$reload" ) &
  SERVER_PID=$!
  for _ in {1..600}; do
    curl -fsS http://127.0.0.1:5000/api/v1/readiness >/dev/null 2>&1 && return 0
    kill -0 "$SERVER_PID" 2>/dev/null || { echo "error: Light server exited during startup" >&2; exit 1; }
    sleep 0.1
  done
  echo "error: Light server did not become ready; see $DATA_DIR/light-server.log" >&2
  exit 1
}

run_ui() {
  require npm
  echo "Starting the control UI in the browser (http://127.0.0.1:5173)..."
  exec bash -c "cd '$UI_DIR' && npm run dev"
}

# server (hot reload) + UI in the browser
run_both() {
  require curl
  start_background_server true
  run_ui
}

# server + the Tauri desktop app (the former ./dev)
run_app() {
  require cargo
  require npm
  require curl
  node "$ROOT/tools/write-tauri-artifact-config.mjs" control "$TAURI_CONFIG"
  start_background_server false
  echo "Starting the Tauri development app..."
  (cd "$UI_DIR" && npm run tauri:dev -- --config "$TAURI_CONFIG")
}

case "${1:-both}" in
  both) run_both ;;
  server) run_server true ;;
  ui) run_ui ;;
  app) run_app ;;
  *) echo "usage: dev.sh {both|server|ui|app}" >&2; exit 2 ;;
esac
