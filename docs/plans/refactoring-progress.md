# Major Refactoring Progress

This is the living handoff for [`major-refactoring.md`](major-refactoring.md). Update it after each
meaningful milestone. A checked item means the implementation is committed on `refactoring` and
has focused verification; it does not replace the final repository-wide acceptance run.

Last updated: 2026-07-19 at commit `f8923fc`.

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
  software/command-line/OSC surfaces, and Group storage. The shared operator helper now parses one
  readable command sequence and preserves exact press/release feedback for real OSC input.
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
- [x] Completed the first scoped Show Objects frontend store: mounted views own kind or exact-object
  subscriptions, REST hydration is independent of WebSocket readiness, derived Groups track their
  transitive dependencies, optimistic mutations reconcile against authoritative events, gaps repair
  from snapshots without cursor loss, malformed events cannot poison reconnects, and unrelated
  global `ServerContext` consumers do not rerender.
- [x] Migrated the primary manual, automatic, scheduled, OSC, Preload, current-page, and
  explicit-page Playback action paths into the typed application service and v2 runtime contract.
  Virtual exclusion peers, startup normalization, and persisted topology are explicitly still in
  progress below.
- [x] Removed mutable Playback-service lock exposure from the migrated paths: Engine callers use
  typed commands and immutable projections, Preload installs generation-bound prepared batches,
  and application-owned units of work serialize page changes, automatic render transitions, and
  semantic event publication.
- [x] Added the view-scoped Playback runtime frontend: exact v2 snapshots and WebSocket filters are
  activated only by mounted Playback/Cuelist views, desk-only views request no runtime identities,
  and gaps and malformed messages repair from authoritative snapshots. Concurrent fader and page
  mutations use independent optimistic overlays with request-ordered rollback and authoritative
  event/outcome reconciliation. Active compatibility panes still poll until their consumers move.
- [x] Added a typed desk-local Programming interaction snapshot and stream. Command-line and
  ordered-selection changes use independent exact-object routes and non-empty sparse payloads;
  Highlight and Preload reconciliation finishes before the one authoritative event is captured.
  Command-only input compares lightweight revisions and does not clone or serialize the complete
  selection. Compatibility HTTP, OSC, and WebSocket paths retain their source behavior.
- [x] Exposed Selective Show Import through authenticated v2 catalog, preview, and atomic apply
  adapters with checked-in schemas, generated TypeScript, exact source/target revisions, strict
  response validation, and focused server contracts. **Show → Load → Partial Show Load** now uses a
  feature-owned capability provider to present dependencies, conflicts, profiles, managed assets,
  blockers, retryable preview work, and a non-cancellable one-revision apply without expanding the
  global server context.
- [x] Converged global Grand Master and blackout changes through the typed Output runtime service,
  with one batched persistence/event publication per control action and an authoritative v2
  snapshot while retaining legacy HTTP and WebSocket response compatibility.
- [x] Added future-extension proofs for stateful/two-attribute/fixed contributions, external
  device intents, Macro runtime and audited HTTP, daily/one-time scheduling, monotonic clocks,
  managed assets, fixture-position commands, and timeline operations.
- [x] Added compiled fixture encoding, compiled Group membership, compiled Playback contribution
  histories, coherent runtime generations, output render benchmarks, and an external-device
  adapter seam without putting transport work in the render loop.
- [x] Split major responsibility hotspots including server runtime/composition, command transport,
  Core, Media/CITP, Highlight, MVR writer/import, Playback projections/adapters, Preload, event
  subscriptions, lossless JSON tests, File Manager platform adapters, shared frontend controls,
  the Stage 3D scene/model/rendering pipeline, the frontend API client, fixture-profile modeling,
  PDF manual generation, global styles, and File Manager styles. Every in-scope production file is
  now at or below the 400-line design goal.
- [x] Modularized Hardware Controls into a dependency-injected OSC bridge and controller,
  idempotent feedback reducer, and focused Playback, Programmer, grid, and settings surfaces while
  preserving exact canonical and legacy OSC feedback behavior.
- [x] Added the architecture overview, state-ownership matrix, code tour, extension recipes, test
  map, refactoring test-boundary guide, frontend performance baseline, and Selective Show Import
  guide under `docs/engineering`.

## In progress

- [ ] Continue vertical feature-store/event slices and move the remaining production callers away
  from broad `useServer()`, polling, and generic show-object mutation.
- [ ] Finish the Playback ownership boundary for virtual exclusion peers, startup normalization,
  persisted Cuelist/topology mutation, and every active compatibility pane still polling.
- [ ] Adopt the sparse Programming stream in the frontend with independently reference-counted
  command-line and selection views, authoritative snapshot repair, and optimistic overlays.

## Remaining architecture work

1. Complete vertical frontend slices for Playback, Programmer, Highlight, Output health, remaining
   Show capabilities, Patch, Screens, Files, and Configuration. Replace polling and broad bootstrap
   refreshes with narrow snapshots plus relevant event subscriptions.
2. Publish the remaining externally observable transitions once through typed events: Programmer
   ownership/value changes, Highlight movement, transition completion, output health/overload,
   and any remaining automatic runtime changes.
3. Migrate remaining layout and miscellaneous portable-show mutations, then remove generic
   frontend show-object mutation.
4. Replace production `useServer()` callers with feature-local stores/hooks. Remove broad global
   React update ownership, DOM/custom-event SET/Store/Update routing, and polling-based refreshes.
5. Expand the public test DSL and migrate remaining legacy command helpers. Tests must express the
   intended operator workflow and keep software, command-line, and OSC surfaces explicit rather
   than hiding meaningful parity behind one generic implementation shortcut.
6. Remove REST/WebSocket v1 and `useServer()` compatibility only after every production caller and
   acceptance test has moved to a typed replacement.
7. Repair the remaining stale feature-plan links and keep the committed `docs/engineering` handoff
   synchronized as compatibility adapters are retired.

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

- Source-size ratchet: 0 production files above 1,200; 0 production functions above 150. The
  current full-tree design-goal report is 52 files above 400 lines and 3,394 functions above 20;
  the touched Programming live-state test was split back below 400 lines.
- In-scope production file goal: 0 files above 400 lines. The scanner still reports larger test and
  planning sources, plus the unrelated Dynamics Editor experiment.
- Focused application, server, wire, frontend, architecture, source-size, MVR, File Manager,
  Playback, Preload, Patch, Output, event, shared-control, Stage 3D, build, and strict Clippy checks
  have passed for their committed slices. The latest Playback frontend run covered 109 focused
  tests; the Selective Show Import frontend run covered 19 focused tests plus a production build.
- A final full-suite and real desktop run has not yet been completed.

The remaining files above the 400-line goal are planning/test sources and the unrelated Dynamics
Editor experiment. Test files may exceed the hard limits, but should still be split when it
improves readability and makes operator intent more visible.
