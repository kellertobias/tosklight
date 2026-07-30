#!/usr/bin/env bash

set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"
light_init_artifact_paths "$ROOT"
UI="$ROOT/apps/light-desktop"
HARDWARE_UI="$ROOT/apps/light-hardware-controls"
CONTROL_TAURI_CONFIG="$LIGHT_TMP_DIR/tauri-control-artifacts.json"

# Backs the root package.json test scripts; invoke via `npm run test:<name>`.
usage(){ echo "Usage: npm run test:{unit|architecture|ui-package|storybook|e2e|e2e-api|e2e-ui|app-icons|artifact-paths|marketing-screenshots|help-screenshots|help-screenshots-live|record|demo|all}"; }
build_e2e(){ (cd "$UI" && npm run build); cargo build --manifest-path "$ROOT/Cargo.toml" -p light-headless --no-default-features; }
architecture(){
  "$ROOT/tools/test-artifact-paths.sh"
  node --test "$ROOT/tools/cargo-workspace-lints.test.mjs"
  node --test "$ROOT/tools/check-control-state-labels.test.mjs"
  node --test "$ROOT/tools/programmer-action-timing.test.mjs"
  node --test "$ROOT/tools/run-release-performance.test.mjs"
  node --test "$ROOT/tools/run-sustained-output-benchmark.test.mjs"
  node --test "$ROOT/tools/semantic-test-docs/"*.test.mjs
  node "$ROOT/tools/check-architecture.mjs"
  node --test "$ROOT/tools/source-size/source-size.test.mjs"
  node --test "$ROOT/tools/test-command-boundaries.test.mjs"
  node --test "$ROOT/tools/test-private-boundaries.test.mjs"
  node --test "$ROOT/tools/test-semantic-world-boundaries.test.mjs"
  node "$ROOT/tools/check-source-size.mjs"
}
unit(){ architecture; (cd "$ROOT" && npm run test:bench-types); (cd "$ROOT" && npm run test:bench-unit); (cd "$ROOT" && npm run test:ui-package); (cd "$UI" && npm run build); (cd "$HARDWARE_UI" && npm run build); cargo test --manifest-path "$ROOT/Cargo.toml" --workspace --exclude light-desktop --exclude light-hardware-controls --no-default-features; (cd "$UI" && npm test); (cd "$HARDWARE_UI" && npm test); }
e2e(){ build_e2e; (cd "$UI" && npm run test:e2e -- "$@"); }
e2e_api(){ e2e --grep '@api' "$@"; }
e2e_ui(){ e2e --grep '@ui' --grep-invert '@(demo|docs)\b' "$@"; }
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
help_screenshots(){
  (cd "$ROOT" && npm run storybook:build)
  (cd "$ROOT" && npx playwright test --config apps/ui-library/storybook/playwright.config.ts apps/ui-library/storybook/tests/help-screenshots.spec.ts "$@")
}
marketing_screenshots(){
  (cd "$ROOT" && npm run storybook:build)
  (cd "$ROOT" && npx playwright test --config apps/ui-library/storybook/playwright.config.ts apps/ui-library/storybook/tests/marketing-screenshots.spec.ts "$@")
}
help_screenshots_live(){ build_e2e; (cd "$UI" && LIGHT_HELP_SCREENSHOTS=1 LIGHT_HELP_SCREENSHOTS_LIVE=1 npm run test:e2e -- 02-help-screenshots.spec.ts --workers=1 "$@"); }
ui_package(){ npm run test:ui-package --prefix "$ROOT"; }
storybook(){ npm run test:storybook --prefix "$ROOT"; }
command="${1:-}"; shift || true
case "$command" in
  app-icons) node "$ROOT/tools/test-app-icons.mjs" ;;
  architecture) architecture ;;
  artifact-paths) "$ROOT/tools/test-artifact-paths.sh" ;;
  unit) unit ;;
  e2e) e2e "$@" ;;
  e2e-api) e2e_api "$@" ;;
  e2e-ui) e2e_ui "$@" ;;
  marketing-screenshots) marketing_screenshots "$@" ;;
  help-screenshots) help_screenshots "$@" ;;
  help-screenshots-live) help_screenshots_live "$@" ;;
  ui-package) ui_package "$@" ;;
  storybook) storybook "$@" ;;
  record) record "$@" ;;
  demo) demo "$@" ;;
  all) unit; e2e "$@" ;;
  *) usage >&2; exit 2 ;;
esac
