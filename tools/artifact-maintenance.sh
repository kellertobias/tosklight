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

light_root_directory_allowed() {
  case "$1" in
    .agents|.artifacts|.cargo|.forgejo|.git|.github|.show|.tour|apps|assets|crates|docs|experiments|node_modules|tests|tools)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

light_list_unexpected_root_entries() {
  local repository_root="$1" entry name
  [[ -n "$repository_root" && "$repository_root" = /* && -d "$repository_root" ]] || {
    echo "error: repository root must be an existing absolute directory" >&2
    return 1
  }
  while IFS= read -r -d '' entry; do
    name="${entry##*/}"
    if [[ -f "$entry" && ! -L "$entry" ]]; then
      continue
    fi
    if light_root_directory_allowed "$name" && [[ -d "$entry" && ! -L "$entry" ]]; then
      continue
    fi
    printf '%s\n' "$entry"
  done < <(find "$repository_root" -mindepth 1 -maxdepth 1 -print0)
}

light_clean_repository_root() {
  local repository_root="$LIGHT_REPOSITORY_ROOT" entry name
  local -a unexpected=()
  while IFS= read -r entry; do
    unexpected+=("$entry")
  done < <(light_list_unexpected_root_entries "$repository_root")
  if [[ "${#unexpected[@]}" -eq 0 ]]; then
    echo "Repository root already contains only approved directories and files."
    return 0
  fi

  local archive="$LIGHT_ARTIFACTS_DIR/cleanup/repository-root/$(date -u '+%Y%m%dT%H%M%SZ')-$$"
  light_assert_safe_cleanup_target "$archive" "$LIGHT_ARTIFACTS_DIR" || return 1
  mkdir -p "$archive"
  for entry in "${unexpected[@]}"; do
    name="${entry##*/}"
    [[ ! -e "$archive/$name" && ! -L "$archive/$name" ]] || {
      echo "error: cleanup archive collision: $archive/$name" >&2
      return 1
    }
    mv -- "$entry" "$archive/$name"
  done
  echo "Moved ${#unexpected[@]} unexpected root entries to $archive"
  echo "Recovery: move any required entry from that directory back to $repository_root."
}

light_clean_reproducible() {
  local target
  for target in \
    "$LIGHT_ARTIFACTS_DIR/build" \
    "$LIGHT_ARTIFACTS_DIR/cache" \
    "$LIGHT_ARTIFACTS_DIR/generated" \
    "$LIGHT_ARTIFACTS_DIR/legacy" \
    "$LIGHT_ARTIFACTS_DIR/performance" \
    "$LIGHT_ARTIFACTS_DIR/release" \
    "$LIGHT_ARTIFACTS_DIR/test" \
    "$LIGHT_ARTIFACTS_DIR/tmp"; do
    light_assert_safe_cleanup_target "$target" "$LIGHT_ARTIFACTS_DIR" || return 1
  done
  for target in \
    "$LIGHT_ARTIFACTS_DIR/build" \
    "$LIGHT_ARTIFACTS_DIR/cache" \
    "$LIGHT_ARTIFACTS_DIR/generated" \
    "$LIGHT_ARTIFACTS_DIR/legacy" \
    "$LIGHT_ARTIFACTS_DIR/performance" \
    "$LIGHT_ARTIFACTS_DIR/release" \
    "$LIGHT_ARTIFACTS_DIR/test" \
    "$LIGHT_ARTIFACTS_DIR/tmp"; do
    [[ ! -e "$target" ]] || rm -rf -- "$target"
  done
  echo "Removed generated artifacts; preserved runtime at $LIGHT_RUNTIME_DATA_DIR and root-cleanup recovery under $LIGHT_ARTIFACTS_DIR/cleanup"
}

light_clean_runtime() {
  local confirmation="${1:-}"
  light_assert_safe_cleanup_target "$LIGHT_RUNTIME_DATA_DIR" "$LIGHT_ARTIFACTS_DIR" || return 1
  [[ "$confirmation" == "$LIGHT_RUNTIME_DATA_DIR" ]] || {
    echo "error: runtime cleanup includes local shows and desk state" >&2
    echo "Confirm with: npm run clean:artifacts -- runtime '$LIGHT_RUNTIME_DATA_DIR'" >&2
    return 1
  }
  [[ ! -e "$LIGHT_RUNTIME_DATA_DIR" ]] || rm -rf -- "$LIGHT_RUNTIME_DATA_DIR"
  echo "Removed development runtime data: $LIGHT_RUNTIME_DATA_DIR"
}
