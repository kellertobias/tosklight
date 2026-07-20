# Major Refactoring Progress

Estimated progress: **81%**

Estimated ETA: **7–11 focused implementation slices, or roughly 2–3 weeks of active
refactoring**, to repository-wide acceptance.

This is the living handoff for [`major-refactoring.md`](major-refactoring.md). Update it after each
meaningful milestone. A checked item means the implementation is committed on `refactoring` and
has focused verification; it does not replace the final repository-wide acceptance run.

Last updated: 2026-07-20 after completing the capture-safe pending-Preload values contract and
migrating the parameter-control value cohort.

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
  selection. The frontend now has strict feature-owned decoders, exact capability subscriptions,
  independently revisioned command-line and selection state, ordered optimistic overlays, and
  narrow snapshot repair. It remains dormant until the production consumers move below, so merely
  mounting the global provider performs no request and opens no socket. Compatibility HTTP, OSC,
  and WebSocket paths retain their source behavior.
- [x] Added strict, revisioned Programmer selection actions for replacement, accumulated gestures,
  live or frozen Group selection, and selection rules. The service expands logical heads, validates
  exact fixture/Group dependencies, retains idempotent warnings, publishes complete authority, and
  serializes environment resolution with active-show installation. The frontend uses a strict FIFO
  optimistic writer with one safe network retry, rollback or narrow repair by failure class,
  show/desk generation guards, and an execution barrier shared with command-line Enter. Capability
  narrowing now reopens its scoped stream from the existing cursor without another REST snapshot.
- [x] Added one authenticated application boundary for adapter-owned Programmer interactions. It
  serializes with typed commands per desk, captures the final command/selection state even when
  persistence fails after an in-memory mutation, and publishes before releasing the gate. The
  compatibility WebSocket path now uses this boundary; capture-mode reconciliation is driven by a
  lightweight coherent version, Preload GO follows activation → Programming → Playback ordering,
  unrelated Output/Playback commands no longer take the Programming gate, and the superseded
  public Programming unit-of-work escape hatch is removed.
- [x] Routed direct Highlight HTTP/OSC, the legacy Programmer value write, and selection-capable
  Playback v1/v2/OSC input through that ordered Programming boundary. Real selection changes now
  publish one authoritative source-scoped event, including the final gesture state; releases,
  Highlight ON/OFF, request replays, and already-current reconciliation publish none. Highlight
  status retains its locked-desk repair behavior while mutating actions remain lock-protected.
- [x] Routed active-show, Patch, selective-import, and show-activation runtime installations through
  one engine-generation selection refresh. It compares resolved Group membership rather than
  unrelated Group metadata, locks changed desk projections in UUID order, and publishes one final
  correlated selection event per changed desk before the owning Show event. Nested command
  mutations exclude their already-held actor gate from the peer lock set while still publishing
  that actor in UUID event order; the outer Programming boundary suppresses the already-sent
  selection component. Focused two-desk Group PUT/undo and software `RECORD GROUP 1` tests prove
  exact event counts, final LiveGroup rules, stable correlation, Show-last ordering, and absence of
  actor re-lock deadlocks.
- [x] Replaced high-rate Group-master whole-snapshot replacement with a topology-invariant engine
  update. It atomically swaps only the snapshot, resolved Group map, and compiled master index while
  sharing Playback and every unrelated compiled generation component, so fader movement neither
  rebuilds Playback nor refreshes live Programmer selections. Legacy WebSocket and Matter writes
  now respect show activation, reject stale-show races, persist only real changes, and retain the
  existing Playback/output compatibility responses.
- [x] Migrated the production command-line editors and action consumers onto the scoped
  Programming store. A provider-owned latest-wins writer gives immediate optimistic feedback,
  bounds slow writes to one in flight plus the newest pending value, waits for accepted writes
  before Enter, gates post-Enter edits until narrow snapshot reconciliation, and repairs conflicts
  without rebasing over OSC or another desk surface. Stream and mutation errors have independent
  ownership. Mounted views activate only the command-line and selection capabilities they use;
  action-only consumers do not rerender for command text. Exact categorized edit events no longer
  trigger broad bootstrap hydration, while malformed, expanded, and runtime changes retain the
  compatibility fallback. Edit persistence now occurs inside the replay boundary, so a failed
  save cannot replay as a false success. The temporary Cue-choice modal remains tied to the
  explicit execute response so it cannot appear before ENT, and scoped Cancel performs one v2
  reset while dismissing that compatibility response locally.
- [x] Migrated the complete Stage selection path, Stage command controls, and Stage/Fixture pane
  counts onto the ordered Programming selection projection. Covered panes do not hydrate or
  subscribe, peer and OSC changes update the mounted view without a legacy reload, Stage gestures
  retain FIFO optimistic accumulation and rollback, and patch Highlight previews resolve selected
  logical heads to their parent fixture without changing Stage visualization polling.
- [x] Migrated Channels, Fixture Sheet, Presets, and the Patch DMX preview onto the scoped ordered
  selection projection. Channels and Fixture Sheet update immediately through typed optimistic
  actions, Patch subscribes only while its active preview needs selected fixtures, inactive panes
  perform no snapshot or subscription work, and none of these views falls back to stale global
  selection while scoped authority is loading.
- [x] Added the transport-neutral user-scoped Programmer-values frontend core. Its deterministic
  immutable projection, external store, request-keyed optimistic overlays, and reference-counted
  session isolate authority by show and user; mounted values views alone trigger hydration and a
  stream, cursor gaps repair from snapshots, late work cannot cross a scope reset, and selectors
  suppress unrelated rerenders. The production wire adapter and value consumers remain below.
- [x] Added the authoritative user-scoped normal Programmer-values backend boundary. A cheap
  per-user mutation generation avoids cloning values for command-only or no-op actions; one
  completed semantic mutation advances one public revision, materializes one deterministic full
  retained projection, and publishes one replaceable event shared by every desk for that user.
  Same-user mutations serialize before desk interaction locks, snapshots and explicit/broad/rate-
  limited subscriptions enforce authenticated user ownership, and v1 compatibility events report
  interaction/value categories from the transitions that actually occurred. Selection, Preload,
  modes, priority, connectivity, Highlight, and transient actions remain outside this projection.
- [x] Completed the normal Programmer-values mutation and production client contract. Typed fixture
  and Group set/release, batch, and clear actions cross the Programming application boundary as one
  unit and return request/correlation identity, replay state, changed/no-change state, authoritative
  projection/revision, and the event sequence only when emitted. The exact-user REST/WebSocket
  adapter, view-scoped provider, FIFO optimistic writer, and committed store handle either
  response/event order, rollback, replay, no-change results, cursor repair, authority replacement,
  and late responses. Action-only consumers stay dormant; the first mounted values view performs
  the narrow snapshot/subscription, and scoped events neither request bootstrap nor rerender an
  unrelated global consumer. A separate exact-user capture-mode projection now supplies the atomic
  revision precondition that prevents a normal write from crossing into active Preload capture.
  Capture changes, Programmer replacement, and session recreation preserve monotonic authority,
  invalidate stale replay entries, and publish at most one values event and one capture event per
  real transition without admitting modes or Preload content into the values projection. The
  frontend decoders reject undeclared fields at every values snapshot, outcome, error, projection,
  value, and event-envelope boundary. Provider disposal is safe under React StrictMode effect
  replay, so reused values/capture sessions and the values writer remain live after the development
  mount cycle while replaced authorities still stop promptly.
- [x] Added a distinct exact-user pending-Preload values authority instead of routing typed normal
  actions through a capture-mode race. Fixture and Group set/release plus ordered batch actions
  carry both pending-values and capture-mode revision preconditions, share the Programming
  application gate, preserve timing and Programmer order, and emit one retained projection/event
  per real transition. Same-user desks share authority; another user cannot read, mutate, or
  subscribe to it. The strict frontend provider remains dormant until capture is authoritatively
  active and a values view mounts, then reconciles response/event ordering, replay, rollback,
  no-change, gaps, scope replacement, and late responses without a bootstrap fallback. Normal and
  pending fixture batches now use borrowed application slices and one indexed domain pass; the
  10,000-mutation limit is covered directly. Both values replay caches retain fixed SHA-256 request
  fingerprints and enforce a conservative 16 MiB projection/outcome budget in addition to their
  entry cap.
- [x] Migrated every parameter-bank family, fader, encoder, range, release, and direct action onto
  the scoped ordered selection projection. Fixture membership uses sets, streamed peer or OSC
  selection immediately retargets writes, and inactive parameter views perform no selection
  hydration, visualization polling, or hardware-listener work. The subsequent value cohort now
  routes those recordable gestures through either normal or pending-Preload scoped authority.
- [x] Migrated parameter faders, software and hardware encoders, range entry, release, and direct
  fixed/indexed choices onto one typed ordered batch per gesture. Capture loading exposes neither
  stale bootstrap values nor a writable route; normal and pending-Preload views are mutually
  exclusive. Hardware deltas accumulate relatively across slow responses and reset at target or
  authority changes, while continuous writes retain only the latest pending value per target and
  range/release/direct work remains an ordered barrier. Transient fixture-control actions stay on
  their independent non-recordable path.
- [x] Migrated Special Dialogs and System Controls selection onto visibility-scoped streams. The
  ordered projection is passed explicitly through Color, Position, Beam/Shapers, Dynamics, Control,
  and Lamp On helpers without per-interaction copies; external changes update open modals, closed
  modals do no hydration and tear down their stream, and System Controls retains its separate
  compatibility list of all active Programmers pending the lifecycle slice.
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
- [ ] Move the remaining selection consumers onto the scoped Programming store, then remove their
  legacy bootstrap fields and broad Programmer refresh paths. Group Pool, Group Strip, and the
  command bar, Stage, Stage/Fixture pane chrome, Channels, Fixture Sheet, Patch, and Presets have
  moved, as have the complete parameter bank and selection-driven operator modals; Patch setup and
  a small number of keypad/miscellaneous readers still use the facade.
- [ ] Migrate production normal Programmer-value readers and writers onto the connected scoped
  values provider in focused cohorts, then remove only their value-triggered bootstrap refreshes.
  Parameter controls have moved; the command-bar activity summary, System Controls current-user
  count, and compatibility facade retirement remain separate follow-up work.
- [ ] Replace inferred Cue ambiguity in the command-line text projection with explicit desk-local
  pending-choice state that is set only by `ChoiceRequired` after ENT and cleared by edit, reset,
  selection, or Cancel. Until then, cross-session choice visibility remains a documented
  compatibility exception.

## Remaining architecture work

1. Complete vertical frontend slices for Playback, Programmer, Highlight, Output health, remaining
   Show capabilities, Patch, Screens, Files, and Configuration. Replace polling and broad bootstrap
   refreshes with narrow snapshots plus relevant event subscriptions.
2. Publish the remaining externally observable transitions once through typed events: Programmer
   ownership changes, Highlight movement, transition completion, output health/overload, and any
   remaining automatic runtime changes.
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
- Measure the complete command edit and execution path with artificial latency and on reference
  hardware. Record keystroke-to-visible time, request backlog depth, edit persistence cost,
  Enter-to-result time, snapshot reconciliation, broad bootstrap requests, and rerender counts.
  Synchronous session persistence per accepted command edit remains an explicit benchmark risk.
- Verify old shows, recovery from malformed/legacy active shows, Save As/export portability,
  layout data, unpatched fixtures, stored-empty Groups, ordered selections, Cue Phaser, Highlight,
  Preload, Update, Move in Black, route termination, shutdown, and first output after restart.
- Run the final formatting, Clippy, Rust workspace tests, frontend typecheck/tests/build, focused
  API/UI/OSC suites, unrestricted socket tests, desktop smoke, and authoritative `./build open`
  readiness/log/operator-path verification.

## Current verification snapshot

- Every production file and function touched by this slice remains within the hard 1,200-line and
  150-line limits; the values wire implementation is split into feature-owned projection, event,
  and mutation modules below 400 lines each. Dependency-direction checks and all 10
  scanner/ratchet unit tests pass. The repository-wide architecture command still exits 1 because
  it reports the pre-existing committed Dynamics Editor experiment at 1,382 lines; this
  Programmer-values slice deliberately did not expand into that separate experiment.
- The earlier full-tree design-goal report was 52 files above 400 lines and 3,479 functions above
  20. The two remaining in-scope hotspots named by that report have since been split: Programming
  service is now 399 lines and the command HTTP adapter is 304 lines. The expanded server feature
  boundary was split into a 113-line composition hook and an 81-line Programmer-values helper.
  The source-size ratchet reports no changed production function above 150 lines and only the
  pre-existing committed Dynamics Editor experiment at 1,382 lines.
- Focused application, server, wire, frontend, architecture, source-size, MVR, File Manager,
  Playback, Preload, Patch, Output, event, shared-control, Stage 3D, build, and strict Clippy checks
  have passed for their committed slices. The latest command-line slice passed 18 Programming
  application tests, 4 command HTTP scenarios, the focused OSC shortcut test, 219 combined scoped
  frontend tests, and 114 focused consumer tests. The external selection-adapter slice passes all
  7 Playback route tests, focused Highlight HTTP/OSC coverage, the legacy Programmer gesture test,
  formatting, and strict server Clippy. The engine-refresh slice adds 3 application coordinator
  tests and 2 server integration tests covering unowned and nested actor-owned multi-desk Group
  changes. The current full server library run passes 215 tests with only the sandbox-blocked CITP
  socket test failing and one standard-port Matter test ignored. All 54 engine unit tests plus its
  integration test pass; focused Group-master WebSocket and Matter activation tests pass. The
  Stage migration adds 6 focused frontend tests for ordered streamed updates, optimistic
  gestures/clear/rollback, and active-only Stage/Fixture pane observation. The following primary
  window slice adds 5 focused streamed/optimistic/dormancy scenarios; its 3 focused files pass all
  19 tests together with the existing selection and Highlight coverage. The isolated
  Programmer-values frontend core passes all 24 focused store/session/view tests and passed the
  full frontend typecheck. The parameter-bank migration passes 17 focused tests across its legacy
  behavior and streamed selection suites. The modal migration passes 13 focused tests including
  ordered streamed writes and closed-view teardown. Both slices pass the full frontend typecheck.
- The completed backend Programmer-values slice passes `cargo fmt --all -- --check`; all 60
  `light-programmer` tests; all 201 `light-application` tests; all 24 `light-wire` unit tests plus
  generated-contract verification; all 8 focused server Programmer-values tests; all 6 focused
  capture-mode tests; the lifecycle deletion/recreation regression; the 6 active-Preload tests;
  the transient compatibility regression; and
  `cargo check -p light-server --no-default-features`. The full server library run has 234 passing
  tests and 1 ignored test; only the known sandbox-blocked CITP socket test fails with the sandbox's
  `Operation not permitted` error. Fixture and Group set/release, one-action batch and clear, exact
  no-op/event behavior, replay, rollback and revision conflict, capture/write concurrency,
  lifecycle replacement, user ownership, same-user multi-desk sharing, foreign-user rejection, and
  timing/order preservation are covered. Wire tests print the existing non-fatal `ts-rs`
  `deny_unknown_fields` warning.
- The production frontend Programmer-values and capture-authority slice passes 106 focused tests
  across exact wire decoding, HTTP/WebSocket transport, prediction, stores, sessions, writer,
  mounted views, and normal `ServerProvider` composition. The focused tests cover rollback, safe
  exact-request replay, no-change results, both response/event orders, capture and values
  cursor-gap repair, concurrent repair joining, server/session replacement, late-response
  isolation, exact-user subscriptions, first-view dormancy, refusal while authority is loading or
  active Preload captures Programmer, absence of a broad bootstrap request, suppression of
  unrelated context rerenders, undeclared wire fields, and React StrictMode effect replay. The
  complete frontend suite passes all 936 tests in 148 files; typecheck and the production Vite
  build pass, with only the existing large-chunk advisory. The raw-control audit and Numeric Pad
  selection harness were corrected so test-only probes do not masquerade as production controls
  and the keypad test follows scoped selection authority.
- Current focused backend verification passes 2 normal-values Programmer tests, 7 Programming
  application action tests, and 8 server authentication/action/subscription tests. Generated wire
  contracts match the Rust DTOs, `cargo fmt --all -- --check` passes, and `git diff --check` is
  clean. Wire tests print the existing non-fatal `ts-rs` `deny_unknown_fields` warnings. A final
  repository-wide Rust/Clippy suite and real desktop run remain pending.
- The pending-Preload and parameter-value slice passes 5 focused Programmer batch/action tests,
  including a 10,000-fixture set/release batch completed in 0.05 seconds in the debug test run, and
  3 focused generation tests including rejected live and staged transaction rollback. All 209
  `light-application` tests pass, including 54 Programming tests, replay fingerprint/payload-reuse
  checks, byte-budget eviction, and rejected-batch rollback. The 6 focused server pending-Preload
  tests cover fixture/Group mutation and release, no-op, replay, conflict, same-user two-desk
  sharing, a successful independent second user, foreign-user rejection, and exact-user event
  filtering; all 8 normal Programmer-values server tests still pass. Both focused Preload wire
  tests and generated-contract verification pass. Strict Clippy passes for Programmer,
  application, wire, and server with the existing `too_many_arguments` allowance.
- The frontend now passes 1,002 tests in 158 files, including pending-Preload wire/transport,
  store/session/writer/provider composition, parameter routing, response-before-event and
  event-before-response, rollback, replay/no-change, cursor repair, server/session replacement,
  capture-loading refusal, dormant first-view behavior, no bootstrap request, and unrelated-render
  suppression. Typecheck and the production Vite build pass; the only build output is the existing
  large-chunk advisory. Dependency-direction and scanner tests pass. The source-size command still
  exits non-zero solely for the pre-existing committed 1,382-line Dynamics Editor experiment; no
  production file or function changed by this slice exceeds a hard limit.

## Wrap-up handoff

- Normal and pending-Preload recordable values now have separate, capture-safe exact-user
  authorities from typed application mutation through authenticated v2 transport to dormant
  production providers and optimistic writers. The parameter-control cohort chooses between them
  only after capture authority is ready and never routes a typed normal action into Preload or a
  legacy Preload action into normal values.
- Batch mutation work is one application action, one domain checkpoint, one timestamp, one
  retained projection, and at most one values event. Large fixture batches are indexed and
  retained once rather than scanning the vector per address; replay retains fingerprints rather
  than request bodies and is explicitly memory bounded.
- Lifecycle deletion/recreation still preserves monotonic exact-user values and capture revisions
  and invalidates old replays. The broader Programmer ownership/lifecycle projection remains
  separate architecture work.
- No Patch/setup selection cleanup or public test-DSL refactoring was started in this slice.
- Recommended next slice: move the command-bar normal/pending activity summary and the current-user
  System Controls value count onto the scoped authorities, then remove the now-unused compatibility
  value actions and value-triggered bootstrap refreshes. Keep multi-user Programmer lifecycle UI,
  Patch/setup selection, and the public test DSL as distinct milestones.

Test files may exceed the hard limits, but should still be split when it improves readability and
makes operator intent more visible.
