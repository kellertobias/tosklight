---
slug: backend
title: Backend / Application
summary: "Axum adapters, the transport-independent application layer, wire contracts, and persistence."
order: 40
---

# Backend and Application Layer

`crates/server/`, `crates/application/`, `crates/wire/`, `crates/show/`. Rust 2024 edition,
`resolver = "3"`, workspace-wide `unsafe_code = "forbid"`.

## Layering

```
crates/server       adapters: Axum HTTP/WebSocket, OSC, Matter, media, files, scheduler
       ↓
crates/application  transport-independent use cases
       ↓
crates/{core, fixture, playback, programmer, output, control, show, media, mvr}
                    domain — no transport
```

`crates/wire` is a leaf: it depends on no workspace crate, and no domain crate depends on it.

`tools/check-architecture.mjs` enforces this against `cargo metadata`:

- `light-wire` may depend on no workspace crate.
- `light-application` must not depend on `light-wire`, `light-server`, or either UI crate.
- Any other `crates/*` except `light-server` must not depend on `light-application`, `light-wire`,
  or `light-server`.
- `light-server` must depend on both `light-application` and `light-wire`, as the composition root.

## crates/server

`src/main.rs` is a thin entry point and CI enforces it: at most 10 non-empty lines, must contain
`light_server::run().await`, must not mention `Router`, `AppState`, `TcpListener`, or
`tokio::spawn`. `src/lib.rs` is 8 lines.

| Path | Role |
| --- | --- |
| `src/runtime/bootstrap.rs` | Startup and background resources |
| `src/runtime/http_router.rs` | Composes feature routers |
| `src/runtime/output_scheduler.rs` | Process-owned timing loop |
| `src/runtime/event_transport/adapter.rs` | Typed domain events to wire messages |
| `src/command_http/` | v2 command-line HTTP API |
| `src/highlight/`, `src/matter/`, `src/file_manager*`, `src/help.rs`, `src/default_show/` | Feature adapters |
| `src/bin/light-benchmark.rs` | Release-only output benchmark |

`*_v2.rs`, `*_http.rs`, and `*_wire.rs` translate between wire DTOs and application
commands/events. `src/runtime/ws_*`, v1 route modules, and legacy event helpers are compatibility
code: keep them behaviour-compatible, put no new domain rules there.

Two more CI-enforced boundaries:

- `src/runtime/update_plans.rs` must not contain `.put_object(`, `refresh_command_show`, or
  `load_engine_snapshot`. Writes route through `ActiveShowService`.
- No `engine.playback()` or `playback_action_lock` in `crates/application/src` or
  `crates/server/src`.

## crates/application

Start at `src/lib.rs`, which exports the supported service surface.

| Path | Role |
| --- | --- |
| `action.rs` | `ActionContext`, `ApplicationCommand`, `ActionEnvelope<C>`, `ActionOutcome<T>`, `ActionError` |
| `event/` | Typed events, routes, filtered subscriptions, replay, gap detection, bounded queues, coalescing, rate limits |
| `active_show/` | The only boundary for active-show mutation; ordered backup, commit, install, reconcile, publish |
| `show_compiler/` | Migrated portable document to `EngineSnapshot` |
| `programming/` | Command line, selection, values, Preload, group/preset/cue recording |
| `playback/`, `playback_topology/` | Runtime actions and portable topology |
| `show_patch/` | Atomic batch `PatchFixtures` |
| `selective_import/`, `mvr_import/` | Cross-show and MVR ingest |
| `output_runtime/` | Grand Master, blackout, output control |
| `lossless_json.rs` | Typed before/after delta applied to raw JSON, so unknown fields survive |
| `managed_assets/`, `macro_runtime/`, `scheduling/`, `timeline/`, `fixture_position/` | Extension seams tested with fakes. Macros and timecode do not exist as products. |

Ports are injected via the `*Ports` trait family, using associated types where static dispatch fits.
`ShowPatchPorts` and `SelectiveShowImportPorts` are supertraits of `ActiveShowPorts`.

## crates/show

- `src/portable/` — lossless `.show` documents, atomically revised. `PortableShowDocument`,
  `PortableShowTransaction`, `PortableShowCommit`, `PortableShowRevision`.
- `src/desk/` — `<data-dir>/desk.sqlite`: users, clients and desks, screens, settings, show index
  and revisions, Programmer recovery checkpoints.
- `src/show_store.rs` — compare-and-swap writes, `mutate_objects_atomically`, borrowed
  `AtomicObjectWrite<'a>`.

## Adding a capability

1. Domain types and rules in the relevant domain crate.
2. A bounded command family and service in `crates/application/` with injected ports.
3. Typed events on the event bus.
4. Versioned DTOs in `crates/wire/src/v2/`, then regenerate contracts.
5. A thin adapter in `crates/server/` mapping DTO to command and event to wire.
6. A feature slice in the control UI.

`docs/engineering/extension-recipes.md` has the full recipe. Do not add a field to a shared struct
or a branch to a broad refresh path.

## Read first

1. `crates/application/src/lib.rs`
2. `crates/application/src/action.rs`
3. `crates/application/src/event/bus.rs`
4. `crates/application/src/active_show/ports.rs`
5. `crates/show/src/show_store.rs`
6. `crates/application/src/lossless_json.rs`
7. `crates/server/src/runtime/http_router.rs`
8. `crates/wire/src/v2/events.rs`
9. `tools/check-architecture.mjs`

## Verify

```sh
cargo fmt
./test architecture
./test unit
./test e2e-api
```
