<p align="center">
  <img src="apps/light-desktop/src-tauri/icons/icon.png" alt="ToskLight application icon" width="96" height="96">
</p>

<h1 align="center">ToskLight</h1>

> [!CAUTION]
> **ToskLight is not yet even a release candidate.** You are welcome to test the published releases, but the code is still unstable and may break with every new version.

`tosk-light` is a show-lighting desk, engine, and headless control application for programming fixtures, groups, cue lists, playback faders, and Art-Net/sACN output from one portable show file. The operator UI is built around a command line, live programmer, fixture sheet, 3D stage view, cue list pool, and playback section so the same show can be edited from the desktop app or browser-connected desks.

Start with the [quickstart help](docs/help/00-quickstart.markdown) or browse the full [operator help](docs/help).

![Light programming desk with fixture selection, group shortcuts, 3D stage preview, and live programmer](docs/help/assets/screenshots/default-desk-overview.png)

![Light Cuelist detail with playback faders and group masters](docs/help/assets/screenshots/cuelist-playback.png)

## Run Light headless

```sh
npm run dev
```

Open `http://127.0.0.1:5000`. A new desk contains an enabled `Operator` user. Use `--bind 0.0.0.0:5000` only on a trusted control network.

Set `LIGHT_DESK_TOKEN` when exposing the server on a LAN. API clients then send the shared value in `X-Light-Desk-Token`; the embedded UI provides a desk-token field. This protects the desk boundary while usernames remain passwordless.

## Development and builds

```sh
npm run dev                    # Light headless + Tauri app with UI hot reload
npm run open             # debug builds, stop old instances, and open the app
npm run manual           # PDF and deployable HTML manuals from docs/help Markdown
npm run bundle          # self-contained server ZIPs for macOS, Windows, Linux AMD64, and Linux ARM64
npm run bundle:install  # build archives and install/open ~/Applications/ToskLight.app
npm run migrate-artifacts # explicitly move legacy ./light-data into .artifacts/runtime/
npm run clean             # remove reproducible artifacts while preserving runtime data
```

The generated manuals are written to `.artifacts/generated/manual/pdf/tosklight-manual.pdf` and
`.artifacts/generated/manual/html/tosklight-manual/index.html`. The webhost-ready package is
`.artifacts/generated/manual/html/tosklight-manual-html.zip`; extract it directly into a document
root to deploy the single-page manual and its images.
Use `npm run test:help-screenshots` to intentionally refresh the application images
consumed by the Help window, PDF, and HTML manual. See the
[manual authoring guide](docs/help/99-Development/04-manual-and-help-screenshots.md) for the Markdown and screenshot
contract.

Repository-owned assets live under `assets/`. The transferable shipped
fixture packages are in `assets/fixture-library/`; a successful `npm run test:demo`
atomically refreshes the completed portable show at `assets/demo.show`.

`npm run bundle` ships the web UI inside each `light-headless` binary. It creates a
universal macOS binary plus Windows, Linux AMD64, and Linux ARM64 binaries in
`.artifacts/release/`; Linux binaries are statically linked. Building the non-macOS
targets requires `zig`, `cargo-zigbuild`, and the Rust targets named by the
build script. The portable Linux binaries omit native USB-MIDI because it
depends on the target machine's ALSA library; RTP-MIDI, OSC, and network output
remain available.

Both local run commands store desk data in `.artifacts/runtime/light-data/` by default. Existing `./light-data` state is never moved implicitly: run `npm run migrate-artifacts` once after reviewing the destination. If both locations contain data, the command stops without merging them. Set `LIGHT_DATA_DIR` to use a different directory. The app talks to the server on `127.0.0.1:5000`; `npm run dev` restarts cleanly as one foreground environment, while backend source changes currently require restarting the command.

All repository-local build products, manuals, release packages, test evidence, caches, and scratch files live below ignored `.artifacts/`. `npm run clean` removes only reproducible subtrees and preserves the active development runtime. Runtime removal is deliberately separate and prints the exact confirmation command because it includes local shows and desk state.

The server maintains:

- `desk.sqlite`: desk users, show-library index, active show, server settings, and durable session programmers.
- `shows/*.show`: portable, versioned SQLite show files.
- A fixed-deadline 44 Hz render scheduler with health counters exposed by `/api/v2/bootstrap`.

## Code tour for new developers

The repository ships a guided onboarding tour under [`.tour/`](.tour). Start it with
[CodeSafari](https://github.com/kellertobias/codesafari) — no install required:

```sh
npm run codesafari
npx --yes "@tobisk/codesafari@1.0.0" validate .
npm run pages:generate
```

The viewer opens a read-only IDE: a file tree, a source pane, and a step panel, so you can follow a
tour and still explore any file. `pages:generate` creates the responsive, deployable export under
`.artifacts/generated/pages/safari/`. It covers:

- **Tours** — Orientation; One Value from desk input to DMX; Cue Tracking and Goto; Ordered
  Selection; Value Spreading; the Portable Show; Add a Capability; Recording and Live References;
  Fixture Semantics; Playback Runtime; State Ownership to Pixels; and Rust/Tauri for TypeScript
  developers.
- **Components** — Control UI, app-local UI primitives, Tauri apps, Backend/Application, Engine &
  Output, Help Generator, and Testbench.
- **Glossary** — the operator vocabulary (Cue, Playback, Programmer, patch) and the architecture
  concepts (action context, projections, revisions, tick budget) you need before reading the code.

Every page is plain Markdown, so it is also readable directly on the file system or on the
repository host. `docs/engineering/` remains the authority for architecture rules; the tour links
into it rather than restating it.

## API model

- Typed REST under `/api/v2` provides sessions, bootstrap snapshots, show upload/download/open, revisioned show objects, patch inspection, programmer management, playback actions, and diagnostics.
- `/api/v2/media-servers` exposes authenticated CITP media-server status, bounded thumbnail retrieval, and
  live-preview snapshots for fixture profiles that explicitly support direct IP control.
- Mutating versioned objects require `If-Match: <revision>` and return an `ETag`. Revision zero creates an object; stale revisions return HTTP 409.
- WebSocket `/api/v2/events` publishes ordered filtered changes and accepts versioned, request-ID-bearing typed commands after subscription. REST remains the authoritative snapshot/recovery path after an event gap.
- A session authenticates a device as a configured user. Selection, command line, programmer values, blind/preview/highlight modes, editing context, and bounded undo/redo history belong to that user and are shared across their connected devices. Disconnected programmers remain present until explicitly cleared and survive server restart. New desks create an `Operator` user, and new devices select it unless a different user was remembered locally.

Show objects use the kinds `patched_fixture`, `cue_list`, and `route` for the live engine snapshot. Other kinds such as presets, groups, phasers, mappings, and user layouts use the same revisioned object store.

## Verification

[Build and test commands](docs/engineering/build-and-test-commands.md) documents every `npm run` dev, build, and test script, what `npm run test:architecture` enforces, and which check to run for which change.

All persisted-data changes are also governed by the [backward-compatibility acceptance criteria](docs/acceptance-criteria.md). A feature is not complete until legacy-file behavior is migrated and tested, or the compatibility requirement has been explicitly decided with the operator.

The repository does not currently ship a standalone UI package or Storybook. Reusable presentation
primitives live in `apps/light-desktop/src/components/common/` and
`apps/light-desktop/src/components/window-kit/`; their executable gates are the Control UI component
tests, typecheck/production build in `npm run test:unit`, and the real-browser coverage in
`npm run test:e2e-ui`.

```sh
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p light-headless --bin light-benchmark --no-default-features -- \
  --profile all --protocol artnet --transport encode-only --seconds 5 \
  --mutation-gate --hardware-label "machine model, CPU, RAM and power mode"
cd apps/light-desktop && npm run typecheck && npm test -- --run && npm run build && npm run test:e2e
```

The release-only benchmark emits JSON for the 32-universe/100 Hz hard floor, the
64-universe/120 Hz target, and both 4- and 8-universe/40 Hz low-power profiles. Each universe is
filled through the real Engine render, contribution arbitration, schema-v2 fixture projection, and
selected production protocol encoder. The scenario overlaps Playback, Programmer, static Group,
and phaser values; the phaser owns one mapped slot that has no static or Programmer value, and a
focused test proves consecutive logical ticks change that slot. Use `--protocol sacn` for the other
production codec and `--transport loopback` for separately reported, safe local UDP `send_to`
timing. Loopback is benchmark-owned and is not presented as production `NetworkOutput` socket
delivery. Each scenario preserves that ordinary scheduled pipeline as its floor measurement, then
reports an unpaced render-only diagnostic with four prebuilt sampled batches replacing a realistic
slice of Programmer and Playback assignments. The JSON explicitly identifies unavailable CPU, allocation,
sub-render phase, production socket, and sound-to-light measurements; do not infer those values from
total latency. Run it on each target, including Raspberry Pi-class hardware, before choosing that
desk's configured universe ceiling, and retain the JSON with the exact hardware label.
`--mutation-gate` additionally measures a cue edit through candidate compilation, runtime
preparation, and generation installation against paired 120- and 1,200-fixture projections; it
fails when untouched projections are rebuilt, the result diverges from the full compiler, or p95
latency scales with fixture count.

For the sustained complex-show acceptance run, use:

```sh
npm run benchmark:sustained-output -- --seconds 120 \
  --hardware-label "machine model, CPU, RAM, OS and power mode"
```

Use `--seconds 60` for the one-minute variant. This release run loads the shipped fixture packages
and patches 20 Showtec Sunstrip LED RGB 42206 in 30 Channel mode, 40 ROBE Robin 600X LEDWash in
Mode 1, 32 ROBE Robin DLS Profile in Mode 1, and 32 ROBE Robin LEDBeam 150 in Standard 16-bit mode.
Those fixtures occupy 4,288 slots. It fills the remaining 12,096 slots with 4,000 Generic RGB LED
fixtures in three-channel RGB virtual-dimmer mode and 24 in four-channel RGBD mode so fixtures do
not cross universe boundaries. The resulting 4,148-fixture show fills all 16,384 slots across 32
universes.

Every timed frame evaluates overlapping Group, Cue/Playback, phaser, and Programmer contributions
before both Art-Net and sACN encoding and UDP loopback delivery. The command schedules 125 Hz to
prove operational headroom above the 100 Hz floor and also reports sampled replacement
contributions as a separate render-only diagnostic. It retains JSON and stderr under
`.artifacts/performance`, prints the average frame rate and the minimum completed frame rate across
every one-second interval, and exits non-zero if the average or any interval falls below 100 Hz.
Dropped, deferred, and late frames remain explicit diagnostics.

## Implementation status

Implemented foundations include fixture-library JSON/SQLite interchange, multi-head patching, 8–32-bit DMX encoding, calibrated XYZ emitter mixing, virtual dimmers, sparse tracked cues and cue-only restoration, HTP/LTP priority resolution, attribute dynamics sampling, immutable engine snapshots, live Art-Net/sACN UDP output, ArtTimeCode/MTC/OSC parsing, explicit timecode source fallback, CITP/MSEX thumbnail and live-preview transport, portable show backups, durable session-isolated programmers, REST/WebSocket control, and the standalone operator page.

The server includes native MIDI input, an Apple Network MIDI/RTP-MIDI transport subset, fade/follow/timecode playback, phasers in the render path, desk/input configuration, optional LAN boundary authentication, automatic retained backups, rollback transitions, and an operational responsive UI. USB DMX and DMX input intentionally remain extension points. Hardware-specific Raspberry Pi capacity still must be established by running the included benchmark on the target device.
