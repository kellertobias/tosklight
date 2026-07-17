#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/artifact-paths.sh"
source "$ROOT/tools/artifact-maintenance.sh"

TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/light artifact paths.XXXXXX")"
trap 'rm -rf -- "$TEST_ROOT"' EXIT
cp "$ROOT/tools/artifact-layout.conf" "$TEST_ROOT/artifact-layout.conf"
mkdir -p "$TEST_ROOT/repository/tools"
cp "$ROOT/tools/artifact-layout.conf" "$TEST_ROOT/repository/tools/artifact-layout.conf"

LIGHT_ARTIFACTS_DIR="$TEST_ROOT/artifacts with spaces"
unset LIGHT_DATA_DIR CARGO_TARGET_DIR LIGHT_CONTROL_FRONTEND_DIR LIGHT_HARDWARE_FRONTEND_DIR
unset LIGHT_PNPM_STORE_DIR LIGHT_MANUAL_ROOT LIGHT_RELEASE_DIR LIGHT_RUNTIME_DATA_DIR
unset LIGHT_TEST_COVERAGE_DIR LIGHT_PLAYWRIGHT_REPORT_DIR LIGHT_TEST_RESULTS_DIR
unset LIGHT_VISUAL_INSPECTION_DIR LIGHT_TMP_DIR
light_init_artifact_paths "$TEST_ROOT/repository"
[[ "$CARGO_TARGET_DIR" == "$TEST_ROOT/artifacts with spaces/build/cargo" ]]
[[ "$LIGHT_DATA_DIR" == "$TEST_ROOT/artifacts with spaces/runtime/light-data" ]]

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

mkdir -p "$LIGHT_ARTIFACTS_DIR"/{build,cache,generated,release,test,tmp} "$LIGHT_RUNTIME_DATA_DIR"
printf keep > "$LIGHT_RUNTIME_DATA_DIR/sentinel"
light_clean_reproducible >/dev/null
[[ "$(<"$LIGHT_RUNTIME_DATA_DIR/sentinel")" == keep ]]
[[ ! -e "$LIGHT_ARTIFACTS_DIR/build" && ! -e "$LIGHT_ARTIFACTS_DIR/test" ]]

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

echo "Artifact path, migration, override, and cleanup safety tests passed."
