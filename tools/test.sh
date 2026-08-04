#!/usr/bin/env bash

set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"
source "$ROOT/tools/cargo-command-lock.sh"
light_init_artifact_paths "$ROOT"
UI="$ROOT/apps/light-desktop"
HARDWARE_UI="$ROOT/apps/light-hardware-controls"
CONTROL_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-control-artifacts.json"

# Backs the root package.json test scripts; invoke via `npm run test:<name>`.
usage(){ echo "Usage: npm run test:{unit|verify|architecture|ui-package|patch-package|viz-editor|storybook|e2e-build|e2e|e2e-api|e2e-ui|e2e-performance|app-icons|artifact-paths|documentation-screenshots|marketing-screenshots|help-screenshots|help-screenshots-live|record|demo|all}"; }
build_e2e(){
  if [[ "${LIGHT_REUSE_E2E_BUILD:-0}" == "1" ]]; then
    local server="${LIGHT_E2E_SERVER:-$LIGHT_CARGO_TARGET_DIR/debug/light-headless}"
    [[ -x "$server" ]] || {
      echo "error: reusable E2E server is missing or not executable: $server" >&2
      return 1
    }
    return
  fi
  (cd "$UI" && npm run build)
  # Playwright shards reuse this debug executable on separate runners. rust-embed normally reads
  # assets from disk in debug builds, so opt into a self-contained UI for the transferable binary.
  light_with_cargo_command_lock "npm run test:e2e build" \
    cargo build --manifest-path "$ROOT/Cargo.toml" -p light-headless --no-default-features --features e2e-embedded-ui
}
architecture(){
  "$ROOT/tools/test-artifact-paths.sh"
  node --test "$ROOT/tools/cargo-workspace-lints.test.mjs"
  node --test "$ROOT/tools/check-control-state-labels.test.mjs"
  node --test "$ROOT/tools/programmer-action-timing.test.mjs"
  node --test "$ROOT/tools/performance-publication.test.mjs"
  node --test "$ROOT/tools/run-release-performance.test.mjs"
  node --test "$ROOT/tools/run-sustained-output-benchmark.test.mjs"
  node --test "$ROOT/tools/semantic-test-docs/"*.test.mjs
  node "$ROOT/tools/check-architecture.mjs"
  node "$ROOT/tools/test-app-icons.mjs"
  node --test "$ROOT/tools/source-size/source-size.test.mjs"
  node --test "$ROOT/tools/test-command-boundaries.test.mjs"
  node --test "$ROOT/tools/build-command-boundaries.test.mjs"
  node --test "$ROOT/tools/test-private-boundaries.test.mjs"
  node --test "$ROOT/tools/test-semantic-world-boundaries.test.mjs"
  node "$ROOT/tools/check-source-size.mjs"
}
typescript_unit(){
  (cd "$ROOT" && npm run test:bench-types)
  (cd "$ROOT" && npm run test:bench-unit)
  (cd "$ROOT" && npm run test:ui-package)
  (cd "$ROOT" && npm run test:patch-package)
  (cd "$ROOT" && npm run test:viz-editor)
  (cd "$UI" && npm test)
  (cd "$HARDWARE_UI" && npm test)
}
rust_unit(){
  # macOS native-process launch overhead can dominate tiny Rust test binaries. Keep the local lane to one
  # broad application test binary; comprehensive verification still executes every workspace
  # crate, integration test, and generated-contract check below.
  light_with_cargo_command_lock "npm run test:unit Rust application library" \
    cargo test --manifest-path "$ROOT/Cargo.toml" --no-default-features --lib -p light-application
}
rust_workspace(){
  light_with_cargo_command_lock "npm run test:verify Rust workspace" \
    cargo test --manifest-path "$ROOT/Cargo.toml" --workspace \
      --exclude light-desktop --exclude light-hardware-controls --no-default-features
}
unit(){ typescript_unit; rust_unit; }
verify(){
  architecture
  typescript_unit
  (cd "$UI" && npm run build)
  (cd "$HARDWARE_UI" && npm run build)
  rust_workspace
}
e2e(){ build_e2e; (cd "$UI" && npm run test:e2e -- "$@"); }
e2e_api(){ e2e --grep '@api' "$@"; }
e2e_ui(){ e2e --grep '@ui' --grep-invert '@(demo|docs|performance)\b' "$@"; }
e2e_performance(){ e2e --grep '@performance' "$@"; }
record(){
  build_e2e
  local status=0
  (cd "$UI" && LIGHT_VISUAL_RECORDING=1 npm run test:e2e -- --workers=1 "$@") || status=$?
  node "$ROOT/tools/assemble-visual-recording.mjs" || { [[ "$status" -ne 0 ]] || return 1; }
  return "$status"
}
demo(){
  build_e2e
  node "$ROOT/tools/configure-playwright-video-quality.mjs"
  (cd "$UI" && LIGHT_VISUAL_RECORDING=1 LIGHT_UPDATE_DEMO_SHOW=1 LIGHT_TEST_RESULTS_DIR="$LIGHT_ARTIFACTS_DIR/test/product-demo-results" npm run test:e2e -- --workers=1 product-demo.spec.ts "$@")
  node "$ROOT/tools/encode-product-demo.mjs"
}
run_help_screenshots(){
  (cd "$ROOT" && npx playwright test --config apps/ui-library/storybook/playwright.config.ts apps/ui-library/storybook/tests/help-screenshots.spec.ts "$@")
}
run_marketing_screenshots(){
  (cd "$ROOT" && npx playwright test --config apps/ui-library/storybook/playwright.config.ts apps/ui-library/storybook/tests/marketing-screenshots.spec.ts "$@")
}
run_documentation_screenshots(){
  (cd "$ROOT" && npx playwright test --config apps/ui-library/storybook/playwright.config.ts \
    apps/ui-library/storybook/tests/help-screenshots.spec.ts \
    apps/ui-library/storybook/tests/marketing-screenshots.spec.ts "$@")
}
help_screenshots(){
  (cd "$ROOT" && npm run storybook:build)
  run_help_screenshots "$@"
}
marketing_screenshots(){
  (cd "$ROOT" && npm run storybook:build)
  run_marketing_screenshots "$@"
}
documentation_screenshots(){
  (cd "$ROOT" && npm run storybook:build)
  run_documentation_screenshots "$@"
}
help_screenshots_live(){ build_e2e; (cd "$UI" && LIGHT_HELP_SCREENSHOTS=1 LIGHT_HELP_SCREENSHOTS_LIVE=1 npm run test:e2e -- 02-help-screenshots.spec.ts --workers=1 "$@"); }
ui_package(){ npm run test:ui-package --prefix "$ROOT"; }
patch_package(){ npm run test:patch-package --prefix "$ROOT"; }
viz_editor(){ npm run test:viz-editor --prefix "$ROOT"; }
storybook(){ npm run test:storybook --prefix "$ROOT"; }
command="${1:-}"; shift || true
case "$command" in
  app-icons) node "$ROOT/tools/test-app-icons.mjs" ;;
  architecture) architecture ;;
  artifact-paths) "$ROOT/tools/test-artifact-paths.sh" ;;
  unit) unit ;;
  verify) verify ;;
  e2e-build) build_e2e ;;
  e2e) e2e "$@" ;;
  e2e-api) e2e_api "$@" ;;
  e2e-ui) e2e_ui "$@" ;;
  e2e-performance) e2e_performance "$@" ;;
  documentation-screenshots) documentation_screenshots "$@" ;;
  marketing-screenshots) marketing_screenshots "$@" ;;
  help-screenshots) help_screenshots "$@" ;;
  help-screenshots-live) help_screenshots_live "$@" ;;
  ui-package) ui_package "$@" ;;
  patch-package) patch_package "$@" ;;
  viz-editor) viz_editor "$@" ;;
  storybook) storybook "$@" ;;
  record) record "$@" ;;
  demo) demo "$@" ;;
  all) verify; e2e "$@" ;;
  *) usage >&2; exit 2 ;;
esac
