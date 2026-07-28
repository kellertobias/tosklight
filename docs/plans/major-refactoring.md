# Major Architecture and Extensibility Refactor

## Goal

Refactor ToskLight around clear, typed capability boundaries so new features can be added without routing every change through the server binary, the global frontend context, or parallel REST, WebSocket, OSC, and persistence implementations.

The refactor preserves:

- the visible desk layout, labels, geometry, gestures, and hardware/software behavior;
- exact OSC paths, feedback, aliases, and desk-sharing semantics;
- existing show, desk, fixture-profile, and layout data; and
- documented Programmer, Playback, Group, Cue, Highlight, Preload, Move in Black, and output behavior.

Internal Rust and TypeScript APIs and the REST/WebSocket v1 implementation may break during the coordinated release. The resulting architecture must make Dynamics, Timecode, bidirectional REST-controlled fixtures, ATEM or sound-mixer integrations, and future programmable Macros possible without another application-wide rewrite. These future features are extension tests for the architecture, not part of this refactor's implementation scope.

```mermaid
flowchart LR
  UI["UI / Keyboard"] --> ADAPTERS["Input adapters"]
  OSC["OSC / Hardware"] --> ADAPTERS
  HTTP["HTTP command API"] --> ADAPTERS
  MIDI["MIDI / Matter"] --> ADAPTERS
  MACRO["Future Macro Runtime"] --> ADAPTERS
  SCHEDULE["Future Schedule Service"] --> MACRO

  ADAPTERS --> APP["Typed application services"]

  APP --> PROG["Programming Service"]
  APP --> PLAY["Playback Service"]
  APP --> DYNAMIC["Future Dynamic Runtime Service"]
  APP --> SHOW["Active Show Service"]
  APP --> DESK["Desk / Session Service"]
  APP --> OUTPUT["Output Service"]
  APP --> ASSETS["Managed Asset Service"]

  SHOW --> STORE["Typed persistence facade"]
  SHOW --> COMPILER["Show Compiler"]
  PROG --> DYNAMIC
  PLAY --> DYNAMIC
  COMPILER --> ENGINE["Render and arbitration engine"]
  PROG --> ENGINE
  PLAY --> ENGINE
  DYNAMIC --> ENGINE
  ENGINE --> OUTPUT

  OUTPUT --> DMX["Art-Net / sACN / future USB"]
  OUTPUT --> DEVICE["Future bidirectional device adapters"]
```

## Current architectural pressure

- `crates/light/adapters/headless/src/main.rs` is currently the application as well as the process entry point. It combines startup, 93 routes, sessions, Programmer, Playback, show mutation, migration, OSC, Matter, output, media, files, events, and more than eight thousand lines of inline tests.
- `apps/light-desktop/src/api/ServerContext.tsx` combines transport, authentication, reconnection, event routing, cached server state, optimistic mutations, errors, and almost every feature command in one context exposed to most of the UI.
- Rust wire types, handwritten TypeScript types, generic JSON show objects, string WebSocket commands, and OSC paths form parallel interfaces that can drift.
- REST, WebSocket, command-line, OSC, MIDI mappings, Matter, and UI paths frequently reproduce orchestration instead of adapting into one authoritative application action.
- Fixture, Programmer, Playback, Show, Control, Output, and Engine crates contain useful domains, but their public facades expose mutable internals or mix stable models with transport and runtime adapters.

The objective is not merely to shorten these files. The refactor must create ownership boundaries that prevent unrelated features from converging in new global modules.

## Core responsibilities and interfaces

### Application layer

- Add `light-application` as the shared use-case layer.
- Define bounded command families such as `ProgrammerCommand`, `PlaybackCommand`, `ShowCommand`, `DeskCommand`, and `OutputCommand`. Future capabilities add their own bounded families instead of extending a universal god enum.
- Every action carries an `ActionContext` containing desk, user, session, source surface, correlation or request identity, and the applicable expected revision.
- UI, OSC, HTTP, MIDI, Matter, Macros, Cues, and Timecodes call the same application services. Adapters parse, authenticate, normalize addressing, and translate; they do not implement business rules.
- Replace the server's shared state bag with service-owned state and locks. Services expose commands and immutable query projections rather than their underlying mutexes or registries.
- Define and document six state lifetimes:
  - portable show;
  - desk installation;
  - desk interaction;
  - user Programmer;
  - connection or session; and
  - transient runtime.
- Every new state field or object must declare its lifetime, persistence location, migration policy, reconnect behavior, restart behavior, Save As behavior, and deletion behavior.

### Server and wire contracts

- Make `crates/light/adapters/headless/src/main.rs` a thin configuration and lifecycle entry point.
- Move routers, startup, shutdown, scheduling, OSC, WebSocket, Matter, media, files, and output orchestration into feature-owned server-library modules.
- Add `light-wire` containing versioned request, response, command, outcome, error, and event DTOs plus schemas.
- Generate checked-in TypeScript definitions from the wire contract and verify the generated files in CI.
- Validate decoded server responses and events at the frontend transport boundary. Keep wire DTOs separate from frontend feature and view models.
- Replace internal string-plus-JSON commands and events with discriminated types. Serialization belongs only in transport adapters.
- Keep OSC exactly compatible. REST/WebSocket v1 may remain temporary adapters and be removed after the UI and tests migrate to the replacement interfaces.

### Engine and runtime event publication

- Every externally observable state transition produced inside Engine, Playback, Programmer, Control, Output, or another runtime service must have a typed domain event. This includes automatic changes, not only operator commands: Chaser steps, FOLLOW/TIME/timecode Cue advances, transition completion, loaded-Cue changes, playback release, Programmer ownership changes, Highlight movement, output-health changes, and future Dynamic or Macro state changes.
- Domain services publish these events through an application-owned event bus. The render engine must not know about WebSocket clients, frontend stores, OSC serialization, or other transport subscribers; adapters translate typed domain events into the appropriate wire or feedback contract.
- Clients subscribe explicitly by capability, object identity, desk, and event class. A UI may subscribe to only the selected Cuelist's Cue transitions, for example, while another surface subscribes to playback summaries or output health. Not subscribing remains valid; clients can continue to use authoritative query projections and snapshots.
- Events describe meaningful state boundaries, not every render sample, DMX frame, or intermediate fade value. High-frequency values use retained projections, client-side interpolation from authoritative timing metadata, bounded sampling, or an explicitly requested telemetry stream.
- Subscription delivery must support filtering, coalescing, per-topic rate limits, bounded queues, backpressure policy, and priority classes. Safety, command outcomes, errors, and discrete state transitions must not be dropped; replaceable telemetry and progress updates may be collapsed to the newest value.
- Every event carries a monotonic sequence or projection revision, event time, source, correlation identity where applicable, and enough stable identity to request the authoritative current projection. Reconnect detects sequence gaps and repairs them with a snapshot before resuming incremental delivery.
- One semantic transition produces one domain event regardless of whether it originated from UI, OSC, HTTP, MIDI, Matter, a Chaser timer, Timecode, a Macro, or another internal scheduler. Adapters must not synthesize competing client-local state or require broad polling to discover ordinary runtime changes.

### Show management and persistence

- Introduce `ActiveShowService` as the only application boundary for active-show mutation.
- Centralize candidate decoding and migration, validation, compilation, backup, atomic revisioned persistence, runtime replacement, adapter reconciliation, audit, and event publication.
- Keep generic JSON object storage internal to `light-show`. Application code uses typed objects and repositories or codecs at the capability boundary.
- Split desk and portable-show schemas, stores, and migration modules inside `light-show` while preserving their current physical separation and data formats.
- Preserve unknown show objects during load, Save As, revision creation, and export so older or newer shows are not destructively normalized.
- Add `SelectiveShowImportService` for loading selected objects from another show. It previews dependencies and conflicts, preserves IDs where possible, skips identical objects, rewrites references when duplicating, and applies the complete import atomically.
- Macros, Dynamics, Presets, Groups, fixture-related objects, and future Timecodes use this general selective-import workflow rather than implementing feature-specific copy paths.

## Programmer and value flow

```mermaid
flowchart TD
  INPUT["UI / Keyboard / OSC / HTTP / Macro"] --> ADAPT["Input adapter"]
  ADAPT --> ACTION["Typed application command"]

  ACTION --> INTERACTION["Desk Interaction State<br/>command line, target, Shift/gesture, page"]
  ACTION --> PROGRAMMER["User Programmer State<br/>ordered selection and semantic values"]
  ACTION --> PLAYBACK["Playback Runtime<br/>active/loaded Cue, masters, transitions"]
  ACTION --> SHOWMUT["Show Mutation<br/>Groups, Presets, Cues, patch"]

  SHOWSTORE["Portable Show Store"] --> COMPILER["Show Compiler"]
  LIBRARY["Desk Fixture Library"] --> PATCH["Portable patched profile snapshots"]
  PATCH --> COMPILER
  COMPILER --> COMPILED["Compiled Show<br/>fixtures, attributes, Groups, Cues, bindings"]

  PROGRAMMER --> ACTIVE["Active semantic attribute programs"]
  PLAYBACK --> ACTIVE
  PRELOAD["Preload semantic attribute programs"] --> ACTIVE
  FUTURE["Future animated attribute programs"] --> ACTIVE

  ACTIVE --> CONTROL["Resolve source priority and control claims"]
  CONTROL --> SAMPLE["Sample applicable animated values"]
  ACTIVE --> VALUES["Static values and releases"]
  SAMPLE --> MERGE["Attribute arbitration"]
  VALUES --> MERGE

  MERGE --> PRIORITY["LTP / HTP / ownership"]
  PRIORITY --> TRANSITIONS["Fade, delay, MIB and masters"]
  HIGHLIGHT["Transient Highlight overlay"] --> TRANSITIONS
  TRANSITIONS --> RESOLVED["Resolved semantic fixture values"]

  COMPILED --> PROJECT["Fixture projection"]
  RESOLVED --> PROJECT
  PROJECT --> DMX["DMX frames"]
  PROJECT --> INTENTS["Future external-device intents"]

  DMX --> OUTPUT["Output Service"]
  INTENTS --> OUTPUT
  OUTPUT --> DRIVERS["Art-Net / sACN / device adapters"]

  RESOLVED --> PROJECTION["Authoritative UI/OSC projection"]
  PROGRAMMER --> PROJECTION
  PLAYBACK --> PROJECTION
  PROJECTION --> CLIENTS["UI / OSC feedback / HTTP reads"]
```

The flow enforces these rules:

- Desk interaction state owns unfinished command-line text, the current command target, Shift and gesture context, and the desk page.
- Programmer state owns the user's ordered selection expression, semantic values, timing, modes, and undo/redo history.
- Groups, Presets, Cues, patching, and other portable definitions live in the show.
- Programmer, Playback, Preload, and Dynamics produce semantic attribute values; they never write DMX directly.
- Highlight remains a transient overlay and is never recorded into Programmer or Cue data.
- Programmer LTP and Playback arbitration remain distinct.
- Group Masters are independent HTP intensity limiters, never LTP or lowest-takes-
  precedence limiters and never parents or masters of one another. Only Groups that are
  actually assigned as Group Master playbacks participate. Ordinary overlapping Group
  membership does not suppress output, and overlapping active Group Masters contribute
  by HTP: the highest applicable Group Master level wins before the Grand Master is
  applied once above it.
- Recording and Update pass through `ActiveShowService`; the render engine never writes persistence.
- The render engine receives immutable compiled-show and contribution snapshots.
- UI and OSC feedback derive from authoritative projections, never client-local approximations.

### Extensible semantic values

Every resolved value is addressed by fixture or logical head plus attribute. The internal value boundary must support static and animated values without changing transport, storage, fixture projection, or output adapters.

The canonical [Dynamics plans](Later/dynamics/README.md) define static, Dynamic On, Dynamic Off, `FixAT`/FAT, release, pause, and hidden-running behavior. This refactor does not implement those product features; it ensures that the value and contribution interfaces are not restricted to stateless numeric producers.

This is sufficient for the planned Dynamics behavior:

- a Group or ordered selection can receive a Dynamic through Programmer;
- the same semantic assignment can be staged in Preload or recorded into a Cue;
- combined Dynamics can produce independent values for multiple attributes;
- different Playbacks can own independent runtime instances; and
- a Fixed At value can suppress an animated value at the normal fixture/head-and-attribute arbitration boundary.

A suppressed Dynamic instance continues running while hidden and resumes output if it becomes the winner again. Pause, Dynamic Off, and restart remain distinct typed operations and do not require another system-wide architectural change.

## Fixture management

```mermaid
flowchart LR
  PACKAGE["Fixture package<br/>profile, modes, channels, assets"] --> LIB["Desk Fixture Library"]
  LIB --> REV["Immutable profile revision"]
  REV --> PATCH["Patch fixture into show"]
  PATCH --> SNAPSHOT["Portable show profile snapshot<br/>one per immutable revision"]
  SNAPSHOT --> FIXTURE["PatchedFixture<br/>snapshot ID + selected mode ID"]
  FIXTURE --> SHOW["Show Store"]

  SHOW --> COMPILER["Show Compiler"]
  COMPILER --> CF["Compiled Fixture"]
  CF --> ATTR["Semantic attributes and logical heads"]
  CF --> BINDING{"Output binding"}

  BINDING -->|DMX| DMX["Universe/address/splits/multipatch"]
  BINDING -->|None| UNPATCHED["Unpatched but programmable"]
  BINDING -->|Future external| EXT["REST/ATEM/mixer adapter binding"]

  ATTR --> ENGINE["Programmer and render engine"]
  ENGINE --> BINDING
  EXT --> OBSERVED["Observed device state"]
  OBSERVED --> UI["Runtime projection and UI feedback"]
```

- `FixtureLibraryService` owns desk-wide fixture packages, immutable revisions, validation, photographs, icons, GLB assets, and import/export.
- `ShowPatchService` owns fixture numbers, stable IDs, selected mode, logical heads, addresses, split patching, multipatch instances, stage transforms, Highlight overrides, and external-device binding references.
- On first use, patching copies the immutable profile revision into a portable show-level snapshot keyed by stable revision identity and verified by content digest. Each `PatchedFixture` stores only the snapshot reference and selected mode ID; fixtures using the same revision never inline duplicate profiles or modes. Later library revisions never silently alter an existing show; upgrading a patched fixture is an explicit revisioned show mutation.
- `ShowCompiler` converts portable patch objects into `CompiledFixture` instances containing semantic attributes and output bindings.
- An unpatched fixture receives no output binding but remains selectable, programmable, groupable, recordable, and visible.
- Logical heads and multipatch instances retain stable identity across recompilation and migration.
- The render engine knows semantic attributes and compiled bindings, not SQLite, REST, fixture-library UI, or network connection details.
- Future bidirectional fixtures use the same Programmer, Preset, Dynamic, Cue, and Playback paths. Desired desk state and observed device state remain separate runtime concepts until their authority policy is specified.

### Patch mutation performance contract

- `ShowPatchService` exposes one revisioned `PatchFixtures` command for a batch of one or more fixtures. It resolves each unique profile revision once, validates fixture numbers, virtual fixture numbers, addresses, splits, and conflicts against the complete candidate batch, and either applies the complete batch or applies nothing.
- `ActiveShowService` processes a patch batch as one show transaction: one candidate migration and validation pass, one backup, one atomic persistence revision, one `ShowCompiler` run, one runtime replacement, and one patch-change event. A fixture count must never become a loop of generic show-object requests, backups, or full-show compilations.
- The command request refers to a library profile revision and selected mode instead of sending the fixture catalog or a complete profile per fixture. Its size is proportional to the number of fixtures plus the number of unique profile revisions first introduced into the show, never fixtures multiplied by profile modes.
- The command outcome and patch-change event contain the created or changed fixture projections, their IDs, and the resulting show and patch revisions. The Patch frontend store applies that delta directly. Patching must not refresh bootstrap, Playbacks, show lists, configuration, media state, fixture profiles, or the fixture catalog; those caches refresh only for their own versioned events.
- Legacy shows with inline `definition.profile_snapshot` data remain loadable. Migration canonicalizes byte-equivalent snapshots into show-level revision objects, preserves selected mode identity and patched behavior, and retains unknown data. Save As, export, selective import, and fixture transfer include every referenced snapshot and asset so portability never depends on the desk library.
- Contract tests patch one and many fixtures from a profile containing at least 2,000 modes and assert one stored snapshot per unique revision, no inline profile copies, one transaction, backup, compile, runtime swap, and event per batch, zero fixture-catalog reads, atomic failure, and request/response growth linear in fixture count. A focused frontend network test asserts that Add sends one batch request and performs no unrelated refresh requests.
- A warm release-build benchmark on documented reference hardware must keep a single-fixture patch below 250 ms server-side and visible in the Patch UI below 500 ms at p95; a 100-fixture batch must remain below 500 ms server-side and use the same single transaction and compile path. Record payload bytes and phase timings so regressions identify serialization, persistence, compilation, or projection refresh separately.

## Engine, Playback, Programmer, Control, and Output boundaries

- Refactor the engine into compiled show, contribution sources, merge and arbitration, transitions, fixture projection, DMX rendering, and visualization modules.
- Replace `Engine::playback()` and other mutable-lock exposure with typed commands, queries, and immutable runtime projections.
- Keep stateful animation outside the deterministic render core. The engine samples immutable values or batches supplied for the current render instant.
- Split Playback into persisted model, Cue tracking, runtime, controls, transitions, Dynamic instance integration, and contribution production.
- Split Programmer into state, selection, Groups, Presets, Preload, history, and registry or service modules.
- Split `light-control` stable action and mapping models from MIDI, OSC, RTP-MIDI, UDP, and timecode transports.
- Split stable output models from Art-Net/sACN codecs, sockets, scheduler, health, and delivery adapters.
- Make rendered output a two-stage result: resolved semantic fixture values become DMX frames and, later, typed external-device intents.
- External device adapters own connection, authentication, requests, feedback, retries, health, and shutdown. They never participate directly in Programmer or Cue storage.

### Output performance contract

Efficiency is a hard architecture requirement, not a post-refactor polish item. The render, arbitration, fixture projection, and output scheduler paths must be designed and benchmarked against fully packed universes with multiple simultaneous contribution sources, including Dynamics, overlapping Playback and Programmer values, and optional sound-to-light analysis.

- Hard acceptance floor: the server must generate complete output for at least 32 fully packed DMX universes at 100 Hz, including all contribution arbitration and output frame production for every universe on each tick.
- Target performance goal: 64 fully packed universes at 120 Hz on slower ordinary show-control hardware, not only on a high-end development laptop.
- Low-power goal: on very slow hardware such as a Raspberry Pi-class device, the output engine should still sustain 40 Hz across 4 to 8 universes.
- Output benchmarks must run against release builds, document the reference hardware, and report p50, p95, p99, dropped/deferred ticks, CPU usage, allocation rate, and time split between contribution sampling, arbitration, fixture projection, protocol encoding, socket delivery, and optional sound-to-light analysis.
- The render loop must avoid per-tick full-show cloning, broad mutex contention, JSON serialization, frontend projection work, fixture-library reads, persistence, or network adapter backpressure on the timing-critical output path.
- Dynamics, Macros, Timecode, sound-to-light detection, and external-device adapters may schedule or submit contributions, but they must not block output frame generation.
- If the system cannot keep the configured output rate, it must report actionable output health and overload diagnostics rather than silently producing stale or irregular frames.

## Macro architecture boundary

The refactor established language-neutral `MacroRuntime`, `MacroHost`, and `MacroService`
application seams above the domain services and outside the render loop. The completed proof does
not define the Macro product.

The sole current specification for language, packages, permissions, execution, persistence,
interaction, panes, Programmer access, and scheduling integration is
[Macros](Later/46-macros-and-scheduled-macros.md). This historical refactor plan must not be used
as an alternate Macro definition.

## Timecode and managed assets

- Introduce a monotonic runtime clock and scheduler boundary distinct from wall-clock metadata and external timecode input.
- Keep Cue fades, Chasers, Move in Black, Dynamics, Macro timers, and Timecode scheduling on the same deterministic timing foundation.
- Add a `ManagedAssetStore` before Timecode implementation for stable asset identity, import and validation, streaming, copying with a show, export, missing-state reporting, revision retention, and cleanup.
- A future Timecode runtime calls the same typed Playback and Macro services as manual operation.
- Timecode audio, timeline editing, seek behavior, missed events, and restart reconstruction remain future product decisions rather than refactor requirements.

## Frontend and desktop structure

- `transport/light`: validated HTTP/WebSocket DTOs and typed events.
- `session`: authentication, reconnect, and explicit primary/secondary-screen ownership.
- `features/{programmer,playback,show,patch,fixture-library,stage,screens,files,...}`: model, ports, store/hooks, and UI.
- `workspace`: panes, windows, modal presentation, and local layout only.
- `platform/desktop`: typed `DesktopBridge` with Tauri and browser-test adapters.
- `shared/ui`: proven presentation primitives only.
- `control-surface-contracts`: keypad IDs, layout, typed intents, and shared OSC action mapping.
- Retain `useServer()` temporarily as a migration facade and delete it after callers use narrow feature hooks.
- Replace DOM-based SET, Store, and Update routing with typed interaction workflows.
- Model primary and secondary-screen session roles so only the primary owns session creation and destruction.
- Split hardware-controls into an OSC bridge, feedback reducer, controller hook, and separate playback, programmer, grid, and settings surfaces.
- UI actions should feel immediate. A user-visible action either updates the visible state promptly, shows an explicit pending state, or opens an actionable loading/progress modal for work that legitimately takes time such as loading, importing, validating, compiling, or migrating a large show.
- Background work must publish success, progress, cancellation or retry options where applicable, and actionable errors. The UI must not leave the operator guessing whether an action was accepted, still running, failed, or completed.

## Command-line HTTP API

Add supported v2 endpoints scoped to the authenticated session's desk:

- `GET /api/v2/desks/{desk_id}/command-line` returns text, target, pristine state, revision, and pending choice.
- `PUT /api/v2/desks/{desk_id}/command-line` replaces the visible shared command line using `If-Match`.
- `POST /api/v2/desks/{desk_id}/command-line/keys` accepts a logical key, press or release phase, and request ID.
- `POST /api/v2/desks/{desk_id}/command-line/execute` executes the current line or an optional supplied full line atomically and returns the typed outcome and resulting command state.

The same `ProgrammingService` processes UI keys, complete HTTP command strings, individual HTTP keys, OSC keys, hardware keys, and future Macro actions.

## Staged migration

Each stage must leave the application buildable, testable, and usable.

### 1. Establish public test boundaries

- Inventory every Playwright and integration test by action and observation surface.
- Add the command-line HTTP adapter over the existing command implementation.
- Migrate Playwright actions to visible UI, exact OSC, command-line HTTP, or explicit deterministic bench controls.
- Move other cross-module integration tests to process-level public boundaries where practical.
- Retain focused same-module unit tests for parsing, arbitration, migrations, scheduling, and codecs.
- Capture current OSC, persistence, command, Playback, output, multi-screen, and UI behavior before structural movement.

### 2. Introduce composition and typed contracts

- Create `light-application` and `light-wire`.
- Move process startup, shutdown, routers, schedulers, and adapters into the server library.
- Introduce typed commands, outcomes, errors, and events behind temporary compatibility adapters.
- Introduce the typed application event bus, filtered subscription contract, sequence-gap recovery, coalescing, and bounded backpressure behavior. Publish automatic Playback transitions such as Chaser and FOLLOW advances as the first runtime slice.
- Add automated Rust and TypeScript dependency-direction checks to CI.

### 3. Migrate the first cross-surface slice

- Migrate command-line editing and execution, selection, Programmer values, Groups, and Presets through `ProgrammingService`.
- Migrate Playback addressing and actions through `PlaybackService`, including current-page and explicit-page resolution.
- Route UI, keyboard, OSC, attached hardware, HTTP, and compatibility WebSocket through the same services.
- Preserve exact command grammar, keypad layout, partial desk command state, request ordering, and source attribution.

### 4. Separate frontend state ownership

- Extract connection, session, and event routing from `ServerContext`.
- Introduce narrow Programmer, Playback, Show, Patch, Screens, Files, Configuration, and Output stores and hooks.
- Separate workspace presentation state from authoritative desk and show state.
- Add explicit primary and secondary-screen ownership.
- Introduce `DesktopBridge` and modularize the Tauri hosts.

### 5. Establish Show, fixture, and persistence boundaries

- Implement `ActiveShowService`, `ShowPatchService`, typed codecs, and `ShowCompiler`.
- Introduce the atomic `PatchFixtures` command, show-level deduplicated profile snapshots, targeted patch projections, and migration from legacy inline snapshots before moving Patch UI callers off generic show-object mutation.
- Migrate Groups and Presets, Cuelists and Playbacks, fixtures and patch, routes, layouts, MVR, Record, Update, undo, migrations, and startup recovery.
- Add `SelectiveShowImportService`.
- Remove generic show-object mutation from frontend features.

### 6. Refactor domain and engine internals

- Refactor Engine, Programmer, Playback, Fixture, Show, Control, and Output behind stable facades.
- Remove mutable lock exposure and direct transport dependencies from the render domain.
- Introduce immutable contribution batches, compiled fixtures, monotonic scheduling, and rendered-output batches.
- Publish every externally observable engine and runtime state transition through the typed application event boundary without adding transport work to the timing-critical render loop.
- Keep the contribution boundary suitable for stateful animated-value sources. When Dynamics is implemented, remove the accidental legacy Cue Phaser fields, evaluator, writer route, UI helpers, and tests as specified by the canonical Dynamics plans; old Phaser fields are ignored and dropped on the next save rather than evaluated or migrated.
- Add release-build output benchmarks and profiling hooks for the hard 32-universe 100 Hz floor, the 64-universe 120 Hz target, and the 4-to-8-universe 40 Hz low-power goal.

### 7. Prove future extension seams

These are architectural tests using fakes, not production feature implementations.

- Add a fake stateful animated attribute source that can be applied through Programmer, Preload, and Playback/Cue projections without changing transport or output code.
- Use a two-attribute fake value to prove combined Intensity and Tilt animation fits ordinary attribute resolution.
- Add a fake fixed contribution to prove a future stomp or `FAT` value can control the overlapping animated attribute without special-casing Groups, Cues, fixtures, or DMX output.
- Add a fake bidirectional external-fixture adapter without changing DMX delivery.
- Add a fake language-neutral Macro runtime that queries fixtures, performs a revisioned position change, waits for typed input, triggers a Playback, and makes a mocked HTTP request.
- Add fake daily and one-time Schedule invocations of a fake Macro with skip and catch-up policies.
- Add a fake managed asset and timeline operation to prove the Timecode seams.

### 8. Remove compatibility facades and document the result

- Remove REST/WebSocket v1 and `useServer()` compatibility layers after all callers migrate.
- Move giant inline server tests into feature-local unit tests and public-boundary integration tests.
- Add an architecture overview, state-ownership matrix, code tour, extension recipes, and test map under `docs/engineering`.
- Repair stale feature-plan links and document selective cross-show import.

## Verification and acceptance

- Playwright scenarios use UI, OSC, command-line HTTP, or explicit bench controls rather than implementation objects.
- Exact OSC paths, aliases, feedback indices, desk sharing, and current-page versus explicit-page semantics remain unchanged.
- Desk geometry, labels, gestures, focus behavior, software layout, and hardware-connected layout remain unchanged.
- Existing show, desk, fixture-profile, patched-fixture, and layout data remains loadable and migratable. Unsupported legacy Cue Phaser fields load safely, are ignored, and disappear on the next save.
- Single and batch patching meet the Patch mutation performance contract, remain atomic, and never reload unchanged fixture-library or unrelated desk state.
- Output generation meets the hard 32 fully packed universes at 100 Hz acceptance floor with multiple simultaneous contribution sources and optional sound-to-light analysis enabled or explicitly accounted for in benchmark results.
- Benchmark evidence records progress toward the 64 fully packed universes at 120 Hz target and the Raspberry Pi-class 4-to-8-universe 40 Hz low-power goal.
- Timing-critical output work is isolated from persistence, fixture-library access, frontend projection refresh, JSON transport serialization, and blocking external-device or sound-to-light adapters.
- Operator-facing UI actions are immediate or show explicit pending, loading, progress, success, failure, cancellation, and retry states appropriate to the operation.
- Invalid active shows still enter actionable recovery without destroying the original.
- Preserve unpatched fixtures, stored-empty Groups, missing Group IDs, ordered selections, Programmer LTP, Playback arbitration, Preload, Highlight, Update, Move in Black, route termination, safe shutdown, and first post-restart output.
- Prove Group Master overlap with a six-fixture source Group split into the stored
  `[DIV] 2` and `[DIV] 2 [+] 1` selections: programming the even selection to 100% and
  raising its assigned Group Master produces output. Membership in the original source
  Group does not limit it when that Group has no assigned Group Master; if that source
  Group is also assigned, its master combines by HTP rather than serial multiplication,
  LTP, or lowest-takes-precedence. Only the Grand Master remains above the resolved
  Group Master level.
- Add contract coverage for command concurrency, primary/secondary session closure, reconnect gaps, atomic revision failure, unknown stored objects, fixture-profile upgrades, selective imports, and adapter lifecycle.
- Event contract coverage proves that manual and automatic transitions publish the same semantic event once, a running Chaser updates subscribed Cue views without polling, narrow subscriptions receive only requested topics, reconnect gaps repair from an authoritative snapshot, and high-rate replaceable updates are coalesced without losing safety, error, or discrete transition events.
- A fake animated value source, fake external-device adapter, and fake Macro runtime must plug in without modifying existing transport adapters, render arbitration, or output drivers.
- At every stage run formatting, Clippy, Rust tests, TypeScript checks, frontend tests and build, focused UI/API/OSC coverage, and the CI desktop launch probes.
- Socket-based CITP and output tests run in normal CI or an unrestricted local environment.
- Methods and modules are split by responsibility, abstraction level, ownership, and test boundary rather than an arbitrary line limit.

## Assumptions and deferred decisions

- Dynamics, Timecode, Macros, Schedules, a `FAT` command, and bidirectional external fixtures are not implemented during this refactor.
- The architecture supports future static and stateful animated attribute values, but does not choose the Dynamics schema or pause behavior.
- Macro product decisions made after this refactor are owned exclusively by
  [Macros](Later/46-macros-and-scheduled-macros.md).
- Device-observed versus desk-desired value authority remains a future fixture-feature decision.
- Internal Rust and TypeScript APIs and REST/WebSocket v1 may break. Visible UI behavior, OSC, and persisted user data remain compatibility surfaces.
- Existing unrelated working-tree changes must remain untouched and be excluded from refactor commits.

## Consolidated execution record

This section is the durable summary of the major-refactoring execution. The former
`docs/plans/refactoring/` queue contained 130 incremental plans and result documents. Those
documents were useful while the work was active, but duplicated the final architecture and test
documentation after completion. They were removed on 2026-07-25; their exact contents remain
available in git history.

The refactoring was executed incrementally while preserving the operator-facing desk contract,
OSC compatibility, persisted show compatibility, unpatched fixtures, stored-empty and ordered
Groups, Programmer LTP behavior, current-page versus explicit-page Playback addressing, and
malformed-show recovery.

### Delivered outcome

- Introduced application and wire boundaries with typed action context, outcomes, errors,
  generated TypeScript contracts, checked-in schemas, and dependency-direction checks.
- Migrated runtime lifecycle, show-library and show-object operations, Playback topology,
  Programmer operations, patching, fixture-library access, configuration, output, files, Help,
  screens, and desk management to v2 capability-oriented interfaces.
- Added lossless show handling, unknown-object preservation, selective cross-show import,
  revision conflicts, atomic mutation behavior, targeted compilation, and interval-gated recovery
  checkpoints.
- Preserved Cue editing, timing, tracking, triggers, Move in Black, Preload, Highlight, Update,
  virtual Playbacks, Stage layout, patch placement, deterministic multi-point spreading, and
  independent overlapping Group Master HTP behavior.
- Replaced the former broad frontend provider with scoped feature state and explicit connection,
  loading, error, retry, optimistic-update, and sequence-repair behavior for the migrated
  capabilities.
- Added future-extension seams and fake-based architecture coverage for animated attribute
  sources, fixed/stomp contributions, external fixtures, Macros, schedules, managed assets, and
  timeline operations. These tests prove extension boundaries; they do not ship those future
  products.
- Organized the semantic Playwright bench under `tests/bench/` by command and selection,
  encoders, Groups and Presets, hardware, output, Playbacks, Programmer, show and show setup,
  specific features, and window system.
- Migrated 309 root acceptance cases with zero pending inventory rows and generated semantic
  documentation for 110 marked scenarios.

### Verification snapshot

The final execution run recorded the following evidence:

| Gate | Result |
| --- | --- |
| Rust workspace | Full workspace passed; the server suite included 469 passed and 1 ignored after rerunning outside the socket-restricted sandbox. |
| Frontend unit tests | 283 files and 2,007 tests passed. |
| TypeScript and production build | Frontend type-check and Vite production build passed. |
| Architecture checks | Dependency direction, Cargo lint inheritance, generated contracts, command boundaries, source-size ratchets, closed bench bindings, and semantic-world boundaries passed. |
| Source-size hard ratchet | No production file remained above 1,200 lines and no function remained above 150 lines. |
| Semantic documentation | Compiler tests passed 8/8 and generated documentation covered 110 scenarios. |
| Focused final migration | 25/25 cases passed. |
| API end-to-end | 86 passed and 1 skipped. |
| UI end-to-end | 131 passed with 3 timing-sensitive failures; the affected group rerun passed 14/14. |
| Supplemental end-to-end | 110 passed and 6 intentionally skipped. |
| Product demo | The complete narrated Full HD scenario passed and produced its H.265 artifact. |
| Visual catalog | 197 passed and 7 skipped; capture stopped after 5 capture-window failures, leaving 129 cases unrun. |

The visual-catalog result is capture-harness debt rather than a hidden product regression: the
affected normal API, UI, and supplemental paths passed. It nevertheless remains incomplete
verification and must not be described as a complete 338-case recording run.

### Known incomplete or separately tracked work

The execution closed the incremental queue, but a later audit identified requirements from this
plan that remain incomplete or lack retained acceptance evidence:

- Some server orchestration still depends on a large shared state container and raw lock-bearing
  compatibility access.
- Not every active-show writer is routed exclusively through `ActiveShowService`; some
  capability adapters still reproduce parts of validation, backup, persistence, runtime install,
  and event publication.
- String-plus-JSON compatibility commands and facade notifications remain on parts of the
  WebSocket/event path, with some frontend event handling still causing broad follow-up reads.
- Highlight HTTP and OSC paths do not yet share one bounded application service.
- Some interaction routing still discovers SET/keypad behavior through the DOM, and the planned
  shared TypeScript control-surface-contract boundary is not yet complete.
- Functional Patch tests prove batching, atomicity, the 2,000-mode contract, and absence of
  unrelated refreshes, but no retained release benchmark proves the required single-fixture and
  100-fixture server/UI p95 budgets with payload and phase timings.
- Release CI now measures the published Linux benchmark artifact without blocking the release:
  it tests 1,024 fixtures across 32 full universes at 100 Hz, conditionally doubles density to
  2,048 fixtures after a pass, attaches the report to the release, and publishes
  healthy/degraded/unknown status on Pages. A retained run will first exist after the next
  release, and fixed reference-hardware evidence, CPU usage, allocation rate, production socket
  delivery, and some phase splits remain outstanding.
- The complete serial visual catalog and final real packaged-desktop/operator-hardware verification
  remain outstanding.
- The non-blocking 400-line file and 20-line function design goals continue to identify possible
  cohesive extractions; the enforceable hard source-size ratchet is clean.
- A repository-wide dead-code and obsolete-compatibility audit is queued after the remaining Stage
  and Virtual Playback feature migrations. It covers the Rust backend first, followed by the UI,
  tests, tooling, dependencies, generated contracts, and documentation; see
  [`refactoring/pending/19-repository-wide-dead-code-removal.md`](refactoring/pending/19-repository-wide-dead-code-removal.md).

Dynamics, full Timecode, Macros, Schedules, `FAT`, and bidirectional external fixtures remain
deliberately separate product work. Their absence is not an unfinished refactoring implementation.
The later Macro product definition, including language and runtime selection, is owned exclusively
by [`Later/46-macros-and-scheduled-macros.md`](Later/46-macros-and-scheduled-macros.md).

### Durable handoff documentation

Living architecture and development guidance is maintained in:

- [`../engineering/architecture-overview.md`](../engineering/architecture-overview.md)
- [`../engineering/architecture-boundaries.md`](../engineering/architecture-boundaries.md)
- [`../engineering/state-ownership.md`](../engineering/state-ownership.md)
- [`../engineering/api-rules.md`](../engineering/api-rules.md)
- [`../engineering/code-tour.md`](../engineering/code-tour.md)
- [`../engineering/extension-recipes.md`](../engineering/extension-recipes.md)
- [`../engineering/test-map.md`](../engineering/test-map.md)
- [`../testing/README.md`](../testing/README.md)

The deleted per-chunk documents are historical evidence, not living documentation. Use this
consolidated record for status and the engineering documents above for current implementation
rules.
