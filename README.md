# Light

`light` is a headless-first show-lighting engine and control server. It uses portable SQLite show files, user-owned programmers shared across that user's devices, tracked cue lists, calibrated fixture attributes, and Art-Net/sACN network output.

## Run the server

```sh
cargo run -p light-server --bin light-server -- --data-dir ./light-data
```

Open `http://127.0.0.1:5000`. A new desk contains an enabled `Operator` user. Use `--bind 0.0.0.0:5000` only on a trusted control network.

Set `LIGHT_DESK_TOKEN` when exposing the server on a LAN. API clients then send the shared value in `X-Light-Desk-Token`; the embedded UI provides a desk-token field. This protects the desk boundary while usernames remain passwordless.

## Development and builds

```sh
./dev                    # server + Tauri app with UI hot reload
./build open             # debug builds, stop old instances, and open the app
./build archive          # self-contained server ZIPs for macOS, Windows, Linux AMD64, and Linux ARM64
./build archive install  # build archives and install/open ~/Applications/ToskLight.app
```

`./build archive` ships the web UI inside each `light-server` binary. It creates a
universal macOS binary plus Windows, Linux AMD64, and Linux ARM64 binaries in
`artifacts/`; Linux binaries are statically linked. Building the non-macOS
targets requires `zig`, `cargo-zigbuild`, and the Rust targets named by the
build script. The portable Linux binaries omit native USB-MIDI because it
depends on the target machine's ALSA library; RTP-MIDI, OSC, and network output
remain available.

Both local run commands store desk data in `./light-data` by default. Set `LIGHT_DATA_DIR` to use a different directory. The app talks to the server on `127.0.0.1:5000`; `./dev` restarts cleanly as one foreground environment, while backend source changes currently require restarting the command.

The server maintains:

- `desk.sqlite`: desk users, show-library index, active show, server settings, and durable session programmers.
- `shows/*.show`: portable, versioned SQLite show files.
- A fixed-deadline 44 Hz render scheduler with health counters exposed by `/api/v1/bootstrap` and `/api/v1/configuration`.

## API model

- REST under `/api/v1` provides sessions, bootstrap snapshots, show upload/download/open, revisioned show objects, patch inspection, programmer management, playback actions, and diagnostics.
- `/api/v1/media` exposes authenticated CITP media-server status, bounded thumbnail retrieval, and
  live-preview snapshots for fixture profiles that explicitly support direct IP control.
- Mutating versioned objects require `If-Match: <revision>` and return an `ETag`. Revision zero creates an object; stale revisions return HTTP 409.
- WebSocket `/api/v1/events` publishes ordered revisioned changes and accepts versioned, request-ID-bearing typed commands. REST remains the authoritative snapshot/recovery path after an event gap.
- A session authenticates a device as a configured user. Selection, command line, programmer values, blind/preview/highlight modes, editing context, and bounded undo/redo history belong to that user and are shared across their connected devices. Disconnected programmers remain present until explicitly cleared and survive server restart. New desks create an `Operator` user, and new devices select it unless a different user was remembered locally.

Show objects use the kinds `patched_fixture`, `cue_list`, and `route` for the live engine snapshot. Other kinds such as presets, groups, phasers, mappings, and user layouts use the same revisioned object store.

## Verification

All persisted-data changes are also governed by the [backward-compatibility acceptance criteria](docs/acceptance-criteria.md). A feature is not complete until legacy-file behavior is migrated and tested, or the compatibility requirement has been explicitly decided with the operator.

```sh
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p light-server --bin light-benchmark -- --universes 64 --seconds 5
cd apps/control-ui && npm run typecheck && npm test -- --run && npm run build && npm run test:e2e
```

The benchmark measures Art-Net and sACN frame encoding throughput for the selected universe count. Run it on each target, including Raspberry Pi 5, before choosing that desk's configured universe ceiling.

## Implementation status

Implemented foundations include fixture-library JSON/SQLite interchange, multi-head patching, 8–32-bit DMX encoding, calibrated XYZ emitter mixing, virtual dimmers, sparse tracked cues and cue-only restoration, HTP/LTP priority resolution, attribute dynamics sampling, immutable engine snapshots, live Art-Net/sACN UDP output, ArtTimeCode/MTC/OSC parsing, explicit timecode source fallback, CITP/MSEX thumbnail and live-preview transport, portable show backups, durable session-isolated programmers, REST/WebSocket control, and the standalone operator page.

The server includes native MIDI input, an Apple Network MIDI/RTP-MIDI transport subset, fade/follow/timecode playback, phasers in the render path, desk/input configuration, optional LAN boundary authentication, automatic retained backups, rollback transitions, and an operational responsive UI. USB DMX and DMX input intentionally remain extension points. Hardware-specific Raspberry Pi capacity still must be established by running the included benchmark on the target device.
