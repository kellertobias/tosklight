#!/usr/bin/env bash
# No real launch, process signaling, network calls or runtime filesystem writes.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/build-test-runtime.sh"
ROOT="/fixture/acceptance-worktree"
TARGET_DIR="$ROOT/.artifacts/build/cargo"
FIXTURE_LIBRARY_DIR="$ROOT/assets/fixture-library"
LIGHT_DESKTOP_TEST_BIND=127.0.0.1:5004
LIGHT_DESKTOP_TEST_OSC_BIND=127.0.0.1:19000
LIGHT_DESKTOP_TEST_DATA_DIR="$ROOT/.artifacts/runtime/test data"
CALLS=()
PORT_OCCUPIED=0
EMPTY_OWN=0
TEST_PROCESS_COMMAND=""
require() { :; }
launchctl() {
  if [[ "$1" == print ]]; then printf '  pid = 42\n'; else CALLS+=("launchctl $*"); fi
}
ps() {
  if [[ "$1" == ax ]]; then
    if [[ "$EMPTY_OWN" == 1 ]]; then
      printf '%s\n' "11 /other/worktree/light-headless --bind 127.0.0.1:5029"
      return
    fi
    printf '%s\n' \
      "11 /other/worktree/.artifacts/debug/light-headless --bind 127.0.0.1:5029" \
      "12 $TARGET_DIR/debug/light-headless --bind 127.0.0.1:5029 --data-dir /other/data" \
      "13 $TARGET_DIR/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-desktop" \
      "14 $TEST_SERVER_COMMAND" \
      "15 /other/ToskLight.app/Contents/MacOS/light-desktop"
  else printf '%s\n' "$TEST_PROCESS_COMMAND"; fi
}
kill() {
  [[ "$1" == -0 ]] && return 1
  CALLS+=("kill $*")
}
lsof() { [[ " $* " == *" -p 42 "* || "$PORT_OCCUPIED" == 1 ]]; }
curl() { CALLS+=("curl $*"); }
open() { CALLS+=("open $*"); }
mkdir() { CALLS+=("mkdir $*"); }

light_configure_desktop_test_runtime
[[ "$DATA_DIR" == "$LIGHT_DESKTOP_TEST_DATA_DIR" ]]
[[ "$TEST_SERVER_LABEL" == de.tokenet.tosklight.test-server.* ]]
light_stop_desktop_test_runtime
[[ "${CALLS[*]}" == "launchctl remove $TEST_SERVER_LABEL kill 13 kill 14" ]]
printf 'PASS exact own-path stopping preserves unrelated worktree and same-binary other bind\n'

CALLS=()
EMPTY_OWN=1
light_stop_desktop_test_runtime
[[ "${CALLS[*]}" == "launchctl remove $TEST_SERVER_LABEL" ]]
EMPTY_OWN=0
printf 'PASS first isolated launch with no owned PID is safe under Bash3 nounset\n'

CALLS=()
TEST_PROCESS_COMMAND="/other/server --bind 127.0.0.1:5004"
if light_desktop_test_server_ready; then exit 1; fi
[[ ${#CALLS[@]} == 0 ]]
TEST_PROCESS_COMMAND="$TEST_SERVER_COMMAND"
light_desktop_test_server_ready
[[ "${CALLS[*]}" == *'http://127.0.0.1:5004/api/v2/readiness'* ]]
printf 'PASS readiness requires exact own command, HTTP/OSC ownership and selected endpoint\n'

CALLS=()
light_start_desktop_test_runtime
[[ "${CALLS[*]}" == *"$TARGET_DIR/debug/light-headless --bind 127.0.0.1:5004 --osc-bind 127.0.0.1:19000 --data-dir $DATA_DIR --fixture-package-dir $FIXTURE_LIBRARY_DIR"* ]]
printf 'PASS server submission carries explicit HTTP, OSC, data and fixture paths\n'

CALLS=()
light_open_desktop_test_app
[[ "${CALLS[*]}" == *'open -n --env LIGHT_DESKTOP_TEST_BIND=127.0.0.1:5004 --env LIGHT_DESKTOP_TEST_OSC_BIND=127.0.0.1:19000 --env LIGHT_DESKTOP_TEST_DATA_DIR='* ]]
[[ "${CALLS[*]}" == *"$TARGET_DIR/debug/bundle/macos/ToskLight.app"* ]]
printf 'PASS fresh exact bundle receives test endpoint and data override\n'

CALLS=()
PORT_OCCUPIED=1
if light_start_desktop_test_runtime; then exit 1; fi
[[ ${#CALLS[@]} == 0 ]]
printf 'PASS occupied unrelated port is rejected before submit\n'

unset LIGHT_DESKTOP_TEST_OSC_BIND
if light_configure_desktop_test_runtime; then exit 1; fi
[[ ${#CALLS[@]} == 0 ]]
printf 'PASS incomplete overrides are rejected without process actions\n'
