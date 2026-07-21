# Major Refactoring Progress

Estimated progress: **97%** (revised down from 98.3% after the first e2e run exposed 88 pre-existing
operator-path failures that earlier unit-only checkpoints had hidden).

Estimated Codex ETA: **roughly 30–55 hours of active Codex execution**, plus required reference-
hardware measurement time, to repository-wide acceptance. The frontend facade decomposition is
mature (Patch, configuration, stage-layout, desk-lock, and shell-status authorities are all
scoped). The dominant remaining risk is now operator-path acceptance: the typed Playback-topology
migration ships with 88 failing e2e scenarios that must be reconciled with the acceptance specs,
plus the one Patch definition-resolution regression owned by this effort. See "End-to-end
acceptance status" below.

This is the living handoff for [`major-refactoring.md`](major-refactoring.md). Update it after each
meaningful milestone. A checked item means the implementation is committed on `refactoring` and
has focused verification; it does not replace the final repository-wide acceptance run.

Last updated: 2026-07-22 after the first full Playwright run of this effort. It surfaced a large
pre-existing e2e regression (88 failures) plus 5 introduced by the Patch reader migration; one class
of the 5 is fixed and the rest is scoped. See "End-to-end acceptance status" below before doing
further facade work — the e2e baseline is not green and must be addressed.

Earlier the same day: completed typed Group management end to end, finished Patch read ownership,
scoped desk-configuration, stage-layout, desk-lock, and shell-status authorities, and fixed the
polled-value rerender churn. The broad Patch snapshot, configuration, stageLayout, deskLock,
status, and error fields are all gone from the frontend facade.

## End-to-end acceptance status (2026-07-22)

This is the first `./test e2e` (now `tools/test.sh e2e`) run since checkpoint `47030ed`; every
milestone before this was verified with unit/contract tests only. The suite is **not green**, and
the failures split cleanly:

- **Full suite at `47030ed` (pre-work checkpoint): 174 passed, 88 failed, 26 skipped, 0 flaky.**
- **Full suite at HEAD (`47a9466`): 169 passed, 93 failed, 26 skipped, 0 flaky.**
- Diff of failing spec titles: **0 fixed, exactly 5 introduced** by the 28 commits since `47030ed`.

**88 failures are pre-existing** — the prior agent's cuelist/topology/visualization migration was
committed with a broken e2e suite that was never run. Signature: `@api` variants pass, `@ui`
variants fail (e.g. CUE-011 "renumber is one revision" — the UI-driven path emits the wrong show
revision count while the API path is correct). This is a large, separate body of work: the typed
Playback-topology migration of inline Cue edits, Cuelist settings, and atomic renumber needs its
operator-path behavior reconciled with the acceptance specs. It is **not** caused by any facade
slice in this document.

**5 failures were introduced here, all one root cause** — the Patch reader migration moved
programmer-surface consumers off the always-present `server.patch.fixtures` onto the scoped Patch
store:

- `POSITION-HOME-001 @ui` — Return Home button stays disabled (no home assignments)
- `PROG-002 @ui` (hardware encoder spread + fixture window)
- `ENCODER-DISPLAY-001 @supplemental-ui` — encoder slots show "Unassigned" instead of Pan/Tilt
- `HIGHLIGHT-003 @ui` — PREV/NEXT/ALL selection + keypad geometry
- `COMMAND-HISTORY-001 @supplemental-ui` — reconnect/hardware-layout unfinished input

Two mechanisms:

1. **Activation cold-start (FIXED, commit `346726b`).** The scoped selectors gated Patch-stream
   activation on a non-empty selection, so fixtures cold-started on the first selection where the
   old bootstrap load made them present as soon as a show opened. `PatchFeatureBoundary` now mounts
   a persistent `PatchAuthorityActivation` so the shared authority hydrates whenever a show is open.
   This clears COMMAND-HISTORY-001 @ui and the PROG-002 supplemental variants. Rerender isolation is
   unaffected (it is in the selectors, not in lazy activation). 1,974/1,974 unit tests still pass.

2. **Definition-resolution race (REMAINING — next fix).** The scoped store resolves fixture
   definitions client-side via `mergeFixtureDefinitions(fixtureProfiles, fixtureLibrary)` and
   applies its snapshot exactly once. If the snapshot is fetched before `fixtureProfiles` finish
   loading, `projectionToPatchedFixture` falls back to `syntheticDefinition`, which carries no head
   parameters — so `useSupportedAttributes`/`returnHomeAssignments` see no attributes and the
   controls render empty/disabled. The old `server.patch` was server-resolved, so it never raced.
   Fix: make `PatchSession`/`PatchStore` re-project (or re-fetch the snapshot) when the definition
   resolver changes, so late-arriving definitions upgrade the synthetic fixtures. This is the
   remaining regression owned by this effort (POSITION-HOME-001, HIGHLIGHT-003, ENCODER-DISPLAY-001,
   PROG-002 @ui).

Desktop smoke (`tools/test.sh desktop-smoke`) passes 2/2 (DESKTOP-001, DESKTOP-002): the packaged
app owns and terminates its exact child server and refuses to adopt an independent one.

Process notes for the next session: the branch now carries semantic-release/DX tooling commits
(`d5206f8`, `62d2ac3`, `47a9466`) that moved `build`/`test`/`dev` to `tools/*.sh` — use those paths.
Playwright clears `.artifacts/test/results` at the start of every run, so copy the failure set aside
before running another suite. `git checkout`-based bisecting is unsafe while uncommitted work is in
the tree; commit or stash first.

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

- [x] Added source-size and dependency-direction architecture checks with a ratchet against new or
  growing hard-limit violations. The two remaining committed findings are tracked explicitly in
  the current verification snapshot below.
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
  Virtual exclusion peers and startup normalization now use that boundary; exact semantic no-op
  reporting is complete. Portable topology, Cuelist mutation, and the primary physical bank are
  complete below, as are the Page and shortcut companion callers. Group and System Controls now
  use exact runtime identities, and the broad frontend Playback snapshot has been removed.
- [x] Removed mutable Playback-service lock exposure from the migrated paths: Engine callers use
  typed commands and immutable projections, Preload installs generation-bound prepared batches,
  and application-owned units of work serialize page changes, automatic render transitions, and
  semantic event publication.
- [x] Added the view-scoped Playback runtime frontend: exact v2 snapshots and WebSocket filters are
  activated only by mounted Playback/Cuelist views, desk-only views request no runtime identities,
  and gaps and malformed messages repair from authoritative snapshots. Concurrent fader and page
  mutations use independent optimistic overlays with request-ordered rollback and authoritative
  event/outcome reconciliation. Exact Group identities share this store without materializing the
  broad v1 snapshot. Playback/Page topology events now reconcile only their scoped stores; no
  compatibility reload remains in the frontend.
- [x] Added typed portable Playback topology actions for Cuelist save, slot configure, and mapped
  Playback clear. One show-revisioned application action preserves legacy storage identities and
  unknown fields, returns one coherent Page/Playback/Cuelist projection, publishes at most one
  Show event, and retains exact request replay. The strict client binds the requested objects and
  exact deleted Playback to captured storage identities and status-aware revisions before atomic
  installation. Virtual Playback now hydrates only its portable topology, exact runtime
  identities, active desk view, and show/desk-scoped exclusion zones; it no longer reads the broad
  Playback bootstrap facade.
  Empty-slot assignment and mapped clear remain one serialized network action, held Flash/Swap
  releases survive same-Show session replacement without crossing a Show switch, and inactive
  panes open no snapshot/socket and subscribe to no topology/runtime selectors.
- [x] Added exact typed mapping of an existing Cuelist Playback into one Page slot without cloning
  or rewriting its source Playback. Changed and replay/no-change outcomes contain one authoritative
  Page, retain request/correlation and event authority, validate exact source/Page identities and
  revisions, and publish at most one Show event. Capability transactions compile compatibility
  migrations in memory without silently persisting unrelated objects, and canonical Cuelist,
  Playback, and Page storage-key collisions fail before mutation rather than overwriting legacy
  data. The primary physical bank and configuration dialog now consume scoped topology, exact
  runtime identities, desk and command-line authority, and Groups only when a visible target needs
  them. Cuelist assignment, configure, and clear are typed revision-checked actions with no textual
  `SET` or broad refresh. Stale runtime targets render no controls, broad parent updates are
  memo-suppressed, and held Flash/Swap releases retain their original semantic and cannot overtake
  a delayed or retried press.
- [x] Added typed Playback Page create and rename actions and strict existing-Page desk selection.
  Page mutations validate exact storage identity and revision, preserve lossless extensions,
  retain request replay, return changed/no-change authority, and publish at most one retained Show
  event.
  Scoped desk selection rejects a missing Page without creating topology or emitting a desk event,
  while explicit compatibility callers retain their documented auto-create behavior. The strict
  frontend transport and writer validate authoritative outcomes, serialize operations, repair
  conflicts, and prevent preflight, retry, repair, or late outcomes from crossing a Show, session,
  server, or writer replacement.
- [x] Moved the Page dialogs and controls, hardware summary, keyboard Playback shortcuts, Numeric
  Pad Page action, secondary Screens, Cuelist Window, and Product Demo Playback controls onto exact
  topology, desk, and runtime authority. Independent Screens read and write only their own Page;
  Follow Main reads only the desk Page. Product Demo resolves portable Page assignments and exact
  mapped runtime identities without a bootstrap fallback. All surfaces refuse stale or loading
  authority, preserve held Flash/Swap release ordering, reject abandoned renders and replaced
  writers, and remain dormant when their owning view is not mounted.
- [x] Migrated inline Cue-editor writes, Cuelist settings, and atomic renumbering from generic
  show-object mutation plus broad refresh onto the typed Playback topology action. Writes capture
  the exact storage identity and revision, preserve lossless body extensions, return authoritative
  scoped objects, and reject replacement races before mutation. Rapid inline edits serialize and
  rebase only on the preceding authoritative outcome; failure cancels later queued intent without
  stranding retries, repaired authority supports explicit retry, and old writer/session responses
  cannot cross into a replacement scope. Real changes remain one action with at most one Show
  event, and the legacy `saveCueList` server-context adapter is removed.
- [x] Completed Programming Update as one typed Programming application workflow. Preview,
  eligible-target discovery, confirmation, and direct apply share one coherent Programmer/desk/
  Show capture, preserve exact Cue context and legacy storage identity, reject stale object,
  Programmer, and Show revisions, retain idempotent replay, and produce one lossless portable
  projection plus at most one Show event only for a semantic change. The strict v2 contract keeps
  preview, target-menu, action, settings, and error payloads separate from feature models and
  preserves external v1 compatibility. The production action-only provider is scoped by server,
  session, Show, desk, and user; its FIFO writer handles either HTTP/event order, identical-request
  retry, replay, pending rollback, exact conflict repair, and same-Show authority replacement.
  Update dialogs and Setup settings now consume that capability without `refresh()`, bootstrap,
  or broad Playback reads, and the internal v1 Update API/server-context facade is removed.
- [x] Completed Cue COPY/MOVE Plain/Status as one typed Programming action. The retained choice
  carries exact Show, source, destination, command-line, user, session, and desk authority; a real
  transfer preserves lossless Cuelist fields and Cue identity rules, commits one or two Cuelist
  projections in one ActiveShow transaction, and emits exactly one Show event. Copy allocates one
  destination Cue ID, Move retains the source ID, Status materializes tracked fixture and Group
  state without per-address timing, and the sole-Cue cross-Cuelist Move remains rejected. The
  authenticated v2 route reports request/correlation identity, replay state, authoritative Show
  and command-line revisions, projection/event authority, and persistence warning. Replay is
  checked before the resolved choice, so a retry after Cancel or success cannot repeat the
  mutation or resurrect the choice. The action-only frontend provider performs no snapshot or
  socket work, installs strict lossless projections into the existing Show Objects authority, and
  reconciles optimistic choice closure whether the response or event arrives first. Conflicts
  repair only the Show/Cuelist or exact desk command-line authority; late responses cannot cross a
  server, session, Show, desk, or user replacement. Legacy command/OSC/WebSocket execution shares
  the typed boundary while its temporary per-object v1 notification remains isolated to the
  compatibility path; non-SET Preset COPY/MOVE remains owned by Preset mutation.
- [x] Made virtual Playback exclusion activation one atomic Engine transition. Actual exclusion and
  auto-off releases are returned as sorted related projections, published once before the primary
  high-water event, retained by idempotent replay without re-execution, and applied to the frontend
  store in one notification regardless of HTTP/event arrival order. Current-page, explicit-page,
  direct, fader, Flash/Swap release-promotion, Crossfade, Matter, and queued Preload paths share the
  same address-aware rule. Preload remains one prepared batch/install with one persistence phase;
  its retained queue now carries the captured page. Restored exclusions normalize before the
  output scheduler can render. The scoped client rejects mismatched request IDs, replaces authority
  on server/session changes, ignores late work, repairs gaps, and remains dormant until a Playback
  runtime view mounts.
- [x] Separated accepted Playback operations from their exact runtime consequence with domain-owned
  `None`, `Transient`, and `Durable` effects. Repeated explicit On/Off, same-value master and virtual
  master, Load, zero-time XFade endpoints, temporary/Flash/Swap edges, repeated Pause, exact GoTo at
  the same runtime instant, and saturated or same-value Group, Grand Master, Speed Group, and
  time-master writes now return no-change without publishing or persisting runtime authority.
  Transient holds still affect live output but never rewrite durable state. Addressed and aggregate
  effects remain distinct: auto-off/exclusion changes persist and publish their changed peers without
  fabricating an equal primary event, while a timed XFade retrigger or hidden On/Preload transition
  still publishes exactly one addressed event. One prepared Preload batch aggregates all effects,
  installs at most once, publishes each changed runtime identity once, and persists active Playback
  runtime only when the batch is durable; an all-no-op batch drains its queue without a runtime event
  or either persistence write. Feature-owned persistence plans keep active Playback and
  output-runtime writes independent, so interaction-only actions and Cuelist changes cannot
  serialize unrelated output state.
- [x] Made queued Preload preparation batch-native and final-state-aware. Batched projection reads
  and shared immutable exclusion-zone data feed one prepared generation; exact final runtime
  effects prevent mutate-then-cancel queues from installing, persisting, or publishing. The final
  effective staged action owns transition cause and surface, so interim actions cannot leak into
  retained events or exclusion audits.
- [x] Added domain-owned Playback activation provenance for restart-exact virtual exclusions.
  False-to-true activations atomically stamp a stable ordinal, activation timestamp, originating
  desk, surface, and exclusion scope; Cue/timing changes preserve that authority and deactivation
  clears it. Timed Preload releases retain the original authority until their release fade ends,
  and deskless Matter activation remains explicitly outside desk-local exclusions. Only the private
  active-runtime checkpoint serializes provenance; public runtime payloads omit it. Legacy
  checkpoints migrate through an explicit all-desks fallback, while new checkpoints replay
  activation order against each captured desk's zones. Same-user queued Preload actions retain
  their capture desk when committed elsewhere, deleted desks inherit no zones, and explicit
  non-current-page activations do not borrow current-page exclusions.
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
  save cannot replay as a false success. Cue ambiguity is now explicit runtime-only desk state set
  only by a `ChoiceRequired` execution after ENT; command edits, reset, Cancel, an accepted choice,
  and show replacement clear it without persisting it. The same desk shares one authoritative
  choice across UI, OSC, and compatibility WebSocket surfaces while another desk remains isolated.
  Replay returns current authority without resurrecting a resolved choice, and each semantic
  transition publishes at most one sparse interaction event. The production modal consumes only
  the scoped projection, reconciles optimistic Cancel with rollback, ignores legacy response-local
  choice data, and rejects late outcomes after server/session authority replacement.
- [x] Removed the production command-line editor's final `useServer()` execution and mutation
  fallbacks. A scoped, explicitly injected execution capability now owns readiness and Enter;
  disabled, unmounted, loading, or writerless views refuse reads and writes instead of borrowing
  stale global state. Edit-before-Enter ordering, optimistic response/event races, recoverable
  failure, request replay, command-choice cancellation, authority replacement, and late outcomes
  are covered through the real Programming interaction provider. Broad test harnesses install an
  explicit feature-owned authority rather than weakening production ownership.
- [x] Added a typed v2 CUE navigation action for `CUE`, `CUE CUE`, decimal Cue numbers, selected-
  Playback, pool `SET <playback>`, and explicit-Page `SET <page> . <slot>` addressing. Pure grammar
  parsing is state-independent; execution resolves
  exact desk selection and Page topology before calling the existing Playback application action.
  The Programming boundary owns command reset and history exactly once, request replay repeats no
  interaction or notification, and no-change emits no typed or compatibility event. The former
  CUE compatibility family is removed from the public helper and ratchet; one dedicated v1
  WebSocket specification remains as the intentional compatibility proof.
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
- [x] Moved the command-bar recordability and pending `PROG n` summaries plus every current-user
  System Controls row onto scalar selectors over the exact-user values stores. Capture authority
  resolves before exactly one normal or pending value view hydrates; loading never falls back to
  stale bootstrap values, same-count value replacements do not rerender the summary, and foreign
  users remain on the compatibility lifecycle list because exact-user authorization deliberately
  prevents inspecting their scoped projections.
- [x] Migrated Channels and every recordable Special Dialog gesture onto capture-safe typed value
  batches. Channel, Beam, Dynamics, and Color motion retain only the newest pending value per
  target; Color range completion and Position Home remain ordered barriers; and each 32 ms
  Position tick sends one ordered batch instead of two requests per fixture. Controls refuse writes
  while capture or values authority is loading. The remaining recordable set/release/batch client
  facade is removed; transient fixture-control actions and preset generation keep their separate
  non-value compatibility boundary.
- [x] Migrated Numeric Pad CLR state and normal Programmer clearing off bootstrap. Scoped selection
  is still the first normal clear step, typed values clear is the second, active or playback-only
  Preload retains lifecycle clear semantics, and the obsolete legacy value-clear action plus its
  broad bootstrap reload are removed. The former oversized pad function is split into feature-owned
  controller and rendering modules below the hard source-size limits.
- [x] Migrated Fixture Sheet active-value filtering and active-first ordering onto a capture-first
  selector over the exact-user normal or pending values authority. The selector observes only
  fixture/Group target membership, expands current Group membership including logical heads,
  remains dormant unless an active pane uses the filter/order, suppresses value/timing-only
  rerenders, and exposes loading instead of treating stale bootstrap values as authoritative empty
  state.
- [x] Stopped exact owned normal/Preload value-only compatibility events from requesting a broad
  bootstrap snapshot. Categorized WebSocket events now carry user and desk identity plus the
  previously omitted pending-values category; same-user peer-desk values stay on the scoped store,
  transient-only changes skip an unrepresentable reload, and foreign, unowned, malformed,
  duplicated, mixed, runtime, and lifecycle events fail closed to compatibility hydration.
- [x] Migrated Special Dialogs and System Controls selection onto visibility-scoped streams. The
  ordered projection is passed explicitly through Color, Position, Beam/Shapers, Dynamics, Control,
  and Lamp On helpers without per-interaction copies; external changes update open modals, closed
  modals do no hydration and tear down their stream, and System Controls retains its separate
  compatibility list of all active Programmers pending the lifecycle slice.
- [x] Migrated Patch/setup selection onto the scoped ordered Programming projection. The Patch
  window activates the selection view only while visible; loading never falls back to bootstrap.
  Normal, range, additive, split-address, parameter, and post-placement selection all issue one
  complete typed replacement. Additive clicks preserve an existing closed ordered selection, and
  every path selects resolved logical heads rather than a parent fixture. Newly patched fixtures
  consume the authoritative server result, so the client does not guess server-generated head IDs.
- [x] Added a safe aggregate Programmer lifecycle authority for System Controls. The application
  projects only ownership, connectivity, per-user value counts, selected-fixture count, and
  session identity; it neither clones full Programmer history nor exposes foreign-user values.
  Shared command contexts count their selection once across an app and attached OSC surface.
  Authenticated snapshots and lossless global deltas cover session and Programmer replacement,
  disconnect, value-count changes, and selection-count changes while quiet paths publish nothing.
  The dormant frontend store updates only mounted lifecycle views and keeps unrelated global
  consumers outside its render path.
- [x] Added an exact-user queued Preload playback authority with ordered duplicate actions and
  typed physical, virtual, OSC, and Matter surfaces. Capture, successful GO drain, clear, release,
  undo/redo, rollback, and Programmer replacement pass through the Programming boundary and emit
  at most one replaceable projection per semantic transition. The strict frontend snapshot/event
  adapter remains dormant until its Command Line value view mounts, repairs cursor gaps, rejects
  foreign scope, and refuses stale bootstrap queue data across show, session, or server changes.
  Queue-only compatibility events, including the interaction-plus-queue GO shape, no longer
  request broad bootstrap state.
- [x] Removed full Programmer state from the unauthenticated bootstrap response and authenticated
  the remaining v1 compatibility list. The compatibility endpoint filters the current user before
  cloning session rows, same-user desks remain visible to each other, and foreign-user values,
  selection, modes, command text, Preload buffers, and undo/redo history are never serialized.
  Production and acceptance callers now authenticate explicitly.
- [x] Added typed action-time normal Preset recording. Touch, command-line HTTP, OSC keys, and the
  compatibility WebSocket command path now capture only normal recordable fixture/Group values
  through the Programming application boundary. One request performs one lossless active-show
  transaction and returns a strict changed/no-change union with request/correlation identity,
  replay state, authoritative Preset body and revisions, and an event sequence only for a real
  mutation. Canonical and numeric legacy storage identities remain compatible, unknown body fields
  survive, and replay does not repeat persistence, command history, or interaction side effects.
  The action-only frontend provider stays dormant until its Show Objects Preset view is mounted;
  its pending-only writer reconciles either response/event order, no-change, replay, conflicts,
  rollback, gaps, authority replacement, and late responses without a Programmer or bootstrap
  read. It prefers the canonical object when a legacy alias also exists and strictly validates the
  complete outcome before installing authority.
- [x] Added typed action-time Group recording. One Programming action captures the actor desk's
  ordered selection for overwrite, merge, subtract, or delete while preserving exact opaque IDs,
  stored-empty versus absent Groups, live/frozen relationships, portable programming, lossless
  legacy bodies, dependency-safe deletion, revision checks, replay isolation, and user ownership.
  Command-line HTTP, keyboard, OSC, and compatibility WebSocket paths converge on the same action
  and publish one retained Show event per semantic transition. Group Pool and Group Strip now use
  a strict action-only client that captures the dialog-open revision, reconciles stored/deleted
  outcomes in either event/response order, repairs only the conflicted Group, and clears the scoped
  `RECORD` command only after success. The old multi-request Group store/refresh adapter is gone.
- [x] Moved Group Strip and Group Pool activation onto scoped Programming selection actions.
  Live activation sends one ordered gesture; frozen activation captures the exact Group rule and
  Show revision at interaction time. Stored-empty Groups remain selectable, inactive surfaces open
  no authority, missing authority sends nothing, and legacy `server.playbacks` membership cannot
  retarget selection. Cuelist Window follow-selection, current-Cue display, and pool masters now
  read exact desk/runtime projections, while Cue-pane and Fixture Sheet Cuelist pickers hydrate only
  their feature-owned portable Show-object views.
- [x] Removed the remaining non-runtime Group reads from the broad Playback snapshot. Parameter
  projection activates portable Group authority only for a visible Group target and hides retained
  programming until that authority is ready. Cue thumbnails consume active-pane portable Groups,
  preserve ordered membership, synchronously hide stale results, and cancel late visualization
  work across Cue, Show, authority, and capability replacement. Selected live-Group membership and
  derived-Group detachment now use the Show Objects projection, and paperwork loads Cuelists,
  Groups, and Presets on demand from the active Show. The unused Page, slot, definition, exclusion,
  pool-action, and Cuelist-unassignment `ServerContext` facade actions are deleted.
- [x] Moved System Controls runtime enumeration and release onto scoped authority. The closed modal
  opens no Show-object or runtime snapshot and no socket; the open modal hydrates only portable
  Cuelists and Playbacks, then subscribes to the exact mapped Playback and direct Cuelist runtime
  identities it displays. Individual and aggregate release preserve request replay, either HTTP/
  event order, loading refusal, and Show/server/session replacement safety. Aggregate Stop
  deduplicates exact sources while retaining Programmer clear and Preload release. Playback store
  scope reset now occurs outside React render, and selector caches invalidate when a replacement
  authority changes the selected identity set.
- [x] Added direct typed Group runtime identity, snapshot, action outcome, and event authority.
  Strict opaque Group IDs resolve either an assigned Playback or an unassigned direct master;
  assigned actions delegate only through the exact valid Group-targeted Playback. Master changes
  persist output runtime, Flash remains transient, replay/no-change emits nothing, and each real
  transition has one sequence routed to both exact Group and mapped Playback subscribers. Missing,
  stale, forged, wrong-target, and foreign scopes are rejected while legacy WebSocket, OSC, and
  Matter entry points retain compatibility behavior.
- [x] Moved Group Window, Fixture Sheet, and Group editing onto dormant exact Group runtime views.
  Portable membership and exact master/Flash authority hydrate separately, stored-empty and ordered
  Groups survive, loading authority is never replaced by stale portable masters, optimistic master
  writes handle either HTTP/event order, rollback, replay, gaps, and scope replacement, and opaque
  delimiter-containing IDs cannot alias another subscription set. The legacy Group runtime merge
  helper and `ServerContext` master writers are deleted.
- [x] Removed the broad frontend `/api/v1/playbacks` snapshot end to end. Initial connection,
  refresh, Show-open, and Playback/Page object reconciliation no longer fetch or store it; exact v2
  runtime actions/snapshots remain. Screens retain their separately owned refresh on Show/Page
  changes. The dead `playbackAction` facade is gone, and a production architecture ratchet rejects
  broad snapshot state, fetches, endpoint strings, and the legacy `useGroups` helper while leaving
  backend/external v1 compatibility coverage intact.
- [x] Moved the remaining production Patch mutation callers behind the feature-owned Patch
  boundary. Patch Parameter Controls, Fixture Patch Setup, Media Server Setup, and Patch Window now
  capture scoped authority and use typed optimistic mutation with rollback and replacement guards;
  the dead broad fixture writers were removed. Stale preview media responses are suppressed, and
  hidden/inactive Patch surfaces do not acquire mutation authority.
- [x] Added explicit public session-handoff, desktop, and hardware-OSC test adapters. Session
  credentials remain Node-owned and are cleared across navigation, close, reconnect, disposal, and
  replacement; late prior-document captures cannot regain authority. Desktop and OSC controls are
  opt-in and absent from ordinary browser execution. Public scenarios no longer inspect private
  session storage or fabricate Tauri globals, and an architecture ratchet enforces those boundaries.
- [x] Added typed per-user Programmer-priority snapshot, action, tombstone, and event authority.
  Revisions, timestamps, request replay, no-change, lifecycle replacement, exact-user subscription
  security, same-user multi-desk sharing, and foreign-user rejection remain independent of the
  normal-values projection. Added atomic Preset recall over one coherent portable Show document:
  exact Preset and Group revisions are captured once, ordered values apply in one Programmer
  transaction, open selection gestures close through one sparse interaction event, interaction-only
  outcomes omit the complete values projection, and v1 compatibility reuses the typed service.
  Both now have dormant production providers, strict HTTP/WebSocket adapters, request-ordered
  optimistic writers, narrow repair, tombstone/recreation handling, replacement guards, and no
  bootstrap fallback. Preset cards activate only the exact Show collection and user authorities
  they need; Priority remains action-only until a consumer mounts.
- [x] Added typed Preload lifecycle actions for Enter, GO, pending clear, and release through the
  Programming boundary. Exact capture, values, queue, selection, portable-Show, and filtered
  Playback cursor preconditions are checked after replay lookup. GO retains one prepared Playback
  batch and one install, returns ordered execution/runtime metadata, and publishes each changed
  authority once; no-op and replay outcomes remain sparse. Same-user desks share the user-owned
  state, foreign paths are rejected, failed GO rolls Programmer and Playback back atomically, and
  legacy WebSocket commands reuse the typed service without duplicate compatibility events.
- [x] Added feature-owned hosted-file-picker control for public acceptance tests. Browser scenarios
  drive the real hosted picker contract rather than dispatching raw private `light:*` events, and
  the private-boundary ratchet rejects restoring that shortcut.
- [x] Added production-decoder-backed public Priority and Preset intents and migrated every direct
  `programmer.priority` and `preset.apply` scenario call. The command-boundary scanner now inventories
  every literal v1 action by both file and family and requires exact baseline updates even for a
  partial shrink. API-004's edit/target/error-envelope checks and CUE-015's dedicated navigation
  compatibility spec remain the explicit retained v1 inventory.
- [x] Added typed action-time Cue recording. One Programming action captures only normal or pending-
  Preload recordable values, resolves explicit or authoritative active targets under the portable
  Show revision, and atomically creates, updates, or deletes the Cue plus any required Cuelist,
  Playback, and Page topology. Overwrite, merge, subtract, tracking, timing, Phaser data, active-
  Preload release, take-live, no-change, revision conflict, replay, user ownership, same-user desks,
  lossless extensions, and one retained Show event per semantic transition remain explicit. Touch,
  command-line HTTP, keyboard, OSC, and compatibility WebSocket paths share that boundary and its
  strict request/correlation/replay/outcome contract. The frontend now hydrates and subscribes only
  when a Cue-related view mounts, installs one coherent multi-kind snapshot, reconciles either
  response/event order, repairs exact objects after gaps or conflicts, and rejects stale work after
  authority replacement without a bootstrap fallback. Exact non-Group repair reads one SQLite row;
  equal-revision topology retains object identity and does not rerender unrelated consumers.
- [x] Completed production Preload lifecycle ownership. The active Preload controls, modal, command
  paths, and queued Playback handoff use exact capture/values/queue/selection/Show/Playback
  authority, remain dormant until mounted, reject stale bootstrap state, and reconcile replay,
  rollback, gaps, either response/event order, and server/session/Show/user replacement. Public
  Preload helpers now express Enter, pending clear, GO, and release through one strict action each;
  the remaining two direct clear call sites are intentionally outside the normal-values contract
  until their acceptance scenarios are classified.
- [x] Completed the scoped Output runtime client and production composition. System Controls and
  command-line blackout feedback consume one retained global-master store with exact active-Show,
  desk, session, and connection authority; loading performs no visualization/bootstrap fallback,
  optimistic actions reconcile either event/response order, and unrelated server updates stop at a
  memoized status leaf. The old App blackout state and `setMaster` client/server facade are removed.
  A production-decoder-backed public helper migrated the final direct `master.set` call while
  preserving safe-blackout restart and first-frame assertions.
- [x] Added a revisioned installation-wide manual Speed Group authority for absolute/relative BPM
  and A-E synchronization. Exact authority/revision expectations, request replay and collision,
  same-installation multi-desk state, direct-ownership reset, persistence warnings, no-change, and
  one lossless event per real transition are application-owned. Command-line HTTP and compatibility
  execution share the syntactic parser and service; authoritative execution emits no v1 payload,
  while the retained compatibility path emits its prior payload once. Sound observation/config,
  Learn, pause, double/half, and legacy Playback controls remain a separate runtime-control slice.
- [x] Completed the Speed Group production client and public action owner. The strict retained
  store, HTTP/WebSocket adapter, and writer bind the installation/session authority, reconcile
  optimistic set/adjust/synchronize actions in either event/response order, repair gaps and
  conflicts, and remain dormant until a Speed view mounts. Playback Tools no longer borrows BPM
  from bootstrap, Sound-to-Light activation is modal-scoped, and all eight public Speed commands
  now use one typed intent each rather than the compatibility WebSocket family.
- [x] Added typed whole-Cue deletion and migrated its public scenarios. Pool, current-Page, and
  explicit-Page addresses resolve exact Playback, Page, Cuelist, Cue, Show, desk, user, and session
  authority; one replayable Programming action preserves lossless Cuelist fields, rejects stale
  revisions and sole-Cue deletion, and emits at most one Show event. The strict public helper
  validates request/correlation identity, ETags, revisions, projection identity, replay/no-change,
  and late session replacement. Five public compatibility calls are gone; one dedicated v1
  WebSocket specification remains as the intentional external-client proof.
- [x] Added one shared, view-scoped Visualization runtime for the transitional v1 projection.
  Normal and Preload lanes are independently reference-counted, use the fastest mounted polling
  interval without overlapping requests, strictly decode the exact lane, and clear immediately on
  Show/session/server replacement. Parameter Controls, Channels, Fixture Sheet, and Stage now
  share the retained projection without duplicate polling or broad rerenders. The adapter verifies
  requested Show/session/server authority and drops late generations; the v1 payload itself lacks
  scope identifiers, so response payload scope cannot yet be independently cross-checked.
- [x] Added strict public Programmer-values intents and migrated 54 normal fixture/Group set,
  release, clear, and ordered batch call sites. Each helper captures exact active Show, desk,
  session, authenticated user, capture-mode revision, and values revision without bootstrap, and
  rejects active Preload or scope replacement before one action POST. The cue-semantic cohort
  preserves its configured zero-millisecond Programmer timing and original action cardinality;
  focused API execution caught and corrected an initial default-timing mismatch.
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
- [x] Completed typed Group management end to end. One application action family owns Group
  property update, undo, frozen refresh, and derived detach. Each request carries request/
  correlation identity and exact Show, Group storage ID, Group revision, optional source Group
  identity/revision, desk, user, and session authority through the existing Active Show boundary.
  One revisioned transaction preserves unknown fields, ordered fixture membership, stored-empty
  Groups, and derived/frozen metadata; it returns the authoritative lossless projection with
  changed/no-change and replay status, Show and object revisions, an event sequence only when
  emitted, and any persistence warning. Exactly one Show event is published per semantic mutation
  and none for a no-op or replay; a failed revision or invalid source mutates nothing.
  Frozen refresh resolves its source under the same Show transaction, stores the refreshed snapshot
  with source revision and timestamp, and leaves the originating desk selection on the frozen
  source. Its selection event is published inside the held show-mutation gate and strictly before
  the owning Show event, so no nested desk lock is taken. The active-show helper gained
  `transact_with_unit` so a capability can read adapter-owned object history inside the same
  transaction that commits it.
  The frontend adds a feature-owned Group management authority: strict wire decoding, an
  authenticated action-only HTTP transport, an action-only provider mounted through the feature
  composition layer, and a request-ordered writer that installs authoritative Groups into the
  existing Show Objects store in one notification. No broad snapshot and no additional global
  context were introduced. Authority is scoped by server, session, active Show, authenticated
  desk/user, exact Group storage ID, and revision; undeclared fields, foreign scope, inconsistent
  revisions, and late outcomes after a writer or authority replacement are rejected. Reconciliation
  is order-independent for response-before-event and event-before-response, and supports rollback,
  replay/no-change, revision-conflict repair from the authoritative object rather than bootstrap,
  retry limited to ambiguous transport failures, and scope replacement.
  `GroupContextMenu` and `GroupPropertiesDialog` no longer use `useServer()`; `updateGroup`,
  `undoGroup`, `refreshFrozenGroup`, and `detachDerivedGroup` were removed from
  `ServerProgrammingContext` and their superseded legacy adapters deleted. These four operations
  never had a server-side v1 or WebSocket handler — they were client-side compositions over the
  generic v1 object endpoints, which already route through `ActiveShowService`, so no group-specific
  compatibility adapter remained to re-route.

- [x] Finished Patch read ownership and removed the broad Patch bootstrap state. A feature-owned
  selector layer projects the authoritative Patch store with per-element equality, so an unrelated
  Patch delta returns the previous reference instead of rerendering a consumer; a disabled reader
  registers no store listener, and a reader outside a mounted Patch boundary observes an inert
  empty snapshot rather than falling back to bootstrap data. Selectors cover the whole list, exact
  storage IDs, one exact fixture, a selection resolved through logical heads, and load status, and
  caller-owned identity arrays are keyed so a fresh array per render cannot bust the cache.
  All 19 production readers migrated: Parameter Controls, the Special Dialogs modal, the Color,
  Control and Position special dialogs, System Controls lamp actions, Highlight Controls, Channels,
  DMX, the Stage visualization and Stage command controls, the Fixture Sheet projection,
  Group-window fixture resolution, and Cue thumbnails. Each activates Patch only while its own view
  is active, so a covered or inactive pane opens no snapshot and no stream. DMX now reads logical
  universes from the scoped output routes, removing the last reader of the routes copy embedded in
  the Patch snapshot.
  Startup no longer loads the broad Patch snapshot, the manual refresh no longer refetches it, a
  `show_opened` event no longer triggers a Patch reload, and a `patched_fixture` object change no
  longer refetches the whole Patch. Paperwork export reads the current Patch on demand and degrades
  to a null Patch rather than blocking the export or masking a collection error. The Patch feature
  boundary no longer seeds `initialFixtures` from the facade. The typed batch mutation, profile
  snapshot, stored-show, and unpatched-fixture contracts are unchanged.

- [x] Scoped desk-configuration authority and removed the broad configuration facade field.
  Configuration reads are almost entirely scalar, so each consumer selects one setting through an
  equality-cached projection: changing the programmer fade no longer rerenders the sequence-master,
  speed-group, Matter, Patch-preview, or file-manager readers, and unrelated server-context churn
  rerenders none of them. A reader outside a mounted configuration boundary observes an inert empty
  snapshot instead of falling back to broad server state. All 13 production readers migrated,
  including the two settings surfaces that genuinely edit the whole configuration. The
  authoritative configuration is published into the store outside the broad React context update
  path, keeping the existing bootstrap load and `server_configuration_changed` refresh as its
  source. The migration also exposed and fixed two Rules-of-Hooks violations: the Patch DMX preview
  and the Cuelist settings speed groups read configuration inside a short-circuit expression, which
  became a conditional hook call once the read was a selector.
  The configuration writers now live in a feature-owned, action-only provider: `saveConfiguration`
  and `setControlTiming` are gone from the facade, timing writes merge onto the authoritative
  configuration read from the scoped store so a partial change cannot revert an unobserved setting,
  and the applied result still flows through the existing configuration and Matter state so the
  store keeps a single source of truth.
- [x] Scoped stage-layout authority and removed `stageLayout` and `saveStageLayout` from the facade.
  Stage positions are read on hot paths such as Cue thumbnails, the Stage window, and Stage command
  controls, so they now come from a scoped store with equality-cached 2d/3d selectors; a stage-layout
  write reads its expected revision from that store rather than from broad server state. The desk
  capability boundaries moved out of `ServerProvider` into their own composition module.
- [x] Scoped desk-lock authority and removed `deskLock`, `configureDeskLock`, `lockDesk`, and
  `unlockDesk` from the facade. The desk lock is polled every 500 ms, and each poll previously
  pushed a fresh object into broad React state, rerendering every `useServer()` consumer twice per
  second whether or not the lock had changed. The lock now lives outside React state entirely:
  polling, bootstrap, and the three actions write straight into the store, and the store compares
  the complete lock state so an unchanged poll publishes nothing. `DeskLockOverlay` no longer uses
  `useServer()` at all.
- [x] Scoped connection status and the shared operator error, removing `status` and `error` from the
  facade. Both are scalars written from many places, so an error raised by one feature previously
  rerendered every unrelated consumer; a status reader is now not woken by an error change and vice
  versa. The duplicated modal-error markup across seven surfaces became one `ServerErrorNotice`
  that reads the error itself, so a surface that only displays errors no longer subscribes to shell
  status at all.

## In progress

- [ ] **Highest priority — fix the e2e baseline before more facade work.** (a) Finish the Patch
  definition-resolution regression owned by this effort (POSITION-HOME-001, HIGHLIGHT-003,
  ENCODER-DISPLAY-001, PROG-002 @ui) by re-projecting the scoped Patch store when definitions load.
  (b) Reconcile the 88 pre-existing operator-path failures from the typed Playback-topology
  migration with the acceptance specs. See "End-to-end acceptance status". Do not treat the facade
  decomposition as done while e2e is red.
- [ ] Continue vertical feature-store/event slices and move the remaining production callers away
  from broad `useServer()`, generic show-object mutation, and one-off polling. Configuration,
  stage-layout, desk-lock, and shell-status are complete (reads and writes). The next coherent
  owners are user/desk-layout persistence (`deskLayout`/`deskLayoutScope`/`saveDeskLayout`, one
  consumer), `bootstrap`/`session` identity (recommend narrow identity hooks, not field removal),
  fixture library and show lists, and the one-shot Cue-thumbnail Visualization read.
- [ ] Add the remaining typed actions required to remove compatibility facades: standalone Playback
  `SET`, command-line bare `UPDATE`, Preset delete/transfer, output-route/user-layout, and residual
  portable-show mutations.
- [ ] Continue converging the public test DSL, then run the final repository-wide performance,
  unrestricted socket, desktop, migration, and operator-path acceptance suite.

## Remaining architecture work

1. Publish the remaining externally observable transitions once through typed events: Highlight
   movement, transition completion, output health/overload, and any remaining automatic runtime
   changes.
2. Migrate remaining layout and miscellaneous portable-show mutations, then remove generic
   frontend show-object mutation.
3. Replace production `useServer()` callers with feature-local stores/hooks. Remove broad global
   React update ownership, DOM/custom-event SET/Store/Update routing, and polling-based refreshes.
4. Add typed public actions for the compatibility families and direct v1 actions still exercised by
   acceptance coverage. Priority, Preset recall, Preload, Output, Speed Group, Cue navigation, and
   whole-Cue deletion are complete. Standalone Playback `SET`, bare `UPDATE`, and Preset deletion/
   transfer follow as separate application-owned actions.
5. Complete public portable-show mutation seams for output routes, user layouts, standalone
   Playback/Page operations, typed undo, and any remaining Patch/setup callers. Preserve lossless
   extensions, one transaction/event, revision checks, replay, and stored-empty semantics.
6. Expand the public test DSL and migrate remaining legacy command helpers. Tests must express the
   intended operator workflow and keep software, command-line, and OSC surfaces explicit rather
   than hiding meaningful parity behind one generic implementation shortcut.
7. Remove REST/WebSocket v1 and `useServer()` compatibility only after every production caller and
   acceptance test has moved to a typed replacement.
8. Repair the remaining stale feature-plan links and keep the committed `docs/engineering` handoff
   synchronized as compatibility adapters are retired.

The command-boundary ratchet now counts **12 direct v1 `ApiDriver.command()` calls across three
files**, all deliberate compatibility or negative-envelope probes: API-004/CROSS-002 retain five
edit, target, unknown-action, and external Group-value calls; CUE-015 retains six CUE-navigation
WebSocket calls; and CUE-016 retains one whole-Cue-delete WebSocket call. There are **zero literal
categorized public compatibility-family calls**. Two shared command dispatch helpers remain
ratcheted because they can route still-untyped Preset delete/transfer and bare `UPDATE` grammar;
new scenarios may not add a direct family or raw v1 action without an exact baseline change.

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

- The shell-status slice passes the full frontend suite of 1,974 tests in 275 files, including 5 new
  tests proving a status reader is not woken by an error change and vice versa, that each reader
  rerenders for its own change, that an equivalent install publishes nothing, and that a reader
  outside a mounted boundary sees the initial snapshot. Typecheck, the production build, and all
  ratchets pass.
  Known gaps: no desktop `./build open` run and no Playwright acceptance run for this slice.

- The polled-value churn fix adds 5 tests and brings the frontend suite to 1,969 tests in 274 files;
  typecheck, the production build, and all ratchets pass. The saving is reasoned from the removed
  state-update path rather than measured on the desk.
- The desk-lock slice passes the full frontend suite of 1,964 tests in 273 files, including 4 new
  tests proving that an equivalent poll result publishes nothing, that a reader does not rerender
  across equivalent polls, that a genuine lock change does rerender, and that a reader outside a
  mounted boundary reports an unknown lock. Frontend typecheck, the production Vite build, and all
  ratchets pass.
  Known gaps: no desktop `./build open` run and no Playwright acceptance run for this slice, and the
  2 Hz saving is reasoned from the removed state path rather than measured on the desk.
- The stage-layout slice passes the full frontend suite of 1,960 tests in 272 files, including 4 new
  selector tests proving that a 3d-only change does not rerender a 2d reader, that a reader
  rerenders for its own positions, that the stored write revision is exposed, and that a reader
  outside a mounted boundary sees empty positions and revision zero. Frontend typecheck and the
  production Vite build pass with only the existing large-chunk advisory. All ratchets pass; the
  slice also split the desk capability boundaries out of `ServerProvider` to keep it under the
  150-line function limit, and split this document's superseded verification entries into
  `refactoring-verification-log.md` to keep it under the 1,200-line file limit.
  Known gaps: no desktop `./build open` run and no Playwright acceptance run for this slice.
- The desk-configuration slice passes the full frontend suite of 1,956 tests in 271 files,
  including 4 new selector tests proving unrelated-setting rerender isolation, own-setting
  invalidation, stable equality for a replaced-but-equal speed-group array, and no broad fallback
  outside a mounted boundary. Frontend typecheck and the production Vite build pass with only the
  existing large-chunk advisory. `node tools/check-architecture.mjs`,
  `node tools/check-source-size.mjs`, `node tools/test-command-boundaries.mjs`, and
  `git diff --check` all pass.
  Rust was not re-run for this slice because it contains no Rust changes; the last full Rust run
  remains 386 `light-application`, 79 `light-wire` plus generated contracts, and 408 `light-server`
  tests with 1 ignored.
  Including the writer migration the suite is 1,956 tests in 271 files.
  Known gaps: no desktop `./build open` run and no Playwright acceptance run for this slice.

- The Patch read-ownership slice passes the full frontend suite of 1,952 tests in 270 files,
  including 7 new selector tests that prove unrelated-delta rerender isolation, own-fixture
  invalidation, whole-list stability across an empty delta, head-selection resolution, caller array
  identity churn, no store listener while disabled, and no bootstrap fallback outside a mounted
  boundary. Frontend typecheck and the production Vite build pass with only the existing large-chunk
  advisory. Rust is unaffected and re-verified: `cargo fmt --all -- --check`, 386 `light-application`
  tests, 79 `light-wire` tests plus generated contracts, and 408 `light-server` tests with 1 ignored.
  `node tools/check-architecture.mjs`, `node tools/check-source-size.mjs`,
  `node tools/test-command-boundaries.mjs`, and `git diff --check` all pass.
  Known gaps: no desktop `./build open` run and no Playwright acceptance run for this slice; the
  rerender-isolation guarantees are proven at the selector level rather than by counting renders in
  each migrated window; and `StageCommandControls` still derives fallback stage positions from the
  fixture array index, so it depends on authoritative Patch ordering.

- The Group management slice passes `cargo fmt --all -- --check`; all 386 `light-application` tests
  including 12 new Group-management candidate tests; all 79 `light-wire` unit tests plus
  `cargo test -p light-wire --test generated_contracts`; and the full server library run of 408
  passing tests with 1 ignored, including 5 new Group-management route contract tests and 2 wire
  translation tests. Coverage spans property update, undo, frozen refresh, derived detach, semantic
  no-op, exact replay, rollback, and revision and source conflict; stored-empty and ordered
  membership preservation; lossless unknown fields; exactly one authoritative Show event per real
  mutation and none for a no-op or replay; selection-before-Show ordering for frozen refresh; and
  rejection of missing authentication, forged desk/user/session/show scope, and a foreign Show.
  The frontend passes 1,945 tests in 269 files, including 13 writer tests, 7 wire-decoding tests,
  and 2 provider tests; the 4 fewer tests than the previous snapshot are the deleted legacy
  `groupEditing`/`groupDerivation` adapter tests. Frontend typecheck and the production Vite build
  pass with only the existing large-chunk advisory. `node tools/check-architecture.mjs`,
  `node tools/check-source-size.mjs`, `node tools/test-command-boundaries.mjs`, and
  `git diff --check` all pass; the source-size ratchet reports 0 files above 1,200 lines and 0
  functions above 150 lines, with design-goal debt at 140 files above 400 lines and 5,879 functions
  above 20. Every new production file in this slice is below 400 lines. Wire generation still prints
  the known non-fatal `ts-rs` `deny_unknown_fields` warning.
  Not covered by this slice: no real desktop `./build open` run, no Playwright acceptance run, and
  no multi-desk frozen-refresh test at the HTTP boundary — same-Show multi-desk isolation is covered
  for Group recording but the new management family is exercised from a single desk plus explicit
  forged-scope rejection.

- Older per-slice verification evidence has moved to
  [`refactoring-verification-log.md`](refactoring-verification-log.md). It is historical: it
  records what passed when each slice landed, not the current state.

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
- Lifecycle deletion/recreation preserves monotonic exact-user values, capture, pending-values,
  queued-playback, and aggregate lifecycle authority. It invalidates old mutation replays and emits
  only the final safe projections.
- Public bootstrap no longer contains Programmer state. The authenticated v1 compatibility list
  is restricted to same-user session rows and remains only for startup and the shrinking
  compatibility surfaces tracked above.
- Patch/setup selection and explicit Cue pending-choice authority are complete. Public Priority,
  Preset recall, hosted-picker, session, desktop, OSC, Preload, Output, normal Programmer values,
  selection, Playback GO, Speed Group, Cue navigation, and whole-Cue deletion intents are complete.
  The only direct v1 command calls left are the explicitly retained probes enumerated above.
- Preload now prepares one final-state-aware batch, and virtual-exclusion restart authority is
  private, desk-exact, migration-compatible, and absent from public runtime projections.
- Recommended next slice: implement typed Group management end to end, then migrate the remaining
  Patch readers before removing broad Patch bootstrap state. Follow with configuration/layout and
  residual portable-show actions, Preset delete/transfer, standalone Playback `SET`, and bare
  `UPDATE`. Keep repository-wide acceptance, desktop verification, and reference-hardware
  performance as the closing milestones.

Test files may exceed the hard limits, but should still be split when it improves readability and
makes operator intent more visible.
