#!/usr/bin/env bash

# shellcheck shell=bash

light_directory_has_entries() {
  [[ -d "$1" ]] && [[ -n "$(find "$1" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]]
}

light_check_runtime_migration() {
  [[ "${LIGHT_DATA_DIR_EXPLICIT:-0}" == 1 ]] && return 0
  local legacy="${LIGHT_LEGACY_DATA_DIR:-$LIGHT_REPOSITORY_ROOT/light-data}"
  local current="$LIGHT_RUNTIME_DATA_DIR"
  if light_directory_has_entries "$legacy" && light_directory_has_entries "$current"; then
    echo "error: development runtime data exists in both locations:" >&2
    echo "  legacy: $legacy" >&2
    echo "  current: $current" >&2
    echo "Resolve the conflict manually; ToskLight will not merge or choose between active shows." >&2
    return 1
  fi
  if light_directory_has_entries "$legacy"; then
    echo "error: legacy development runtime data is still at $legacy" >&2
    echo "Move it safely with: npm run migrate-artifacts" >&2
    return 1
  fi
}

light_migrate_runtime() {
  local legacy="${LIGHT_LEGACY_DATA_DIR:-$LIGHT_REPOSITORY_ROOT/light-data}"
  local current="$LIGHT_RUNTIME_DATA_DIR"
  [[ "${LIGHT_DATA_DIR_EXPLICIT:-0}" == 0 ]] || { echo "error: migration is unavailable while LIGHT_DATA_DIR is set" >&2; return 1; }
  [[ ! -L "$legacy" && ! -L "$current" ]] || { echo "error: refusing to migrate a symlinked runtime directory" >&2; return 1; }
  if light_directory_has_entries "$legacy" && light_directory_has_entries "$current"; then
    echo "error: both runtime locations contain data; move or archive one manually" >&2
    return 1
  fi
  if ! light_directory_has_entries "$legacy"; then
    echo "No legacy development runtime data found at $legacy"
    return 0
  fi
  [[ ! -e "$current" ]] || { echo "error: destination already exists: $current" >&2; return 1; }
  mkdir -p "$(dirname "$current")"
  mv "$legacy" "$current"
  echo "Moved development runtime data to $current"
  echo "Recovery: stop ToskLight, then move '$current' back to '$legacy' if needed."
}

light_assert_safe_cleanup_target() {
  local target="$1" artifact_root="$2"
  [[ -n "$target" && "$target" = /* ]] || { echo "error: cleanup target must be a resolved absolute path" >&2; return 1; }
  [[ "$target" != / && "$target" != "${HOME:-}" && "$target" != "$LIGHT_REPOSITORY_ROOT" && "$target" != "$artifact_root" ]] || {
    echo "error: refusing broad cleanup target: $target" >&2; return 1;
  }
  [[ "$target" == "$artifact_root"/* ]] || { echo "error: cleanup target is outside $artifact_root: $target" >&2; return 1; }
  [[ ! -L "$target" ]] || { echo "error: refusing symlinked cleanup target: $target" >&2; return 1; }
}

light_clean_reproducible() {
  local target
  for target in \
    "$LIGHT_ARTIFACTS_DIR/build" \
    "$LIGHT_ARTIFACTS_DIR/cache" \
    "$LIGHT_ARTIFACTS_DIR/generated" \
    "$LIGHT_ARTIFACTS_DIR/release" \
    "$LIGHT_ARTIFACTS_DIR/test" \
    "$LIGHT_ARTIFACTS_DIR/tmp"; do
    light_assert_safe_cleanup_target "$target" "$LIGHT_ARTIFACTS_DIR" || return 1
  done
  for target in \
    "$LIGHT_ARTIFACTS_DIR/build" \
    "$LIGHT_ARTIFACTS_DIR/cache" \
    "$LIGHT_ARTIFACTS_DIR/generated" \
    "$LIGHT_ARTIFACTS_DIR/release" \
    "$LIGHT_ARTIFACTS_DIR/test" \
    "$LIGHT_ARTIFACTS_DIR/tmp"; do
    [[ ! -e "$target" ]] || rm -rf -- "$target"
  done
  echo "Removed reproducible artifacts; preserved runtime at $LIGHT_RUNTIME_DATA_DIR"
}

light_clean_runtime() {
  local confirmation="${1:-}"
  light_assert_safe_cleanup_target "$LIGHT_RUNTIME_DATA_DIR" "$LIGHT_ARTIFACTS_DIR" || return 1
  [[ "$confirmation" == "$LIGHT_RUNTIME_DATA_DIR" ]] || {
    echo "error: runtime cleanup includes local shows and desk state" >&2
    echo "Confirm with: npm run clean -- runtime '$LIGHT_RUNTIME_DATA_DIR'" >&2
    return 1
  }
  [[ ! -e "$LIGHT_RUNTIME_DATA_DIR" ]] || rm -rf -- "$LIGHT_RUNTIME_DATA_DIR"
  echo "Removed development runtime data: $LIGHT_RUNTIME_DATA_DIR"
}
