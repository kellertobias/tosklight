<p align="center">
  <img src="apps/control-ui/src-tauri/icons/icon.png" alt="ToskLight application icon" width="96" height="96">
</p>

<h1 align="center">ToskLight</h1>

`tosk-light` is a show-lighting desk, engine, and control server for programming fixtures, groups, cue lists, playback faders, and Art-Net/sACN output from one portable show file. The operator UI is built around a command line, live programmer, fixture sheet, 3D stage view, cue list pool, and playback section so the same show can be edited from the desktop app or browser-connected desks.

Start with the [quickstart help](docs/help/00-quickstart.markdown) or browse the full [operator help](docs/help).

![Light programming desk with fixture selection, group shortcuts, 3D stage preview, and live programmer](docs/help/assets/screenshots/default-desk-overview.png)

![Light Cuelist detail with playback faders and group masters](docs/help/assets/screenshots/cuelist-playback.png)

## Run the server

```sh
./dev
```

Open `http://127.0.0.1:5000`. A new desk contains an enabled `Operator` user. Use `--bind 0.0.0.0:5000` only on a trusted control network.

Set `LIGHT_DESK_TOKEN` when exposing the server on a LAN. API clients then send the shared value in `X-Light-Desk-Token`; the embedded UI provides a desk-token field. This protects the desk boundary while usernames remain passwordless.

## Development and builds

```sh
./dev                    # server + Tauri app with UI hot reload
./build open             # debug builds, stop old instances, and open the app
./build manual           # PDF and deployable HTML manuals from docs/help Markdown
./build archive          # self-contained server ZIPs for macOS, Windows, Linux AMD64, and Linux ARM64
./build archive install  # build archives and install/open ~/Applications/ToskLight.app
./build migrate-artifacts # explicitly move legacy ./light-data into .artifacts/runtime/
./build clean             # remove reproducible artifacts while preserving runtime data
```

The generated manuals are written to `.artifacts/generated/manual/pdf/tosklight-manual.pdf` and
`.artifacts/generated/manual/html/tosklight-manual/index.html`. The webhost-ready package is
`.artifacts/generated/manual/html/tosklight-manual-html.zip`; extract it directly into a document
root to deploy the single-page manual and its images.
Use `./test help-screenshots` to intentionally refresh the application images
consumed by the Help window, PDF, and HTML manual. See the
[manual authoring guide](docs/help/99-Development/04-manual-and-help-screenshots.md) for the Markdown and screenshot
contract.

Repository-owned assets live under `assets/`. The transferable shipped
fixture packages are in `assets/fixture-library/`; a successful `./test demo`
atomically refreshes the completed portable show at `assets/demo.show`.

`./build archive` ships the web UI inside each `light-server` binary. It creates a
universal macOS binary plus Windows, Linux AMD64, and Linux ARM64 binaries in
`.artifacts/release/`; Linux binaries are statically linked. Building the non-macOS
targets requires `zig`, `cargo-zigbuild`, and the Rust targets named by the
build script. The portable Linux binaries omit native USB-MIDI because it
depends on the target machine's ALSA library; RTP-MIDI, OSC, and network output
remain available.

Both local run commands store desk data in `.artifacts/runtime/light-data/` by default. Existing `./light-data` state is never moved implicitly: run `./build migrate-artifacts` once after reviewing the destination. If both locations contain data, the command stops without merging them. Set `LIGHT_DATA_DIR` to use a different directory. The app talks to the server on `127.0.0.1:5000`; `./dev` restarts cleanly as one foreground environment, while backend source changes currently require restarting the command.

All repository-local build products, manuals, release packages, test evidence, caches, and scratch files live below ignored `.artifacts/`. `./build clean` removes only reproducible subtrees and preserves the active development runtime. Runtime removal is deliberately separate and prints the exact confirmation command because it includes local shows and desk state.

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
cargo run --release -p light-server --bin light-benchmark --no-default-features -- \
  --profile all --protocol artnet --transport encode-only --seconds 5 \
  --hardware-label "machine model, CPU, RAM and power mode"
cd apps/control-ui && npm run typecheck && npm test -- --run && npm run build && npm run test:e2e
```

The release-only benchmark emits JSON for the 32-universe/100 Hz hard floor, the
64-universe/120 Hz target, and both 4- and 8-universe/40 Hz low-power profiles. Each universe is
filled through the real Engine render, contribution arbitration, schema-v2 fixture projection, and
selected production protocol encoder. The scenario overlaps Playback, Programmer, static Group,
and phaser values; the phaser owns one mapped slot that has no static or Programmer value, and a
focused test proves consecutive logical ticks change that slot. Use `--protocol sacn` for the other
production codec and `--transport loopback` for separately reported, safe local UDP `send_to`
timing. Loopback is benchmark-owned and is not presented as production `NetworkOutput` socket
delivery. The JSON explicitly identifies unavailable CPU, allocation, sub-render phase, production
socket, and sound-to-light measurements; do not infer those values from total latency. Run it on
each target, including Raspberry Pi-class hardware, before choosing that desk's configured universe
ceiling, and retain the JSON with the exact hardware label.

## Implementation status

Implemented foundations include fixture-library JSON/SQLite interchange, multi-head patching, 8–32-bit DMX encoding, calibrated XYZ emitter mixing, virtual dimmers, sparse tracked cues and cue-only restoration, HTP/LTP priority resolution, attribute dynamics sampling, immutable engine snapshots, live Art-Net/sACN UDP output, ArtTimeCode/MTC/OSC parsing, explicit timecode source fallback, CITP/MSEX thumbnail and live-preview transport, portable show backups, durable session-isolated programmers, REST/WebSocket control, and the standalone operator page.

The server includes native MIDI input, an Apple Network MIDI/RTP-MIDI transport subset, fade/follow/timecode playback, phasers in the render path, desk/input configuration, optional LAN boundary authentication, automatic retained backups, rollback transitions, and an operational responsive UI. USB DMX and DMX input intentionally remain extension points. Hardware-specific Raspberry Pi capacity still must be established by running the included benchmark on the target device.
