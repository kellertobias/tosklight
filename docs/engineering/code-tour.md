# Code Tour

This tour follows an operator action from a surface to persistence or output. Paths are relative to
the repository root. Start with [Architecture overview](architecture-overview.md) for the rules
behind the layout.

## Root entry points

- `Cargo.toml` lists every Rust workspace member and shared dependencies.
- Root `package.json` is the supported build, test, package, manual, and desktop entry point.
  `npm run open` builds both Tauri applications and the server, starts the canonical development
  server, and opens ToskLight.
- `npm run test:*` composes architecture, unit, Playwright, and screenshot
  workflows. GitHub Actions additionally probes each newly built desktop
  application. See the [test map](test-map.md) before choosing a broad command.
- `tools/check-architecture.mjs` enforces Rust dependency direction, the thin server entry point,
  closed Playback ownership, and TypeScript wire-boundary imports.
- `tools/check-source-size.mjs` applies the source-size ratchet used during the refactor.

## Rust layers

### Stable domain crates

| Crate | Responsibility | Useful starting paths |
| --- | --- | --- |
| `crates/shared/core` | Shared semantic identities, attributes, clocks, and small value types | `crates/shared/core/src/attributes.rs`, `crates/shared/core/src/clock.rs` |
| `crates/shared/fixture` | Fixture profile/package/patch models, validation, and channel encoding | `crates/shared/fixture/src/definition.rs`, `crates/shared/fixture/src/profile.rs`, `crates/shared/fixture/src/patch.rs` |
| `crates/light/domain/programmer` | User Programmer, ordered selection, Groups, Presets, Preload, command state, and history | `crates/light/domain/programmer/src/lib.rs`, `crates/light/domain/programmer/src/registry.rs`, `crates/light/domain/programmer/src/command_state.rs` |
| `crates/light/domain/playback` | Cue models, tracking, active Playback runtime, automatic transitions, arbitration, and phasers | `crates/light/domain/playback/src/lib.rs`, `crates/light/domain/playback/src/model/`, `crates/light/domain/playback/src/automatic.rs` |
| `crates/light/domain/control` | Stable input actions and MIDI, OSC, RTP-MIDI, UDP, and timecode codecs/adapters | `crates/light/domain/control/src/model.rs`, `crates/light/domain/control/src/input.rs`, `crates/light/domain/control/src/osc.rs` |
| `crates/light/domain/output` | DMX frames, Art-Net/sACN encoding/delivery, routes, scheduler primitives, health, and the external-device port | `crates/light/domain/output/src/frame.rs`, `crates/light/domain/output/src/delivery/`, `crates/light/domain/output/src/external.rs` |
| `crates/light/adapters/media` | CITP/media-server protocol, cache, and client models | `crates/light/adapters/media/src/protocol.rs`, `crates/light/adapters/media/src/client.rs` |
| `crates/shared/mvr` | MVR archive model and writer | `crates/shared/mvr/src/lib.rs`, `crates/shared/mvr/src/writer.rs` |

Domain crates do not depend on `light-application`, `light-wire`, or `light-headless`. Public methods
expose semantic commands and immutable projections, not transport DTOs or mutable locks.

### Persistence and compilation

`crates/shared/show` owns two physically and semantically separate stores:

- `crates/shared/show/src/portable/` reads and atomically revises lossless `.show` documents. Raw object
  bodies retain unknown fields; fixture-profile revisions are deduplicated by stable identity and
  digest.
- `crates/shared/show/src/desk/` owns `<data-dir>/desk.sqlite`: users, clients/desks, screens, settings,
  show index/revisions, and Programmer recovery checkpoints.

`crates/light/src/show_compiler/` turns a migrated portable document into an
`EngineSnapshot`. `crates/light/src/active_show/` owns the ordered backup, commit, runtime
installation, reconciliation, and event lifecycle. Capability-specific mutations such as Patch,
MVR import, and Selective Show Import prepare transactions through their own application modules;
they do not write SQLite from a router.

### Application services

`crates/light` is transport-independent. Begin at `crates/light/src/lib.rs`, which
exports the supported service surface.

- `action.rs` defines `ActionContext`, bounded command families, outcomes, and errors.
- `event/` defines typed semantic events, stable routes, filtered subscriptions, replay, gap
  detection, bounded queues, coalescing, and rate limits.
- `programming/`, `playback/`, `output_runtime/`, `show_patch/`, `active_show/`,
  `selective_import/`, and `mvr_import/` own current use cases and their dependency-injected ports.
- `managed_assets/`, `macro_runtime/`, `scheduling/`, `timeline/`, and `fixture_position/` contain
  tested architecture seams. They do not mean the deferred Macro or Timecode products exist.

Application services may depend on domain crates. They must not depend on `light-wire`, HTTP,
WebSocket, Tauri, or a concrete database/network adapter.

### Engine and output

`crates/light/domain/engine/src/engine.rs` owns coherent runtime generation installation. The rest of
`crates/light/domain/engine/src/` separates contributions, resolution, transitions, fixture projection,
Playback commands/projections, visualization, and rendering. External stateful sources sample into
immutable `ContributionBatch` values; the engine still performs normal fixture/head-and-attribute
arbitration.

`crates/light/adapters/headless/src/runtime/output_scheduler.rs` is the process-owned timing loop. It calls the
engine, publishes automatic semantic transitions after leaving domain locks, and sends encoded
routes through `light-output`. The release-only benchmark executable is
`crates/light/adapters/headless/src/bin/light-benchmark.rs` with implementation under
`crates/light/adapters/headless/src/bin/light_benchmark/`.

### Wire and server adapters

`crates/light/contracts/wire/src/v2/` contains versioned serialized DTOs only. `crates/light/contracts/wire/src/generation.rs`
generates JSON Schemas and `apps/light-desktop/src/api/generated/light-wire.ts`. Never edit that
TypeScript file by hand; run:

```sh
cargo run -p light-wire --example generate-contracts
```

`crates/light/adapters/headless/src/main.rs` is the thin executable. `crates/light/adapters/headless/src/runtime/bootstrap.rs` owns
startup/background resources, and `crates/light/adapters/headless/src/runtime/http_router.rs` composes feature
routers. Files named `*_v2.rs`, `*_http.rs`, `*_wire.rs`, and
`event_transport/adapter.rs` translate between wire DTOs and application commands/events.

The v2 event transport also accepts correlated command frames so the desk UI keeps one ordered live
connection. OSC, integrator HTTP actions, and desktop bridges are deliberate adapters; none owns a
second copy of domain rules.

## Control UI

The control UI lives in `apps/light-desktop`.

- `apps/light-desktop/src/api/generated/light-wire.ts` is the checked-in generated transport contract.
- `apps/light-desktop/src/api/client/`, wire decoder files in `apps/light-desktop/src/api/`, and typed
  HTTP/WebSocket transports validate and map untrusted responses. Generated DTO imports are
  intentionally confined to this directory.
- `apps/light-desktop/src/features/showObjects/`, `features/playbackRuntime/`, and `features/patch/`
  are the reference narrow store/session/transport/view slices. `features/files/`,
  `features/screens/`, `features/session/`, and `platform/desktop/` expose other bounded contexts.
- `apps/light-desktop/src/components/` contains reusable controls, desk surfaces, setup views, modals,
  and shell composition. `apps/light-desktop/src/windows/` contains pane/window features.
- `apps/light-desktop/src/components/shell/` owns workspace layout and pane presentation; it must not
  become an authoritative show or runtime store.
- `apps/light-desktop/src/platform/desktop/` defines `DesktopBridge` plus Tauri and browser adapters.

`apps/light-desktop/src/api/ServerContext.ts` is retained only as a narrow contract for legacy test
mocks. `apps/light-desktop/src/features/server/` composes focused capabilities and shared connection
infrastructure; it no longer exposes `useServer()`. A new capability belongs in a feature-local
store/hook and validated API adapter, not a broad context or refresh branch.

The native host is `apps/light-desktop/src-tauri/`. It launches and supervises the sibling server,
owns native windows, and exposes only the typed desktop bridge needed by frontend code.

## Hardware Controls

`apps/light-hardware-controls/src/App.tsx` is a small composition root. Responsibility is divided into:

- `apps/light-hardware-controls/src/transport/oscBridge.ts` for the bridge port;
- `apps/light-hardware-controls/src/controller/feedbackReducer.ts` for pure idempotent feedback state;
- `apps/light-hardware-controls/src/controller/useHardwareController.ts` for subscription/lifecycle;
- `apps/light-hardware-controls/src/surfaces/` for Playback, Programmer, grid, and settings views; and
- `apps/light-hardware-controls/src-tauri/src/osc.rs` for native UDP OSC.

Preserve canonical and legacy OSC paths in `apps/light-hardware-controls/src/oscPaths.ts` and the public
server OSC adapters. Hardware Controls is a sibling application, not a hidden control-UI pane.

## Tests and operator contracts

- `docs/help/` is the operator manual source of truth.
- `docs/testing/` records human-readable acceptance contracts and stable IDs.
- `tests/` contains process-level Playwright acceptance coverage.
- `tests/support/operator/` contains intent-level helpers with explicit software, command-line, OSC,
  pool, and typed API surfaces.
- `tests/bench/` owns process lifecycle, deterministic time, output receivers, OSC,
  UI drivers, and paired API/UI scenario registration.
- Rust integration tests live in each crate's `tests/` directory or feature-local server test
  modules; pure reducers and codecs retain adjacent unit tests.

The [test map](test-map.md) explains which boundary proves which kind of behavior.
