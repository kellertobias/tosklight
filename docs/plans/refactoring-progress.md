# Major Refactoring Progress

This is the living handoff for [`major-refactoring.md`](major-refactoring.md). Update it after each
meaningful milestone. A checked item means the implementation is committed on `refactoring` and
has focused verification; it does not replace the final repository-wide acceptance run.

Last updated: 2026-07-18 at commit `54e7182`.

## Guardrails

- Preserve operator-visible UI, keypad, OSC, hardware, persistence, and desk-sharing behavior.
- Keep production files at or below 1,200 lines and production functions at or below 150 lines.
  Prefer files below 400 lines and short, single-purpose functions.
- Keep unrelated user changes out of refactoring commits.
- Measure the complete operator-visible path before declaring a performance problem fixed.
- Frontend mutations should use targeted optimistic state with rollback and authoritative event
  reconciliation. Views subscribe only to the capabilities and object identities they display.
- Public-boundary tests should read as operator intent while naming the exercised surface: visible
  software controls, command-line HTTP, or exact OSC.

## Completed and committed

- [x] Added source-size and dependency-direction architecture checks. The current tree has no
  production file above 1,200 lines and no production function above 150 lines.
- [x] Reduced the server executable to composition and lifecycle startup; routers and feature
  adapters live in the server library.
- [x] Added `light-application` and `light-wire`, checked-in schemas, generated TypeScript DTOs,
  typed action context, command outcomes, errors, and the bounded event bus.
- [x] Added command-line HTTP v2 and the first intent-level test helpers for Programmer steps,
  software/command-line/OSC surfaces, and Group storage.
- [x] Centralized active-show decoding, migration, validation, backup, CAS persistence, runtime
  preparation/install, adapter reconciliation, audit, and event publication.
- [x] Migrated Groups, Presets, Cuelists, Playbacks, Preload, Update, undo, output routes, Patch,
  and active-show MVR import through typed lossless transactions.
- [x] Added atomic batch patching, deduplicated portable profile revisions, legacy inline-profile
  migration, targeted Patch projections, optimistic Patch UI updates, race/idempotency coverage,
  large-profile coverage, and phase-oriented benchmark support.
- [x] Added selective-show-import planning and atomic application, including dependency closure,
  conflicts, identity/reference rewrites, fixture profile snapshots, managed assets, unknown data,
  compensation, and concurrent target/source revision checks.
- [x] Added scoped Show and Playback events with related-object routing, bounded queues,
  coalescing, rate limits, monotonic sequences, explicit gaps, and snapshot repair.
- [x] Migrated manual, automatic, scheduled, OSC, preload, current-page, and explicit-page
  Playback behavior into the typed application service and v2 runtime contract.
- [x] Added future-extension proofs for stateful/two-attribute/fixed contributions, external
  device intents, Macro runtime and audited HTTP, daily/one-time scheduling, monotonic clocks,
  managed assets, fixture-position commands, and timeline operations.
- [x] Added compiled fixture encoding, compiled Group membership, compiled Playback contribution
  histories, coherent runtime generations, output render benchmarks, and an external-device
  adapter seam without putting transport work in the render loop.
- [x] Split major responsibility hotspots including server runtime/composition, command transport,
  Core, Media/CITP, Highlight, MVR writer/import, Playback projections/adapters, Preload, event
  subscriptions, lossless JSON tests, and File Manager platform adapters.

## In progress

- [ ] Finish the scoped Show Objects frontend store. Current work covers active-view subscriptions,
  exact-object hydration, optimistic overlays, and selective-import reconciliation, but must also
  prove no cursor loss during replay/scope churn, recover from malformed events, keep derived
  Groups authoritative with partial caches, hydrate without WebSocket readiness, and isolate React
  publications from the global `ServerContext`.
- [ ] Converge global Grand Master and blackout mutations through a typed Output runtime service,
  event, and authoritative v2 snapshot while preserving legacy HTTP/WebSocket response shapes.
- [ ] Continue responsibility-based splits for remaining production files above the 400-line goal.

## Remaining architecture work

1. Complete vertical frontend slices for Playback, Programmer, Highlight, Output health, remaining
   Show capabilities, Patch, Screens, Files, and Configuration. Replace polling and broad bootstrap
   refreshes with narrow snapshots plus relevant event subscriptions.
2. Remove the remaining direct `Engine::playback()` mutable-lock exposure and the transitional
   `PlaybackService::operation_lock()` adapter path. Domain services should expose commands and
   immutable projections only.
3. Publish the remaining externally observable transitions once through typed events: Programmer
   ownership/value changes, Highlight movement, transition completion, output health/overload,
   and any remaining automatic runtime changes.
4. Add a typed server adapter and operator workflow for selective import. Migrate remaining layout
   and miscellaneous portable-show mutations, then remove generic frontend show-object mutation.
5. Replace production `useServer()` callers with feature-local stores/hooks. Remove broad global
   React update ownership, DOM/custom-event SET/Store/Update routing, and polling-based refreshes.
6. Modularize Hardware Controls into its OSC bridge, feedback reducer, controller hook, and
   Playback, Programmer, grid, and settings surfaces.
7. Expand the public test DSL and migrate remaining legacy command helpers. Tests must express the
   intended operator workflow and keep software, command-line, and OSC surfaces explicit rather
   than hiding meaningful parity behind one generic implementation shortcut.
8. Remove REST/WebSocket v1 and `useServer()` compatibility only after every production caller and
   acceptance test has moved to a typed replacement.
9. Add the final architecture overview, state-ownership matrix, code tour, extension recipes, test
   map, selective-import guide, and repaired feature-plan links under `docs/engineering`.

## Performance and acceptance still required

- Record warm release-build Patch measurements on documented reference hardware: one fixture
  below 250 ms server-side and 500 ms visible at p95, plus a 100-fixture batch below 500 ms.
- Record release output benchmarks for the 32-universe/100 Hz floor, 64-universe/120 Hz target,
  and 4-to-8-universe/40 Hz low-power goal. Include p50/p95/p99, missed ticks, CPU, allocations,
  pipeline phase timings, socket delivery, and sound-to-light accounting.
- Verify old shows, recovery from malformed/legacy active shows, Save As/export portability,
  layout data, unpatched fixtures, stored-empty Groups, ordered selections, Cue Phaser, Highlight,
  Preload, Update, Move in Black, route termination, shutdown, and first output after restart.
- Run the final formatting, Clippy, Rust workspace tests, frontend typecheck/tests/build, focused
  API/UI/OSC suites, unrestricted socket tests, desktop smoke, and authoritative `./build open`
  readiness/log/operator-path verification.

## Current verification snapshot

- Source-size ratchet: 0 production files above 1,200; 0 production functions above 150.
- Focused application, server, wire, frontend, architecture, source-size, MVR, File Manager,
  Playback, Preload, Patch, event, and strict Clippy checks have passed for their committed slices.
- A final full-suite and real desktop run has not yet been completed.

The remaining files above the 400-line goal currently include frontend transport/3D/setup/control
hotspots, Matter, the manual builder, large style sheets, and several test modules. Test files may
exceed the hard limits, but should still be split when it improves readability.
