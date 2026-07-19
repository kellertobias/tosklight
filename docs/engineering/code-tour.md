# Code Tour

This tour follows an operator action from a surface to persistence or output. Paths are relative to
the repository root. Start with [Architecture overview](architecture-overview.md) for the rules
behind the layout.

## Root entry points

- `Cargo.toml` lists every Rust workspace member and shared dependencies.
- `build` is the supported build/package/manual/desktop entry point. `./build open` builds both
  Tauri applications and the server, starts the canonical development server, and opens ToskLight.
- `test` composes architecture, unit, Playwright, screenshot, and desktop-smoke workflows. See the
  [test map](test-map.md) before choosing a broad command.
- `tools/check-architecture.mjs` enforces Rust dependency direction, the thin server entry point,
  closed Playback ownership, and TypeScript wire-boundary imports.
- `tools/check-source-size.mjs` applies the source-size ratchet used during the refactor.

## Rust layers

### Stable domain crates

| Crate | Responsibility | Useful starting paths |
| --- | --- | --- |
| `crates/core` | Shared semantic identities, attributes, clocks, and small value types | `crates/core/src/attributes.rs`, `crates/core/src/clock.rs` |
| `crates/fixture` | Fixture profile/package/patch models, validation, and channel encoding | `crates/fixture/src/definition.rs`, `crates/fixture/src/profile.rs`, `crates/fixture/src/patch.rs` |
| `crates/programmer` | User Programmer, ordered selection, Groups, Presets, Preload, command state, and history | `crates/programmer/src/lib.rs`, `crates/programmer/src/registry.rs`, `crates/programmer/src/command_state.rs` |
| `crates/playback` | Cue models, tracking, active Playback runtime, automatic transitions, arbitration, and phasers | `crates/playback/src/lib.rs`, `crates/playback/src/model/`, `crates/playback/src/automatic.rs` |
| `crates/control` | Stable input actions and MIDI, OSC, RTP-MIDI, UDP, and timecode codecs/adapters | `crates/control/src/model.rs`, `crates/control/src/input.rs`, `crates/control/src/osc.rs` |
| `crates/output` | DMX frames, Art-Net/sACN encoding/delivery, routes, scheduler primitives, health, and the external-device port | `crates/output/src/frame.rs`, `crates/output/src/delivery/`, `crates/output/src/external.rs` |
| `crates/media` | CITP/media-server protocol, cache, and client models | `crates/media/src/protocol.rs`, `crates/media/src/client.rs` |
| `crates/mvr` | MVR archive model and writer | `crates/mvr/src/lib.rs`, `crates/mvr/src/writer.rs` |

Domain crates do not depend on `light-application`, `light-wire`, or `light-server`. Public methods
expose semantic commands and immutable projections, not transport DTOs or mutable locks.

### Persistence and compilation

`crates/show` owns two physically and semantically separate stores:

- `crates/show/src/portable/` reads and atomically revises lossless `.show` documents. Raw object
  bodies retain unknown fields; fixture-profile revisions are deduplicated by stable identity and
  digest.
- `crates/show/src/desk/` owns `<data-dir>/desk.sqlite`: users, clients/desks, screens, settings,
  show index/revisions, and Programmer recovery checkpoints.

`crates/application/src/show_compiler/` turns a migrated portable document into an
`EngineSnapshot`. `crates/application/src/active_show/` owns the ordered backup, commit, runtime
installation, reconciliation, and event lifecycle. Capability-specific mutations such as Patch,
MVR import, and Selective Show Import prepare transactions through their own application modules;
they do not write SQLite from a router.

### Application services

`crates/application` is transport-independent. Begin at `crates/application/src/lib.rs`, which
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

`crates/engine/src/engine.rs` owns coherent runtime generation installation. The rest of
`crates/engine/src/` separates contributions, resolution, transitions, fixture projection,
Playback commands/projections, visualization, and rendering. External stateful sources sample into
immutable `ContributionBatch` values; the engine still performs normal fixture/head-and-attribute
arbitration.

`crates/server/src/runtime/output_scheduler.rs` is the process-owned timing loop. It calls the
engine, publishes automatic semantic transitions after leaving domain locks, and sends encoded
routes through `light-output`. The release-only benchmark executable is
`crates/server/src/bin/light-benchmark.rs` with implementation under
`crates/server/src/bin/light_benchmark/`.

### Wire and server adapters

`crates/wire/src/v2/` contains versioned serialized DTOs only. `crates/wire/src/generation.rs`
generates JSON Schemas and `apps/control-ui/src/api/generated/light-wire.ts`. Never edit that
TypeScript file by hand; run:

```sh
cargo run -p light-wire --example generate-contracts
```

`crates/server/src/main.rs` is the thin executable. `crates/server/src/runtime/bootstrap.rs` owns
startup/background resources, and `crates/server/src/runtime/http_router.rs` composes feature
routers. Files named `*_v2.rs`, `*_http.rs`, `*_wire.rs`, and
`event_transport/adapter.rs` translate between wire DTOs and application commands/events.

Files under `crates/server/src/runtime/ws_*`, v1 route modules, and legacy event helpers are
compatibility adapters. Keep them behavior-compatible while callers migrate; do not put new domain
rules in them.

## Control UI

The control UI lives in `apps/control-ui`.

- `apps/control-ui/src/api/generated/light-wire.ts` is the checked-in generated transport contract.
- `apps/control-ui/src/api/client/`, wire decoder files in `apps/control-ui/src/api/`, and typed
  HTTP/WebSocket transports validate and map untrusted responses. Generated DTO imports are
  intentionally confined to this directory.
- `apps/control-ui/src/features/showObjects/`, `features/playbackRuntime/`, and `features/patch/`
  are the reference narrow store/session/transport/view slices. `features/files/`,
  `features/screens/`, `features/session/`, and `platform/desktop/` expose other bounded contexts.
- `apps/control-ui/src/components/` contains reusable controls, desk surfaces, setup views, modals,
  and shell composition. `apps/control-ui/src/windows/` contains pane/window features.
- `apps/control-ui/src/components/shell/` owns workspace layout and pane presentation; it must not
  become an authoritative show or runtime store.
- `apps/control-ui/src/platform/desktop/` defines `DesktopBridge` plus Tauri and browser adapters.

`apps/control-ui/src/api/ServerContext.tsx` and `apps/control-ui/src/features/server/` still compose
unmigrated capabilities. Treat them as temporary. A new capability belongs in a feature-local
store/hook and validated API adapter, not another field on `ServerContextValue` or another broad
refresh branch.

The native host is `apps/control-ui/src-tauri/`. It launches and supervises the sibling server,
owns native windows, and exposes only the typed desktop bridge needed by frontend code.

## Hardware Controls

`apps/hardware-controls/src/App.tsx` is a small composition root. Responsibility is divided into:

- `apps/hardware-controls/src/transport/oscBridge.ts` for the bridge port;
- `apps/hardware-controls/src/controller/feedbackReducer.ts` for pure idempotent feedback state;
- `apps/hardware-controls/src/controller/useHardwareController.ts` for subscription/lifecycle;
- `apps/hardware-controls/src/surfaces/` for Playback, Programmer, grid, and settings views; and
- `apps/hardware-controls/src-tauri/src/osc.rs` for native UDP OSC.

Preserve canonical and legacy OSC paths in `apps/hardware-controls/src/oscPaths.ts` and the public
server OSC adapters. Hardware Controls is a sibling application, not a hidden control-UI pane.

## Tests and operator contracts

- `docs/help/` is the operator manual source of truth.
- `docs/testing/` records human-readable acceptance contracts and stable IDs.
- `tests/` contains process-level Playwright acceptance coverage.
- `tests/support/operator/` contains intent-level helpers with explicit software, command-line, OSC,
  pool, and typed API surfaces.
- `apps/control-ui/e2e/bench/` owns process lifecycle, deterministic time, output receivers, OSC,
  UI drivers, and paired API/UI scenario registration.
- Rust integration tests live in each crate's `tests/` directory or feature-local server test
  modules; pure reducers and codecs retain adjacent unit tests.

The [test map](test-map.md) explains which boundary proves which kind of behavior.
