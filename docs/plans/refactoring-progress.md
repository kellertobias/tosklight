# Major Refactoring Progress

Estimated progress: **96%**

Estimated Codex ETA: **roughly 18–30 hours of active Codex execution**, to repository-wide
acceptance. Typed Cue transfer is complete. Physical Playback authority still requires an exact
map-existing action and a page create/rename action before the broad Playback snapshot can be
retired; the remaining compatibility callers, public test-DSL handoff, and final performance and
desktop acceptance follow. This corrected estimate reflects the audited compatibility surface,
not a regression in completed work.

This is the living handoff for [`major-refactoring.md`](major-refactoring.md). Update it after each
meaningful milestone. A checked item means the implementation is committed on `refactoring` and
has focused verification; it does not replace the final repository-wide acceptance run.

Last updated: 2026-07-20 after completing typed Cue COPY/MOVE Plain/Status authority from the
Programming application boundary through the scoped frontend writer.

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
  reporting is complete. Portable topology and Cuelist mutation are complete below, while the
  physical compatibility panes remain.
- [x] Removed mutable Playback-service lock exposure from the migrated paths: Engine callers use
  typed commands and immutable projections, Preload installs generation-bound prepared batches,
  and application-owned units of work serialize page changes, automatic render transitions, and
  semantic event publication.
- [x] Added the view-scoped Playback runtime frontend: exact v2 snapshots and WebSocket filters are
  activated only by mounted Playback/Cuelist views, desk-only views request no runtime identities,
  and gaps and malformed messages repair from authoritative snapshots. Concurrent fader and page
  mutations use independent optimistic overlays with request-ordered rollback and authoritative
  event/outcome reconciliation. Unmigrated physical panes remain on the broad v1 `/playbacks`
  snapshot; they do not poll periodically. Real Playback/Page topology events trigger a coalesced
  compatibility reload, and legacy mutation callers may issue an additional explicit reload.
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
- [ ] Finish the remaining Playback ownership callers: add exact typed map-existing Playback and
  page create/rename actions, move the physical bank and companion page tools onto committed
  topology/runtime/desk boundaries, then retire broad `/playbacks` consumers. Update, Cue transfer,
  Cue editor/Cuelist settings, and Virtual Playback are complete.
- [ ] Move the remaining selection consumers onto the scoped Programming store, then remove their
  legacy bootstrap fields and broad Programmer refresh paths. Group Pool, Group Strip, and the
  command bar, Stage, Stage/Fixture pane chrome, Channels, Fixture Sheet, Patch, and Presets have
  moved, as have Patch setup, the complete parameter bank, and selection-driven operator modals;
  a small number of keypad/miscellaneous readers still use the facade.

## Remaining architecture work

1. Complete physical Playback authority without changing desk semantics. Add a typed
   `map_existing_playback` action that retains the existing Playback number instead of allocating a
   clone, then add typed page create/rename. Migrate the bank, configuration, page tools, hardware
   summary, shortcuts, Numeric Pad, secondary screens, and Product Demo onto exact topology,
   runtime, Group, and desk projections before deleting the broad `/playbacks` snapshot.
2. Publish the remaining externally observable transitions once through typed events: Highlight
   movement, transition completion, output health/overload, and any remaining automatic runtime
   changes.
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

The audited public test boundary still contains 145 direct v1 WebSocket commands, 40 legacy text
commands, and 142 generic show-object writes. The first test-DSL migration should converge visible
command-line, software-key, and OSC command intent while retaining one explicit v1 compatibility
test. Missing typed seams for priority, Preset recall, output-route/user-layout mutation, session
handoff, and DesktopBridge must remain visible work rather than being hidden behind a generic test
helper.

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
- The scoped activity-summary cohort passes 19 focused tests covering capture-first dormancy,
  mutually exclusive normal/pending hydration, route replacement, scalar rerender suppression,
  stale-bootstrap refusal, same-user System Controls rows, and foreign-user compatibility rows.
  The remaining writer cohort passes 61 focused tests across the shared bounded write queue,
  parameter controls, Channels, modal selection, one-batch Position ticks and Home, inactive-view
  dormancy, API facade removal, and frontend typecheck. Numeric Pad and keypad coverage passes all
  26 focused tests after typed selection/value clearing. Dependency checks and all 10 architecture
  scanner tests pass; the architecture command still exits non-zero only for the separately owned
  Dynamics Editor experiment at 1,382 lines.
- The Fixture Sheet target reader and compatibility-hydration slice passes 22 focused values,
  filter, window, loading, logical-head, and dormancy tests plus 40 server-routing tests. Focused
  Rust tests cover normal and pending value categorization, user/desk identity, transient
  retrigger/release, and timed-pulse completion. Frontend typecheck, `cargo fmt --all -- --check`,
  and `git diff --check` pass. The changed routing and server production modules are below 400
  lines; malformed, foreign, and mixed compatibility events retain the conservative fallback.
- The Patch/setup migration and semantic correction pass 69 focused Patch, selection-provider,
  window-activation, parameter-control, and Patch-feature tests across 8 files plus the full
  frontend typecheck. Tests cover hidden-view dormancy, exact selection-only subscription,
  optimistic closed-selection preservation, ordered range/additive replacement, logical-head
  toggle/removal, split and parameter selection, and authoritative post-placement head targets.
  `git diff --check`, dependency directions, and all 10 architecture scanner tests pass; the
  architecture command still exits non-zero only for the pre-existing committed 1,382-line
  Dynamics Editor experiment. Every changed production file remains below 400 lines.
- The Programmer lifecycle slice passes the complete 215-test application suite, 69 Programmer
  tests before the later queue additions, 29 wire tests plus generated-contract verification, 6
  focused server lifecycle tests, and 31 focused frontend tests. The row-level selection-count
  correction separately passes 6 Programmer, 9 application, 3 wire, and 6 server lifecycle tests.
  Dependency directions, all 10 scanner tests, frontend typecheck, and `cargo fmt` pass.
- The queued Preload playback backend passes all 73 Programmer tests, all 219 application tests,
  all 30 wire tests plus generated-contract verification, 3 focused snapshot/subscription server
  tests, the successful capture/replay/GO-drain scenario, and failed-GO rollback. The final
  generated DTO matches the strict frontend decoder. Eight frontend files pass 82 focused tests;
  the complete frontend run passes 1,093 tests in 174 files, typecheck, and the production build
  with only the existing large-chunk advisory. Review confirms first-view dormancy, exact-user
  routing, stale-bootstrap refusal, scope replacement, gap repair, and unrelated-render isolation.
- Public Programmer-state hardening passes the focused unauthenticated-bootstrap, authentication,
  same-user/foreign-user server tests, the pre-clone user-filter Programmer test, 18 API-client
  tests, frontend typecheck, `cargo check -p light-server --no-default-features`, `cargo fmt`, and
  `git diff --check`. The server library passes 254 tests with 1 ignored when the CITP thumbnail
  socket test is excluded; that one test cannot bind under the sandbox. Thirteen stale absolute
  event-sequence assertions exposed by login lifecycle publication now use post-setup baselines
  while retaining exact per-action count and ordering checks.
- Typed Preset recording passes all 76 `light-programmer` tests, all 233 `light-application`
  tests, all 35 `light-wire` unit tests and generated-contract verification, and all 31 focused
  command HTTP server tests. The server set covers authoritative changed/no-change outcomes,
  revision conflict, replay, authentication, forged values, Preload rejection, one Show event per
  real mutation, command-line convergence, a real OSC key sequence, and WebSocket replay without
  repeated interaction side effects. `cargo check -p light-server --no-default-features` and
  `cargo fmt --all -- --check` pass. The frontend passes 82 focused Preset/Show Objects/API tests
  and the complete 1,124-test suite in 178 files; typecheck and the production build pass with only
  the existing large-chunk advisory. Dependency directions, all 10 source-size scanner tests,
  generated contracts, and `git diff --check` pass. The repository-wide source-size command still
  exits non-zero for two pre-existing committed ratchet findings outside this slice: the
  1,382-line Dynamics Editor experiment and the 154-line `handle_subscription_osc` function.
  Every production file and function changed by Preset recording remains within the hard limits.
- Typed Group recording passes all 83 `light-programmer` tests, all 250 `light-application` tests,
  all 42 `light-wire` unit tests plus generated-contract verification, and the full server library
  with 272 passing, 1 ignored, and only the sandbox-blocked CITP test filtered. Independent focused
  review additionally passes 7 Group domain tests, 9 Group service tests, 8 active-show transaction
  tests, 7 wire tests, 11 Group server tests, and the compatibility WebSocket scenario. The frontend
  passes all 1,161 tests in 182 files, typecheck, and the production build with only the existing
  large-chunk advisory. Coverage includes overwrite/merge/subtract/delete, stored-empty and no-op,
  replay, conflict rollback and exact-object repair, both event/response orders, deletion settlement,
  same-user two-desk capture, foreign-user and forged-context rejection, one Show event per mutation,
  gesture-event ordering, direct/nested Highlight reconciliation, first-action dormancy, authority
  replacement, and successful-command reset versus failure retention. `cargo fmt --all -- --check`,
  generated contracts, dependency directions, all 10 scanner tests, and `git diff --check` pass.
  The repository-wide architecture command still exits non-zero only for the same unrelated
  1,382-line Dynamics experiment and 154-line `handle_subscription_osc`; every production file
  changed by Group recording remains below 400 lines and every changed production function remains
  below the hard limit.
- Typed Cue recording passes all 87 `light-programmer` tests, 268 `light-application` tests, 66
  `light-playback` unit tests plus 4 automatic-transition tests, 55 `light-show` tests, 79
  `light-fixture` tests, and 47 `light-wire` tests plus generated-contract verification. All 50
  focused command HTTP tests and all 16 Cue-recording server tests pass. The complete server library
  passes 296 tests with 1 ignored when the sandbox-blocked CITP socket test is skipped; the
  unfiltered run fails only that test with `Operation not permitted`. The frontend passes all 1,230
  tests in 189 files, typecheck, and the production build with only the existing large-chunk
  advisory. The 27 focused Cue semantics and hardware-connected Playback API/UI scenarios pass
  serially outside the sandbox. Coverage includes normal and Preload capture, overwrite/merge/
  subtract/delete, topology creation, take-live, no-op, replay, conflict rollback, one authoritative
  event, same-user desks, foreign-user rejection, exact snapshot repair, both optimistic ordering
  races, scope replacement, first-view dormancy, no broad bootstrap request, and unrelated-render
  suppression. Strict Clippy, formatting, dependency directions, all 10 source-size scanner tests,
  generated contracts, and `git diff --check` pass. The source-size command now reports only the
  pre-existing 1,382-line Dynamics experiment; OSC subscription handling was split below the hard
  function limit. `./build open` succeeds and `/api/v1/readiness` reports `ready` with no current-
  launch server error. Wire tests retain the existing non-fatal `ts-rs` warning.
- Explicit Cue pending-choice authority passes all 88 `light-programmer` tests, all 271
  `light-application` tests, all 47 `light-wire` unit tests plus generated-contract verification,
  64 focused command HTTP tests, the Cue-transfer and 3 engine-selection-refresh regressions, and
  the accepted compatibility Speed reset regression. Four focused frontend files pass all 44
  modal, view, store, writer, rollback, narrow-selector, and authority-replacement tests; frontend
  typecheck passes. All 8 Cue arbitration/transfer and Speed API/UI Playwright scenarios pass
  serially outside the sandbox. Strict Clippy for Programmer, application, wire, and server,
  `cargo fmt --all`, and `git diff --check` pass. The accepted Speed path exposed and fixed two
  operator-visible integration defects: compatibility execution now persists the authoritative
  command reset before its single retained event, and Programming snapshots acquire the user gate
  before the desk gate so the globally mounted exact-desk observer cannot deadlock startup. Every
  changed production file remains at or below 400 lines. `./build open` rebuilds and launches both
  desktop bundles, `/api/v1/readiness` reports `ready`, and the current launch adds no server error;
  Vite retains only its existing large-chunk advisory and wire generation retains the existing
  non-fatal `ts-rs` warning.
- Atomic Playback exclusions and scoped related-outcome reconciliation pass all 89 Programmer, 61
  Engine, and 273 application library tests. The server library passes 315 tests with 1 intentional
  Matter-port ignore when the CITP thumbnail socket test is filtered; that CITP test passes
  separately outside the sandbox. Focused coverage passes 9 Engine Playback-boundary tests, 16
  Playback application tests, 17 v2 route tests, 28 Preload tests, the restored-exclusion
  normalization test, all 4 output-scheduler tests, and 7 Matter tests. The prior parallel-only Cue
  transfer failure also passes alone. The 48 wire tests and generated-contract verification pass;
  strict Clippy for Engine, application, wire, and server passes with the established
  `too_many_arguments` allowance. `cargo fmt --all -- --check` and `git diff --check` pass. The
  focused frontend contract passes 74 Playback, Preload-queue, store, session, view, and adapter
  tests; request-correlation and command-summary regressions add 30 passing tests. The complete
  frontend suite passes all 1,260 tests in 190 files; typecheck and the production build pass, with
  only the existing Vite chunk-size advisory. Every changed production file remains below 400
  lines; the Playback ports module is 352 lines after moving its focused test into a 60-line
  feature-owned test module. The focused VPB-007, CUE-005, and OSC-006 API/UI/OSC acceptance paths
  pass. OSC-006 now acknowledges
  the expected page value instead of attributing an earlier UDP feedback packet to the subsequent
  page transition; its final API/UI/OSC run passes all 3 paths. `./build open` rebuilds and launches
  both desktop bundles; `/api/v1/readiness` reports `ready` at snapshot revision 30 in 0.073 seconds,
  `/health` returns 200 in 0.001 seconds, and `/bootstrap` returns 200 in 0.003 seconds. The current
  launch reaches Engine-ready/server-bind state without a new log error.
- Exact Playback runtime effects and domain-specific persistence pass all 21 `light-control` tests,
  all 80 `light-playback` unit tests plus 4 automatic-transition integration tests, all 69 Engine
  unit tests plus its integration test, and all 276 application tests. The server library passes
  334 tests with 1 intentional Matter-port ignore and the CITP thumbnail socket test filtered;
  all 23 focused v2 route tests prove repeat no-change outcomes, transient holds, independent
  persistence sentinels, an all-no-op Preload drain, peer-only auto-off publication, timed-XFade
  retriggering, and hidden addressed Preload publication. The 48 wire tests and generated-contract
  test pass, as does the control-ui typecheck.
  Strict Clippy passes for Control, Playback, Engine, application, and server with the established
  `too_many_arguments` allowance; `cargo fmt --all -- --check`, the no-default-features server
  check, and `git diff --check` pass. Dependency directions and all 10 architecture scanner tests
  pass; the aggregate architecture command still exits 1 solely for the separately owned
  1,382-line Dynamics Editor experiment. Every production file touched by this slice remains below
  400 lines.
- Final-state Preload preparation and activation provenance pass all 80 `light-playback` unit tests
  plus 4 automatic-transition integration tests, all 73 Engine unit tests plus its integration
  test, all 90 `light-programmer` tests, 21 focused Engine Playback-boundary tests, 33 focused
  server Preload tests, and 25 focused v2 route tests. The server library passes 339 tests with 1
  intentional Matter-port ignore and only the sandbox-blocked CITP thumbnail socket test filtered.
  Coverage includes transient mutate/cancel no-op behavior, final-action event attribution,
  captured origin across same-user desks, explicit-page no-scope behavior, deskless Matter
  checkpoint/restore, false-to-true-only stamping, stable ordinals at equal timestamps, provenance
  preservation through Cue/timing changes and timed release fades, release clearing, legacy
  migration, both two-desk activation orders, deleted-desk isolation, public-payload omission,
  private-checkpoint retention, and idempotent repeat startup normalization. All 48 wire tests and
  generated-contract verification, control-ui typecheck, strict Clippy, the no-default-features
  server check, `cargo fmt --all -- --check`, and `git diff --check` pass. Dependency directions
  and all 10 architecture scanner tests pass; the aggregate architecture command still exits 1
  solely for the separately owned 1,382-line Dynamics Editor experiment. Every touched production
  file remains below 400 lines; the largest is Playback ports at 393 lines.
- Portable Playback topology and the Virtual Playback migration pass 10 focused application tests,
  4 wire tests, generated-contract verification, 5 topology route tests, 2 scoped-zone route tests,
  the stale-Show runtime-action guard, and the legacy Speed Group migration regression. The
  no-default-features server check, formatting, strict application/wire/server Clippy, and
  `git diff --check` pass; Clippy and wire generation retain only the known non-fatal `ts-rs`
  `deny_unknown_fields` warning. The complete frontend passes all 1,341 tests in 199 files,
  typecheck, and the production build. Focused coverage proves strict relational outcomes and
  status-aware object revisions, legacy storage identities, one-action configure/clear, response
  ordering, replay/no-change/conflict repair, same-Show authority replacement, Show-switch
  rejection, held Flash/Swap release, shared zone-cache/save ordering, inactive-view dormancy,
  no bootstrap or broad Playback read, and unrelated-render suppression. The Vite build retains
  its existing large-chunk advisory. Dependency directions and all 10 architecture scanner tests
  pass; the aggregate architecture command still exits 1 only for the separately owned 1,382-line
  Dynamics Editor experiment. New feature-owned production modules remain below 400 lines; the
  existing shared Show Objects session/store files received only narrow dormancy hooks and remain
  below the hard 1,200-line limit. One coalesced v1 `/playbacks` compatibility reload remains after
  real topology changes so unmigrated physical panes stay current; legacy physical mutation callers
  may also request an explicit reload. Scoped Virtual and Cue-editor paths never consume the broad
  projection. Empty-slot assignments display the authoritative server allocation rather than a
  speculative grid identity.
- Cue-editor/Cuelist topology convergence passes 11 application tests, 4 strict wire tests,
  generated-contract verification, 5 server route tests, 41 focused frontend tests, frontend
  typecheck/build, and all 3 CUE-011 API/UI/supplemental browser paths. Coverage proves required
  nullable storage-identity preconditions, revision and replacement conflicts, lossless extension
  retention, one-action settings and renumber saves, rapid-edit response/event ordering, queued
  failure cancellation, repaired-authority retry, same-ID writer/session replacement, no broad
  `refresh()` or generic Cuelist reload, and the stale-dialog E2E conflict path. Strict application,
  wire, and server Clippy passes with the established `too_many_arguments` allowance; formatting
  and `git diff --check` pass. Dependency directions and all 10 architecture scanner tests pass;
  the aggregate architecture command still exits 1 only for the separately owned 1,382-line
  Dynamics Editor experiment. Every touched production file remains below 400 lines.
- Programming Update convergence passes all 26 focused application tests, all 8 strict wire tests,
  generated-contract verification, and the 14-test server `update` filter including all 12 Update
  application/compatibility/v2 route paths. Strict Clippy for application, wire, and server,
  `cargo fmt --all -- --check`, and `git diff --check` pass; wire generation retains only the known
  non-fatal `ts-rs` `deny_unknown_fields` warning. The frontend passes 53 focused transport,
  decoder, writer, provider, workflow, and legacy-facade tests plus the complete 1,379-test suite
  in 203 files, typecheck, and the production build with only the existing Vite chunk advisory.
  The focused UPDATE-001 API/UI acceptance pair passes outside the sandbox. Coverage proves strict
  DTO mapping, exact request/revision/scope authority, one-action direct apply, safe retry/replay,
  both HTTP/event orders, pending abandonment, narrow Group and legacy-key Cue conflict repair,
  FIFO rebasing, late-response rejection, provider dormancy, stable unrelated renders, settings
  ownership, and absence of bootstrap or broad Playback requests. Dependency directions and all
  10 architecture scanner tests pass. Every new production file remains below 400 lines and the
  split `UpdateWorkflow` function is 59 lines; the committed aggregate source-size exception for
  the isolated Dynamics Editor experiment remains outside this slice.
- Typed Cue transfer passes all 91 Programmer tests, 8 focused application tests, 4 Playback
  transfer tests, 4 wire transfer tests, generated-contract verification, 7 focused server route/
  compatibility tests, and 6 targeted server regressions covering the four Plain/Status and
  Move/Copy axes, choice reset, v1-notification isolation, Preset ownership, replay after choice
  resolution, and desk-scoped command-line idempotency. The doubly stale Show regression proves a
  conflict reports the current ActiveShow revision rather than the older pending-choice revision.
  Strict Clippy for the touched Rust crates, `cargo fmt --all -- --check`, and `git diff --check`
  pass. Eight focused frontend files pass all 71 transport, decoder, provider, writer, modal, and
  Programming-interaction tests; the full frontend run passes 1,395 tests in 207 files and
  typecheck passes. Coverage includes forged-scope rejection, exact one-event mutation
  cardinality, both HTTP/event orderings, rollback, replay/no-change, narrow conflict and cursor
  repair, dormant action-only composition, scope replacement, and late response rejection. A
  future route hardening test should still exercise valid same-user peer-desk credentials and a
  second authenticated user rather than only forged contexts. Wire generation retains only the
  known non-fatal `ts-rs`
  `deny_unknown_fields` warning; Serde and checked-in schemas remain strict.

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
- Patch/setup selection and explicit Cue pending-choice authority are complete. The public test DSL
  remains a separate future milestone.
- Preload now prepares one final-state-aware batch, and virtual-exclusion restart authority is
  private, desk-exact, migration-compatible, and absent from public runtime projections.
- Recommended next slice: add typed `map_existing_playback` and migrate the physical Playback bank
  onto exact topology/runtime/desk authority. Preserve the existing Playback number, hardware and
  touch geometry, Record/Update interception, explicit-page behavior, `surface: "physical"`, and
  Flash/Swap release cleanup. Page create/rename and the remaining broad Playback consumers follow;
  keep the public test DSL and final repository-wide acceptance/performance run as the closing
  milestones.

Test files may exceed the hard limits, but should still be split when it improves readability and
makes operator intent more visible.
