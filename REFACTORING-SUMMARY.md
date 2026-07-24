# ToskLight Refactoring Summary

## Executive summary

The major refactor moved ToskLight from a server-owned application with parallel transport rules
and a global frontend context to typed, capability-owned application services, lossless show
transactions, deterministic render/output boundaries, and narrow frontend projections.

The operator contract did not change: desk terminology and geometry, OSC paths and feedback,
software/hardware parity, persisted shows, unpatched fixtures, ordered selection, Programmer LTP,
Playback arbitration, Preload, Highlight, Move in Black, and output behavior remain compatibility
surfaces.

The final audit compares local `main` at `5c92eb07` with the refactoring work. That history touches
2,840 paths because it includes the complete application, fixture-library, generated-contract,
manual, and acceptance-test migration; the number is a historical snapshot, not an architectural
metric.

## Architecture before and after

| Before | After |
| --- | --- |
| `light-server` combined process startup, routes, orchestration, state, and business rules. | `crates/server/src/main.rs` is a thin entry point; feature adapters compose typed application services. |
| Rust wire models, handwritten TypeScript, generic JSON, string WebSocket commands, and OSC paths could drift. | `light-wire` owns versioned DTOs and checked-in generated TypeScript/schema contracts; adapters map to domain models. |
| UI, HTTP, WebSocket, OSC, and automatic sources could repeat orchestration. | One semantic action crosses one application boundary and produces one authoritative outcome/event. |
| Generic show-object writes mixed migration, persistence, compilation, runtime replacement, and publication. | `ActiveShowService` owns one ordered, revisioned, lossless transaction and install lifecycle. |
| Engine and application callers exposed mutable Playback/runtime state. | Commands and immutable projections cross crate boundaries; the render core consumes coherent snapshots and contribution batches. |
| `ServerProvider`/`useServer()` exposed broad state and refresh behavior to most of React. | A stable connection owner composes capability stores with narrow hydration, events, repair, and loading state. |
| Browser and Tauri details leaked into feature code. | `DesktopBridge` separates browser tests from Tauri hosts; Hardware Controls remains a sibling app. |

The living rules are in
[`docs/engineering/architecture-overview.md`](docs/engineering/architecture-overview.md),
[`architecture-boundaries.md`](docs/engineering/architecture-boundaries.md), and
[`state-ownership.md`](docs/engineering/state-ownership.md).

## Independently runnable components

| Component | Entry point | Contract |
| --- | --- | --- |
| Server | `crates/server/src/main.rs` / `light_server::run()` | Authenticates and adapts HTTP, WebSocket, OSC, Matter, media, file, and output work into application services. |
| Control UI | `apps/control-ui` | Main Tauri/React desk, secondary Screen and Stage windows, validated v2 transports, and feature projections. |
| Hardware Controls | `apps/hardware-controls` | Sibling Tauri app using the frozen OSC control and feedback contract. |
| Domain/application workspace | `crates/*` | Pure domain rules, use cases, lossless persistence, compilation, render, output codecs, and typed wire contracts. |
| UI package | `packages/ui` | Shared presentation primitives with its own package tests/build. |
| Help/manual generator | `docs/help` plus `tools/manual` | Markdown source rendered into in-app help, offline HTML, and PDF. |
| Acceptance bench | `apps/control-ui/e2e/bench` plus root `tests` | Isolated server/browser, virtual clock, OSC and output receivers, restart/fault controls, and paired API/UI scenarios. |

Supported commands and artifact paths are documented in
[`docs/engineering/build-and-test-commands.md`](docs/engineering/build-and-test-commands.md).

## Major changes

### Backend and application

- Added `light-application` with bounded Programming, Playback, Patch, active-show, output,
  selective-import, and related services.
- Added `ActionContext`, typed outcomes, request identity/replay, semantic events, filtered
  subscriptions, bounded delivery, coalescing, and gap repair.
- Reduced the server executable to composition and moved routers, lifecycle, scheduling, and
  adapters into feature-owned library modules.
- Retired every served `/api/v1` route. The desk live-control path uses correlated command frames
  on the v2 event WebSocket; integrator HTTP and OSC forms adapt to the same services.

### Persistence and show lifecycle

- Split desk installation storage from portable show storage explicitly.
- Preserved unknown objects and fields through load, migration, typed edits, revisions, Save As,
  export, and selective import.
- Centralized candidate decode/migration, validation, backup, CAS commit, compile, prepared runtime
  install, reconciliation, audit, and publication.
- Added an in-memory active-show document and interval-gated recovery checkpoints while retaining
  the small ordered SQLite WAL commit per mutation.
- Added typed show-library/object/patch actions, server-side fan-out and spread resolution,
  selective import, migration observability, and recording-aware undo.

### Engine and output

- Separated contribution production, arbitration, transitions, fixture projection, and DMX frame
  delivery behind immutable snapshots.
- Kept stateful sources outside the deterministic render core and published automatic transitions
  after releasing domain locks.
- Preserved fixture modes, logical-head identity, splits, multipatch, virtual intensity, fine-byte
  encoding, unpatched programming, and independent overlapping Group Master HTP.
- Added release benchmarks for render/output capacity and a mutation gate. Cue-object mutation now
  rebuilds only dirty compiled subgraphs and structurally shares untouched projections.

### Frontend

- Replaced the broad runtime provider and `LightApiClient` facade with stable connection ownership,
  explicit API capabilities, and scoped stores.
- Added authoritative snapshot hydration, narrow event subscriptions, optimistic overlays,
  response/event race handling, revision conflict repair, sequence-gap repair, authority
  replacement, and view dormancy.
- Removed production `useServer()` calls and broad v1 refresh paths.
- Added explicit boot, show-load, reconnect, and secondary-window loading/error presentation while
  retaining mounted same-show content during refreshes.

### Desktop and hardware

- Kept ToskLight and Hardware Controls as separately built sibling Tauri applications.
- Added typed desktop/browser bridge boundaries and stable primary/secondary window ownership.
- Preserved one shared desk command line and state across the main application and attached OSC
  hardware; different desk aliases remain isolated.
- Hardened app-owned server startup, readiness, process ownership, and desktop-smoke coverage.

### Tooling, tests, and documentation

- Added dependency-direction, thin-entry-point, ownership, source-size, generated-contract, and
  command-boundary ratchets.
- Standardized developer workflows under root `npm run` scripts and `.artifacts/`.
- Expanded paired API/UI, OSC, wire, restart, output, recovery, migration, desktop, and product-demo
  acceptance coverage.
- Added the engineering overview, state matrix, code tour, extension recipes, test map, and this
  CodeSafari handoff.

## Correctness, performance, and operator-visible fixes

The refactor also exposed and fixed defects that could not be left behavior-preserving:

- silent Cue migration revision churn and unpublished migration write-backs;
- independent overlapping Group Masters incorrectly suppressing each other;
- client-side two-point/multi-point spread divergence and missing software encoder THRU parity;
- Stage move/patch placement fan-out and whole-layout persistence;
- show-loading, reconnect, secondary-window, and scoped Patch authority gaps;
- frontend churn from provider polling, broad refresh, and unstable connection lifecycle;
- color-range writes falsely conflicting when one Rust `f32` had compact and widened JSON
  spellings; and
- size-dependent full-show compilation on Cue/object mutations.

The release mutation gate measured 120- and 1,200-fixture Cue mutations at about 1 ms p95 on the
2026-07-24 reference run, with full-compiler equivalence and structural sharing of untouched
projections.

## Compatibility decisions

- OSC is the frozen customer-facing protocol. Paths, aliases, key phases, feedback indices, and
  desk-sharing semantics remain exact.
- HTTP/WebSocket are internal/integrator transports and moved together to typed v2 contracts.
- Portable `.show` files and `desk.sqlite` remain separate. Legacy shows, fixture snapshots, and
  unknown data remain loadable and lossless.
- An unpatched fixture remains selectable, programmable, groupable, recordable, and visible; only
  physical output is suppressed.
- Stored-empty Groups, absent Groups, missing range IDs, and ordered membership remain distinct.
- Programmer values remain LTP. Playback intensity HTP and ownership rules were not generalized
  into the Programmer.
- Current-page and explicit-page Playback addressing remain separate and covered across software,
  API, OSC, and hardware paths.
- UI labels, key order, geometry, gestures, focus, and software/hardware layout parity remain
  operator contracts.

## Extension seams

Common changes are now local vertical slices:

- a new live capability adds a domain model, one bounded application service, typed wire adapter,
  semantic event/snapshot, narrow frontend store, and focused tests;
- a new portable object registers a lossless typed codec and Selective Show Import descriptor
  rather than adding a copy endpoint;
- a new runtime source contributes semantic fixture/head/attribute values without changing DMX
  drivers;
- a new output adapter consumes typed external-device intents without entering Programmer or Cue
  storage; and
- a new frontend pane activates only the projections it displays and becomes dormant when hidden.

See [`docs/engineering/extension-recipes.md`](docs/engineering/extension-recipes.md). Fake Dynamic,
Macro, scheduling, managed-asset, timeline, and external-device implementations prove boundaries;
they are not shipped product features.

## Verification snapshot

The final audit records exact reruns in
[`docs/plans/refactoring/done/27-wrap-refactor.md`](docs/plans/refactoring/done/27-wrap-refactor.md).
Evidence already established on the final pre-capstone tree includes:

- focused Programmer reconciliation tests: 35 passed;
- repeated color-range acceptance: 10 API and 10 UI repetitions passed;
- UI acceptance: 104 passed / 5 skipped;
- complete Playwright acceptance: 287 passed / 9 skipped;
- release show-mutation benchmark: below the 5 ms p95 gate with small/large fixture equivalence;
- Rust CITP loopback tests: 5 passed when local socket binding was allowed.

The nine intentional Playwright skips are explicit product gaps or conditional desktop cases:
DMX-008 on two surfaces, PRELOAD-002 UI, PRELOAD-004 supplemental UI, three MANUAL-019 UI
contracts, and two desktop cases that run separately under `npm run test:desktop-smoke`.

## Known risks and deliberately deferred work

- DMX-008 minimum-universe padding/default behavior and the named PRELOAD/MANUAL UI contracts remain
  product work, not hidden refactor failures.
- Dynamics, Macros, scheduled Macros, Timecode, managed assets, and bidirectional external fixtures
  remain future products. Only their architectural seams exist.
- The contract-only `ServerContext` test export and feature composition under
  `features/server/` are retained deliberately; neither is a broad runtime provider.
- Fixture-library, output, desktop, and network tests that bind sockets must run in an unrestricted
  CI/local environment; sandbox denial is reported separately.
- Historical refactoring documents retain old counts and migration terminology as execution
  evidence. Living rules are under `docs/engineering/`.

## Handoff map

- Living architecture: [`docs/engineering/`](docs/engineering/)
- Guided CodeSafari: [`.tour/`](.tour/)
- Refactor intent: [`docs/plans/major-refactoring.md`](docs/plans/major-refactoring.md)
- Ordered execution history: [`docs/plans/refactoring/done/`](docs/plans/refactoring/done/)
- Next acceptance-bench work: [`docs/plans/refactoring/pending/28-test-bench/README.md`](docs/plans/refactoring/pending/28-test-bench/README.md)
