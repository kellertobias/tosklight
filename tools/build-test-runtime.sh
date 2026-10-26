#!/usr/bin/env bash
# Isolated native acceptance launch helpers; sourced by build.sh, never launched alone.

light_desktop_test_runtime_requested() {
  [[ -n "${LIGHT_DESKTOP_TEST_BIND:-}${LIGHT_DESKTOP_TEST_OSC_BIND:-}${LIGHT_DESKTOP_TEST_DATA_DIR:-}" ]]
}

light_configure_desktop_test_runtime() {
  local name address port checksum
  for name in LIGHT_DESKTOP_TEST_BIND LIGHT_DESKTOP_TEST_OSC_BIND; do
    address="${!name:-}"
    if [[ ! "$address" =~ ^127\.0\.0\.1:([0-9]+)$ ]]; then
      echo "error: isolated launch requires $name=127.0.0.1:PORT" >&2
      return 1
    fi
    port="${BASH_REMATCH[1]}"
    if (( ${#port} > 5 )) || (( 10#$port < 1 || 10#$port > 65535 )); then
      echo "error: invalid isolated port in $name" >&2
      return 1
    fi
  done
  if [[ "${LIGHT_DESKTOP_TEST_DATA_DIR:-}" != /* ]]; then
    echo "error: isolated launch requires an absolute LIGHT_DESKTOP_TEST_DATA_DIR" >&2
    return 1
  fi
  if [[ -n "${LIGHT_DESKTOP_TEST_SHOW:-}" ]] && { [[ "$LIGHT_DESKTOP_TEST_SHOW" != /* ]] || [[ ! -f "$LIGHT_DESKTOP_TEST_SHOW" ]]; }; then
    echo "error: LIGHT_DESKTOP_TEST_SHOW must name an existing absolute show file" >&2
    return 1
  fi
  require launchctl
  require ps
  require lsof
  require cksum
  DATA_DIR="$LIGHT_DESKTOP_TEST_DATA_DIR"
  # This explicit test directory is not the default runtime migration destination.
  LIGHT_DATA_DIR_EXPLICIT=1
  TEST_HTTP_PORT="${LIGHT_DESKTOP_TEST_BIND##*:}"
  TEST_OSC_PORT="${LIGHT_DESKTOP_TEST_OSC_BIND##*:}"
  checksum="$(printf '%s' "$ROOT:$DATA_DIR:$LIGHT_DESKTOP_TEST_BIND" | cksum)"
  TEST_SERVER_LABEL="de.tokenet.tosklight.test-server.${checksum%% *}"
  TEST_SERVER_COMMAND="$TARGET_DIR/debug/light-headless --bind $LIGHT_DESKTOP_TEST_BIND --osc-bind $LIGHT_DESKTOP_TEST_OSC_BIND --data-dir $DATA_DIR --fixture-package-dir $FIXTURE_LIBRARY_DIR"
  if [[ -n "${LIGHT_DESKTOP_TEST_SHOW:-}" ]]; then TEST_SERVER_COMMAND+=" --show $LIGHT_DESKTOP_TEST_SHOW"; fi
}

light_stop_desktop_test_runtime() {
  echo "Stopping only this worktree's isolated acceptance runtime..."
  launchctl remove "$TEST_SERVER_LABEL" 2>/dev/null || true
  local pid command attempts=0 running app="$TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS"
  local own_pids=()
  while read -r pid command; do
    [[ "$pid" =~ ^[0-9]+$ ]] || continue
    case "$command" in
      "$app/light-desktop"|"$app/light-desktop "*|"$app/ToskLight"|"$app/ToskLight "*)
        own_pids+=("$pid")
        kill "$pid" 2>/dev/null || true ;;
      "$TEST_SERVER_COMMAND"|"$TEST_SERVER_COMMAND "*)
        own_pids+=("$pid")
        kill "$pid" 2>/dev/null || true ;;
      "$app/light-headless --data-dir $DATA_DIR "*)
        # Pad the command so bind must be a complete argument, wherever it appears.
        # The native bundled helper appends --osc-bind after --bind.
        if [[ " $command " == *" --bind $LIGHT_DESKTOP_TEST_BIND "* ]]; then
          own_pids+=("$pid")
          kill "$pid" 2>/dev/null || true
        fi ;;
    esac
  done < <(ps ax -o pid= -o command=)
  (( ${#own_pids[@]} > 0 )) || return 0
  while (( attempts < 100 )); do
    running=0
    for pid in "${own_pids[@]}"; do
      if kill -0 "$pid" 2>/dev/null; then running=1; fi
    done
    [[ "$running" == 0 ]] && return 0
    sleep 0.1
    attempts=$((attempts + 1))
  done
  echo "error: own isolated processes did not exit; refusing restart" >&2
  return 1
}

light_desktop_test_server_ready() {
  local details pid command
  details="$(launchctl print "gui/$(id -u)/$TEST_SERVER_LABEL" 2>/dev/null || true)"
  pid="$(sed -n 's/^[[:space:]]*pid = \([0-9][0-9]*\)$/\1/p' <<<"$details" | head -1)"
  [[ -n "$pid" ]] || return 1
  command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  [[ "$command" == "$TEST_SERVER_COMMAND" ]] || return 1
  lsof -nP -a -p "$pid" -iTCP:"$TEST_HTTP_PORT" -sTCP:LISTEN -t >/dev/null 2>&1 || return 1
  lsof -nP -a -p "$pid" -iUDP:"$TEST_OSC_PORT" -t >/dev/null 2>&1 || return 1
  curl -fsS "http://$LIGHT_DESKTOP_TEST_BIND/api/v2/readiness" >/dev/null 2>&1
}

light_start_desktop_test_runtime() {
  local server="$TARGET_DIR/debug/light-headless" attempts=0
  if lsof -nP -iTCP:"$TEST_HTTP_PORT" -sTCP:LISTEN -t >/dev/null 2>&1 || \
    lsof -nP -iUDP:"$TEST_OSC_PORT" -t >/dev/null 2>&1; then
    echo "error: an unrelated process occupies the selected test HTTP/OSC port; refusing isolated startup" >&2
    return 1
  fi
  mkdir -p "$DATA_DIR"
  local server_args=("$server" --bind "$LIGHT_DESKTOP_TEST_BIND" --osc-bind "$LIGHT_DESKTOP_TEST_OSC_BIND" --data-dir "$DATA_DIR" --fixture-package-dir "$FIXTURE_LIBRARY_DIR")
  if [[ -n "${LIGHT_DESKTOP_TEST_SHOW:-}" ]]; then server_args+=(--show "$LIGHT_DESKTOP_TEST_SHOW"); fi
  launchctl submit -l "$TEST_SERVER_LABEL" -o "$DATA_DIR/light-headless.log" -e "$DATA_DIR/light-headless.log" -- \
    "${server_args[@]}"
  while (( attempts < 600 )); do
    if light_desktop_test_server_ready; then return 0; fi
    sleep 0.1
    attempts=$((attempts + 1))
  done
  echo "error: own isolated server did not own readiness and both ports; see $DATA_DIR/light-headless.log" >&2
  launchctl remove "$TEST_SERVER_LABEL" 2>/dev/null || true
  return 1
}

light_open_desktop_test_app() {
  local app="$TARGET_DIR/debug/bundle/macos/ToskLight.app"
  light_desktop_test_server_ready || {
    echo "error: refusing isolated GUI launch without verified own headless service" >&2
    return 1
  }
  local name
  local args=(-n --env "LIGHT_DESKTOP_TEST_BIND=$LIGHT_DESKTOP_TEST_BIND"
    --env "LIGHT_DESKTOP_TEST_OSC_BIND=$LIGHT_DESKTOP_TEST_OSC_BIND"
    --env "LIGHT_DESKTOP_TEST_DATA_DIR=$DATA_DIR")
  # LaunchServices does not inherit this shell's benchmark environment. Forward only the
  # explicitly present controls needed by this isolated launch, preserving each argument.
  for name in LIGHT_DESKTOP_TEST_CAPTURE_DISPLAY LIGHT_DESKTOP_TEST_READY_FILE LIGHT_STAGE_PACKAGED_BENCH_REPORT LIGHT_STAGE_PACKAGED_BENCH_DISPLAY \
    LIGHT_STAGE_PACKAGED_BENCH_LAYOUT LIGHT_STAGE_PACKAGED_BENCH_PREPARED \
    LIGHT_STAGE_PACKAGED_BENCH_PROFILE LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS \
    LIGHT_STAGE_PACKAGED_BENCH_CONTROL_DURATION_SECONDS LIGHT_STAGE_PACKAGED_BENCH_STAGE \
    LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET LIGHT_STAGE_PACKAGED_BENCH_ADDITIONAL_STAGE_WINDOW \
    LIGHT_STAGE_PACKAGED_BENCH_QUALITY LIGHT_STAGE_PACKAGED_BENCH_VIEW LIGHT_STAGE_PACKAGED_BENCH_HELPER_HZ \
    LIGHT_STAGE_GPU_TIMING_CADENCE; do
    if [[ "${!name+x}" == x ]]; then args+=(--env "$name=${!name}"); fi
  done
  open "${args[@]}" "$app"
  echo "Isolated ToskLight opened at http://$LIGHT_DESKTOP_TEST_BIND; OSC $LIGHT_DESKTOP_TEST_OSC_BIND. Server log: $DATA_DIR/light-headless.log"
}
