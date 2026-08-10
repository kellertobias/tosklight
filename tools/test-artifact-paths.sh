#!/usr/bin/env bash

set -Eeuo pipefail

report_failure() {
  local status=$?
  local line="${BASH_LINENO[0]:-1}"
  printf '::error file=tools/test-artifact-paths.sh,line=%s,title=Artifact path contract failed::Assertion or command exited with status %s\n' "$line" "$status" >&2
  exit "$status"
}
trap report_failure ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"

WINDOWS_REPOSITORY_ROOT='D:\a\tosklight\tosklight'
WINDOWS_TMP_DIR='D:\a\tosklight\tosklight\.artifacts\tmp'
[[ "$(light_absolute_path "$WINDOWS_REPOSITORY_ROOT" "$WINDOWS_TMP_DIR")" == "$WINDOWS_TMP_DIR" ]]
[[ "$(light_absolute_path "$WINDOWS_REPOSITORY_ROOT" 'D:/external/artifacts')" == 'D:/external/artifacts' ]]
[[ "$(light_absolute_path "$WINDOWS_REPOSITORY_ROOT" '\\server\share\artifacts')" == '\\server\share\artifacts' ]]

BOOTSTRAP_TMP_ROOT="${LIGHT_TMP_DIR:-$ROOT/.artifacts/tmp}"
mkdir -p "$BOOTSTRAP_TMP_ROOT"
TEST_ROOT="$(mktemp -d "$BOOTSTRAP_TMP_ROOT/light artifact paths.XXXXXX")"
trap 'rm -rf -- "$TEST_ROOT"' EXIT
cp "$ROOT/tools/artifact-layout.conf" "$TEST_ROOT/artifact-layout.conf"
mkdir -p "$TEST_ROOT/repository/tools"
cp "$ROOT/tools/artifact-layout.conf" "$TEST_ROOT/repository/tools/artifact-layout.conf"

LIGHT_ARTIFACTS_DIR="$TEST_ROOT/artifacts with spaces"
unset LIGHT_DATA_DIR CARGO_TARGET_DIR LIGHT_CONTROL_FRONTEND_DIR LIGHT_HARDWARE_FRONTEND_DIR
unset LIGHT_VIZ_EDITOR_FRONTEND_DIR LIGHT_MEDIA_FRONTEND_DIR
unset LIGHT_STORYBOOK_UI_DIR LIGHT_PNPM_STORE_DIR LIGHT_VITE_CACHE_DIR LIGHT_PYTHON_CACHE_DIR
unset LIGHT_MANUAL_ROOT LIGHT_ICON_CONTACT_SHEETS_DIR LIGHT_RELEASE_DIR LIGHT_PERFORMANCE_DIR LIGHT_RUNTIME_DATA_DIR LIGHT_RUNTIME_EXTENSIONS_DIR LIGHT_EXTENSIONS_DIR
unset LIGHT_TEST_COVERAGE_DIR LIGHT_PLAYWRIGHT_REPORT_DIR LIGHT_TEST_RESULTS_DIR
unset LIGHT_VISUAL_INSPECTION_DIR LIGHT_TMP_DIR LIGHT_PAGES_DIR LIGHT_SAFARI_DIR
light_init_artifact_paths "$TEST_ROOT/repository"
[[ "$CARGO_TARGET_DIR" == "$TEST_ROOT/artifacts with spaces/build/cargo" ]]
[[ "$LIGHT_CONTROL_FRONTEND_DIR" == "$TEST_ROOT/artifacts with spaces/build/frontend/light-desktop" ]]
[[ "$LIGHT_HARDWARE_FRONTEND_DIR" == "$TEST_ROOT/artifacts with spaces/build/frontend/light-hardware-controls" ]]
[[ "$LIGHT_MEDIA_FRONTEND_DIR" == "$TEST_ROOT/artifacts with spaces/build/frontend/media" ]]
[[ "$LIGHT_STORYBOOK_UI_DIR" == "$TEST_ROOT/artifacts with spaces/build/storybook/ui" ]]
[[ "$LIGHT_VITE_CACHE_DIR" == "$TEST_ROOT/artifacts with spaces/cache/vite" ]]
[[ "$LIGHT_PYTHON_CACHE_DIR" == "$TEST_ROOT/artifacts with spaces/cache/python" ]]
[[ "$LIGHT_MANUAL_ROOT" == "$TEST_ROOT/artifacts with spaces/generated/manual" ]]
[[ "$LIGHT_RELEASE_DIR" == "$TEST_ROOT/artifacts with spaces/release" ]]
[[ "$LIGHT_PERFORMANCE_DIR" == "$TEST_ROOT/artifacts with spaces/performance" ]]
[[ "$LIGHT_TEST_RESULTS_DIR" == "$TEST_ROOT/artifacts with spaces/test/results" ]]
[[ "$LIGHT_PLAYWRIGHT_REPORT_DIR" == "$TEST_ROOT/artifacts with spaces/test/playwright-report" ]]
[[ "$LIGHT_TMP_DIR" == "$TEST_ROOT/artifacts with spaces/tmp" ]]
[[ "$LIGHT_DATA_DIR" == "$TEST_ROOT/artifacts with spaces/runtime/light-data" ]]
[[ "$LIGHT_RUNTIME_EXTENSIONS_DIR" == "$TEST_ROOT/artifacts with spaces/runtime/extensions" ]]
[[ "$TMPDIR" == "$LIGHT_TMP_DIR" && "$TMP" == "$LIGHT_TMP_DIR" && "$TEMP" == "$LIGHT_TMP_DIR" ]]
[[ "$PYTHONPYCACHEPREFIX" == "$LIGHT_PYTHON_CACHE_DIR" ]]
[[ "$(node -e 'const { artifactPaths } = require(process.argv[1]); process.stdout.write(artifactPaths.tmp)' "$ROOT/tools/artifact-paths.cjs")" == "$LIGHT_TMP_DIR" ]]
[[ "$(node -e 'const { artifactPaths } = require(process.argv[1]); process.stdout.write(artifactPaths.performance)' "$ROOT/tools/artifact-paths.cjs")" == "$LIGHT_PERFORMANCE_DIR" ]]
[[ "$(node -e 'const { artifactPaths } = require(process.argv[1]); process.stdout.write(artifactPaths.extensions)' "$ROOT/tools/artifact-paths.cjs")" == "$LIGHT_RUNTIME_EXTENSIONS_DIR" ]]
[[ "$(PYTHONPATH="$ROOT/tools" python3 -c 'from artifact_paths import artifact_path; print(artifact_path("LIGHT_TMP_DIR", "TMP_ROOT"))')" == "$LIGHT_TMP_DIR" ]]

printf root-file > "$TEST_ROOT/repository/README.md"
mkdir -p "$TEST_ROOT/repository/.next"
printf generated > "$TEST_ROOT/repository/.next/output"
ln -s "$TEST_ROOT/repository/README.md" "$TEST_ROOT/repository/root-link"
unexpected="$(light_list_unexpected_root_entries "$TEST_ROOT/repository")"
[[ "$unexpected" == *"$TEST_ROOT/repository/.next"* ]]
[[ "$unexpected" == *"$TEST_ROOT/repository/root-link"* ]]
[[ "$unexpected" != *"$TEST_ROOT/repository/tools"* ]]
[[ "$unexpected" != *"$TEST_ROOT/repository/README.md"* ]]
light_clean_repository_root >/dev/null
[[ ! -e "$TEST_ROOT/repository/.next" && ! -L "$TEST_ROOT/repository/root-link" ]]
cleanup_archive="$(find "$LIGHT_ARTIFACTS_DIR/cleanup/repository-root" -mindepth 1 -maxdepth 1 -type d -print -quit)"
[[ -n "$cleanup_archive" ]]
[[ "$(<"$cleanup_archive/.next/output")" == generated ]]
[[ -L "$cleanup_archive/root-link" ]]

LIGHT_LEGACY_DATA_DIR="$TEST_ROOT/legacy/light-data"
mkdir -p "$LIGHT_LEGACY_DATA_DIR/matter"
printf 'show-state' > "$LIGHT_LEGACY_DATA_DIR/show.show"
printf 'identity' > "$LIGHT_LEGACY_DATA_DIR/matter/identity.json"
printf 'recovery' > "$LIGHT_LEGACY_DATA_DIR/.upload-recovery"
printf 'wal' > "$LIGHT_LEGACY_DATA_DIR/desk.sqlite-wal"
printf 'shm' > "$LIGHT_LEGACY_DATA_DIR/desk.sqlite-shm"
light_migrate_runtime >/dev/null
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/show.show")" == show-state ]]
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/matter/identity.json")" == identity ]]
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/.upload-recovery")" == recovery ]]
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/desk.sqlite-wal")" == wal ]]
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/desk.sqlite-shm")" == shm ]]
[[ ! -e "$LIGHT_LEGACY_DATA_DIR" ]]

mkdir -p "$LIGHT_LEGACY_DATA_DIR" "$LIGHT_RUNTIME_DATA_DIR"
printf legacy > "$LIGHT_LEGACY_DATA_DIR/conflict"
printf current > "$LIGHT_RUNTIME_DATA_DIR/conflict"
if light_migrate_runtime >/dev/null 2>&1; then
  echo "error: two-location migration conflict was accepted" >&2
  exit 1
fi
[[ "$(<"$LIGHT_LEGACY_DATA_DIR/conflict")" == legacy ]]
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/conflict")" == current ]]
rm -rf -- "$LIGHT_LEGACY_DATA_DIR"

mkdir -p "$LIGHT_ARTIFACTS_DIR"/{build,cache,generated,legacy,performance,release,test,tmp} "$LIGHT_RUNTIME_DATA_DIR"
printf keep > "$LIGHT_RUNTIME_DATA_DIR/sentinel"
printf recovery > "$LIGHT_ARTIFACTS_DIR/cleanup/recovery-sentinel"
mkdir -p "$CARGO_TARGET_DIR/debug/incremental/stale-work" "$CARGO_TARGET_DIR/debug/deps"
printf stale > "$CARGO_TARGET_DIR/debug/incremental/stale-work/object.o"
printf keep > "$CARGO_TARGET_DIR/debug/deps/library.rlib"
light_clean_cargo_incremental >/dev/null
[[ ! -e "$CARGO_TARGET_DIR/debug/incremental" ]]
[[ "$(<"$CARGO_TARGET_DIR/debug/deps/library.rlib")" == keep ]]
light_clean_reproducible >/dev/null
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/sentinel")" == keep ]]
[[ ! -e "$LIGHT_ARTIFACTS_DIR/build" && ! -e "$LIGHT_ARTIFACTS_DIR/test" ]]
[[ ! -e "$LIGHT_ARTIFACTS_DIR/legacy" && ! -e "$LIGHT_ARTIFACTS_DIR/performance" ]]
[[ "$(<"$LIGHT_ARTIFACTS_DIR/cleanup/recovery-sentinel")" == recovery ]]

if light_clean_runtime wrong >/dev/null 2>&1; then
  echo "error: runtime cleanup accepted an incorrect confirmation" >&2
  exit 1
fi
[[ -e "$LIGHT_RUNTIME_DATA_DIR/sentinel" ]]
light_clean_runtime "$LIGHT_RUNTIME_DATA_DIR" >/dev/null
[[ ! -e "$LIGHT_RUNTIME_DATA_DIR" ]]

for unsafe in "" / "${HOME:-}" "$LIGHT_REPOSITORY_ROOT" "$LIGHT_ARTIFACTS_DIR"; do
  if light_assert_safe_cleanup_target "$unsafe" "$LIGHT_ARTIFACTS_DIR" >/dev/null 2>&1; then
    echo "error: cleanup accepted unsafe target: ${unsafe:-<empty>}" >&2
    exit 1
  fi
done

mkdir -p "$TEST_ROOT/external"
ln -s "$TEST_ROOT/external" "$LIGHT_ARTIFACTS_DIR/build"
if light_clean_reproducible >/dev/null 2>&1; then
  echo "error: cleanup followed a symlinked artifact subtree" >&2
  exit 1
fi
[[ -d "$TEST_ROOT/external" ]]

LIGHT_DATA_DIR="$TEST_ROOT/explicit data"
CARGO_TARGET_DIR="$TEST_ROOT/explicit cargo"
light_init_artifact_paths "$TEST_ROOT/repository"
[[ "$LIGHT_DATA_DIR_EXPLICIT" == 1 && "$LIGHT_DATA_DIR" == "$TEST_ROOT/explicit data" ]]
[[ "$CARGO_TARGET_DIR" == "$TEST_ROOT/explicit cargo" ]]

if (LIGHT_ARTIFACTS_DIR=""; light_init_artifact_paths "$TEST_ROOT/repository" >/dev/null 2>&1); then
  echo "error: an explicitly empty artifact root was silently accepted" >&2
  exit 1
fi

if grep -ERn '\$RUNNER_TEMP|os\.tmpdir\(\)|mktemp -d\)' \
  "$ROOT/.github" \
  "$ROOT/tools"/*.mjs \
  "$ROOT/tools/semantic-test-docs"/*.mjs \
  "$ROOT/tests/bench/core/lightBench.ts"; then
  echo "error: repository-owned command or CI temporary output bypasses .artifacts/tmp" >&2
  exit 1
fi

if [[ -e "$ROOT/artifacts" ]]; then
  echo "error: legacy undotted artifact root exists: $ROOT/artifacts" >&2
  exit 1
fi
unexpected="$(light_list_unexpected_root_entries "$ROOT")"
if [[ -n "$unexpected" ]]; then
  echo "error: unexpected non-file entries exist at the repository root:" >&2
  while IFS= read -r entry; do
    printf '  %s\n' "$entry" >&2
  done <<<"$unexpected"
  echo "Run: npm run clean:root" >&2
  exit 1
fi
if grep -ERn '(^|[^.[:alnum:]_-])artifacts/' \
  "$ROOT/.github" \
  "$ROOT/package.json" \
  "$ROOT/apps"/*/package.json \
  "$ROOT/tools"/*.mjs \
  "$ROOT/tools/build.sh" \
  "$ROOT/tools/dev.sh" \
  "$ROOT/tools/test.sh"; then
  echo "error: repository-owned command references the legacy undotted artifacts directory" >&2
  exit 1
fi
if grep -Fq '/artifacts/' "$ROOT/.gitignore"; then
  echo "error: .gitignore hides the forbidden undotted artifact root" >&2
  exit 1
fi

grep -Fq 'target-dir = ".artifacts/build/cargo"' "$ROOT/.cargo/config.toml"
grep -Fq 'outputDir: artifactPaths.results' "$ROOT/playwright.config.ts"
grep -Fq 'outDir: artifactPaths.controlFrontend' "$ROOT/apps/light-desktop/vite.config.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/light-desktop`' "$ROOT/apps/light-desktop/vite.config.ts"
grep -Fq 'outDir: artifactPaths.mediaFrontend' "$ROOT/apps/media/vite.config.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/media`' "$ROOT/apps/media/vite.config.ts"
grep -Fq 'outDir: artifactPaths.hardwareFrontend' "$ROOT/apps/light-hardware-controls/vite.config.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/light-hardware-controls`' "$ROOT/apps/light-hardware-controls/vite.config.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/ui-library-vitest`' "$ROOT/apps/ui-library/vitest.config.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/ui-library-storybook`' "$ROOT/apps/ui-library/storybook/config/main.ts"
grep -Fq 'cacheDir: `${artifactPaths.viteCache}/root`' "$ROOT/vitest.config.ts"
grep -Fq 'config="$(npm run --silent artifact-path -- tmp)/tauri-control-release.json"' "$ROOT/.github/workflows/release.yml"

# The canonical headless hot-reload command must not scan the custom Cargo target under
# .artifacts. Explicit source roots keep first startup bounded and avoid compiler-output feedback.
grep -Fq -- '--no-vcs-ignores' "$ROOT/tools/dev.sh"
grep -Fq -- '--no-dot-ignores' "$ROOT/tools/dev.sh"
grep -Fq -- '--watch "$ROOT/apps/light-headless"' "$ROOT/tools/dev.sh"
grep -Fq -- '--watch "$ROOT/crates"' "$ROOT/tools/dev.sh"
grep -Fq -- '--watch "$ROOT/Cargo.toml"' "$ROOT/tools/dev.sh"
grep -Fq -- '--watch "$ROOT/Cargo.lock"' "$ROOT/tools/dev.sh"

echo "Artifact path, migration, override, and cleanup safety tests passed."
