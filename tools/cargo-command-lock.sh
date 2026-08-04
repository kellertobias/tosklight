#!/usr/bin/env bash

# shellcheck shell=bash

light_cargo_lock_directory() {
  printf '%s\n' "$LIGHT_TMP_DIR/cargo-command.lock"
}

light_release_cargo_command_lock() {
  local lock_directory="$1" owner_file="$1/owner" owner_pid="" current_pid
  current_pid="${BASHPID:-$$}"
  if [[ -f "$owner_file" ]]; then
    IFS='|' read -r owner_pid _ < "$owner_file" || true
  fi
  [[ "$owner_pid" == "$current_pid" ]] || return 0
  rm -f -- "$owner_file"
  rmdir -- "$lock_directory" 2>/dev/null || true
}

light_cargo_owner_is_alive() {
  local owner_pid="$1"
  kill -0 "$owner_pid" 2>/dev/null && return 0
  # Sandboxed developer shells can deny signal probes across sibling command sandboxes even when
  # the process is alive. lsof provides a read-only fallback on macOS and avoids stealing its lock.
  command -v lsof >/dev/null 2>&1 && lsof -p "$owner_pid" -a -d cwd >/dev/null 2>&1
}

light_acquire_cargo_command_lock() {
  local label="$1" lock_directory owner_file owner_pid owner_label owner_started current_pid
  local waited=0 missing_owner_waits=0
  lock_directory="$(light_cargo_lock_directory)"
  owner_file="$lock_directory/owner"
  mkdir -p "$LIGHT_TMP_DIR"

  while ! mkdir "$lock_directory" 2>/dev/null; do
    owner_pid=""
    owner_label="unknown Cargo command"
    owner_started="unknown time"
    if [[ -f "$owner_file" ]]; then
      IFS='|' read -r owner_pid owner_label owner_started < "$owner_file" || true
      missing_owner_waits=0
    else
      missing_owner_waits=$((missing_owner_waits + 1))
      if (( missing_owner_waits >= 30 )) && rmdir "$lock_directory" 2>/dev/null; then
        continue
      fi
    fi

    if [[ "$owner_pid" =~ ^[0-9]+$ ]] && ! light_cargo_owner_is_alive "$owner_pid"; then
      rm -f -- "$owner_file"
      rmdir -- "$lock_directory" 2>/dev/null || true
      continue
    fi

    if (( waited == 0 || waited % 30 == 0 )); then
      if [[ "$owner_pid" =~ ^[0-9]+$ ]]; then
        echo "Waiting for Cargo: $owner_label (pid $owner_pid, started $owner_started)" >&2
      else
        echo "Waiting for another repository Cargo command to publish its owner..." >&2
      fi
    fi
    sleep 1
    waited=$((waited + 1))
  done

  current_pid="${BASHPID:-$$}"
  printf '%s|%s|%s\n' "$current_pid" "$label" "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" > "$owner_file"
  if (( waited > 0 )); then
    echo "Cargo lock acquired for $label after ${waited}s."
  fi
}

light_with_cargo_command_lock() (
  local label="$1" lock_directory
  shift
  lock_directory="$(light_cargo_lock_directory)"
  light_acquire_cargo_command_lock "$label"
  trap 'light_release_cargo_command_lock "$lock_directory"' EXIT INT TERM
  "$@"
)
