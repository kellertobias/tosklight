#!/usr/bin/env bash

# shellcheck shell=bash

light_absolute_path() {
  local base="$1" value="$2"
  [[ -n "$value" ]] || { echo "error: artifact path override cannot be empty" >&2; return 1; }
  case "$value" in
    ../*|*/../*|*/..|./*|*/./*|*/.)
      echo "error: artifact path override must not contain unresolved dot segments: $value" >&2
      return 1
      ;;
  esac
  if [[ "$value" = /* ]]; then
    printf '%s\n' "$value"
  else
    printf '%s/%s\n' "$base" "$value"
  fi
}

light_export_path() {
  local name="$1" base="$2" value="$3" resolved
  resolved="$(light_absolute_path "$base" "$value")" || return 1
  printf -v "$name" '%s' "$resolved"
  export "$name"
}

light_init_artifact_paths() {
  local repository_root="$1"
  local layout="$repository_root/tools/artifact-layout.conf"
  [[ -f "$layout" ]] || { echo "error: artifact layout not found: $layout" >&2; return 1; }

  # shellcheck disable=SC1090
  source "$layout"
  export LIGHT_REPOSITORY_ROOT="$repository_root"
  light_export_path LIGHT_ARTIFACTS_DIR "$repository_root" "${LIGHT_ARTIFACTS_DIR-.artifacts}" || return 1
  light_export_path CARGO_TARGET_DIR "$repository_root" "${CARGO_TARGET_DIR-$LIGHT_ARTIFACTS_DIR/$BUILD_CARGO}" || return 1
  export LIGHT_CARGO_TARGET_DIR="$CARGO_TARGET_DIR"
  light_export_path LIGHT_CONTROL_FRONTEND_DIR "$repository_root" "${LIGHT_CONTROL_FRONTEND_DIR-$LIGHT_ARTIFACTS_DIR/$FRONTEND_CONTROL}" || return 1
  light_export_path LIGHT_HARDWARE_FRONTEND_DIR "$repository_root" "${LIGHT_HARDWARE_FRONTEND_DIR-$LIGHT_ARTIFACTS_DIR/$FRONTEND_HARDWARE}" || return 1
  light_export_path LIGHT_PNPM_STORE_DIR "$repository_root" "${LIGHT_PNPM_STORE_DIR-$LIGHT_ARTIFACTS_DIR/$CACHE_PNPM}" || return 1
  light_export_path LIGHT_MANUAL_ROOT "$repository_root" "${LIGHT_MANUAL_ROOT-$LIGHT_ARTIFACTS_DIR/$MANUAL_ROOT}" || return 1
  light_export_path LIGHT_RELEASE_DIR "$repository_root" "${LIGHT_RELEASE_DIR-$LIGHT_ARTIFACTS_DIR/$RELEASE_ROOT}" || return 1
  light_export_path LIGHT_RUNTIME_DATA_DIR "$repository_root" "${LIGHT_RUNTIME_DATA_DIR-$LIGHT_ARTIFACTS_DIR/$RUNTIME_DATA}" || return 1
  light_export_path LIGHT_TEST_COVERAGE_DIR "$repository_root" "${LIGHT_TEST_COVERAGE_DIR-$LIGHT_ARTIFACTS_DIR/$TEST_COVERAGE}" || return 1
  light_export_path LIGHT_PLAYWRIGHT_REPORT_DIR "$repository_root" "${LIGHT_PLAYWRIGHT_REPORT_DIR-$LIGHT_ARTIFACTS_DIR/$TEST_REPORT}" || return 1
  light_export_path LIGHT_TEST_RESULTS_DIR "$repository_root" "${LIGHT_TEST_RESULTS_DIR-$LIGHT_ARTIFACTS_DIR/$TEST_RESULTS}" || return 1
  light_export_path LIGHT_VISUAL_INSPECTION_DIR "$repository_root" "${LIGHT_VISUAL_INSPECTION_DIR-$LIGHT_ARTIFACTS_DIR/$TEST_VISUAL}" || return 1
  light_export_path LIGHT_TMP_DIR "$repository_root" "${LIGHT_TMP_DIR-$LIGHT_ARTIFACTS_DIR/$TMP_ROOT}" || return 1
  export LIGHT_MANUAL_PDF="$LIGHT_MANUAL_ROOT/pdf/tosklight-manual.pdf"
  export LIGHT_MANUAL_HTML_DIR="$LIGHT_MANUAL_ROOT/html/tosklight-manual"
  export LIGHT_MANUAL_HTML_ARCHIVE="$LIGHT_MANUAL_ROOT/html/tosklight-manual-html.zip"
  light_export_path LIGHT_PAGES_DIR "$repository_root" "${LIGHT_PAGES_DIR-$LIGHT_ARTIFACTS_DIR/$PAGES_ROOT}" || return 1
  light_export_path LIGHT_SAFARI_DIR "$repository_root" "${LIGHT_SAFARI_DIR-$LIGHT_ARTIFACTS_DIR/$SAFARI_ROOT}" || return 1

  if [[ -n "${LIGHT_DATA_DIR+x}" ]]; then
    light_export_path LIGHT_DATA_DIR "$repository_root" "$LIGHT_DATA_DIR" || return 1
    export LIGHT_DATA_DIR_EXPLICIT=1
  else
    export LIGHT_DATA_DIR="$LIGHT_RUNTIME_DATA_DIR"
    export LIGHT_DATA_DIR_EXPLICIT=0
  fi
}
