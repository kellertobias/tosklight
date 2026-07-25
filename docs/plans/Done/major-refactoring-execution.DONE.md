# ToskLight refactoring — execution record (DONE)

> **Finalized 2026-07-25.** P1–P4, D1–D4, the complete numbered chunk queue, the semantic
> Playwright bench, and the final documentation/tooling cleanup are complete. The queue contains
> 130 archived result documents and has zero files in `pending/` or `doing/`.

This is the final execution summary for the major ToskLight refactoring. Architectural intent
lives in [`../major-refactoring.md`](../major-refactoring.md); the implemented boundaries and
engineering handoff live in:

- [`../../engineering/architecture-overview.md`](../../engineering/architecture-overview.md)
- [`../../engineering/architecture-boundaries.md`](../../engineering/architecture-boundaries.md)
- [`../../engineering/api-rules.md`](../../engineering/api-rules.md)
- [`../../engineering/code-tour.md`](../../engineering/code-tour.md)
- [`../../engineering/extension-recipes.md`](../../engineering/extension-recipes.md)
- [`../../engineering/test-map.md`](../../engineering/test-map.md)
- [`../../testing/README.md`](../../testing/README.md)

Older trackers (`refactoring-progress.md`, `refactoring-verification-log.md`,
`e2e-failure-diagnosis.md`, the root `REFACTORING-HANDOFF.md`, and
`PLAYBACK-RUNTIME-DESK-SCOPE-REFACTOR.md`) are superseded and folded into this record; their detail
remains in git history.

## Final outcome

The refactoring changed ToskLight from a prototype-shaped application with broad client/server
facades, duplicated mutation paths, mixed ownership, large UI modules, v1 compatibility routes,
and selector-heavy browser tests into an application with explicit domain/application ownership,
intent-shaped v2 APIs, typed live-control messages, scoped frontend boundaries, deterministic
persistence, enforceable architecture rules, and an operator-semantic acceptance bench.

The work was incremental: every numbered chunk retained a passing, reviewable state and recorded
its own result under [`../refactoring/done/`](../refactoring/done/). Persisted show compatibility,
the command-line and OSC contracts, desk-local interaction state, shared programmer semantics,
unpatched fixtures, ordered and intentionally empty Groups, current-page versus explicit-page
Playback addressing, and safe malformed-show recovery remained compatibility boundaries throughout.

### Architecture and ownership

- Server handlers and transport adapters now delegate to application-owned services instead of
  duplicating show, Programmer, Playback, patch, and persistence policy.
- `ActionContext` carries desk, user, session, surface, correlation, request, and expected-revision
  identity through mutations. Show revision conflicts are explicit and recoverable.
- Group, Preset, Cue, Playback, Preload, Highlight, patch, output, file, help, configuration, and
  fixture-library behavior use typed capability boundaries rather than a broad server facade.
- The Control UI no longer depends on the old `ServerProvider`/v1 client facade. Scoped stores and
  feature boundaries own server state, loading, error, retry, and mutation feedback.
- Workspace dependency direction is machine-checked. Every Cargo member inherits the root Rust and
  Clippy policy, and architecture checks reject future crates that omit it.

### API, events, and live control

- The remaining v1 REST and event WebSocket surfaces were inventoried, migrated, and retired.
- Runtime lifecycle, sessions/bootstrap, show library, show objects, Playback topology, screen and
  control-desk configuration, fixture library, media/output, files/help, and desk management moved
  to intent-shaped v2 routes.
- Command edits and semantic events share the multiplexed v2 WebSocket. Playback, Programmer,
  topology, Preset, command-line, programming-interaction, Speed Group, output, and runtime actions
  use typed live-control messages instead of direct client-local reducers or generic mutation
  endpoints.
- Unknown wire fields are accepted and logged at tolerant compatibility boundaries. Optional show
  guards and explicit context headers prevent accidental cross-show mutations without inventing a
  second ownership model.
- OSC remains a frozen public control contract and converges on the same authoritative desk state as
  the UI and WebSocket paths.

### Show data, persistence, and engine behavior

- Show persistence is write-behind and interval-driven, with deferred commits, bounded dirty
  subgraphs, atomic revision behavior, and explicit failure/repair handling.
- Cue editing, renumbering, deletion, tracking, merge/coexistence, timing, trigger selection,
  navigation, and Cuelist settings preserve runtime identity and persisted behavior.
- Parameter, Position, Color, and multi-point range spreading are server-owned. The shared
  deterministic anchor rule is used across command line, software encoders, attached hardware, and
  Color gestures.
- Patch address assignment, Stage layout/drag persistence, exclusion zones, independent overlapping
  Group Masters, Move in Black, Preload capture modes, virtual Playbacks, and Playback configuration
  have one authoritative implementation path.
- Legacy programmer `frozen_group` selection restores tolerantly to dereferenced fixture sources.
  Virtual-dimmer metadata is derived from the raw fixture profile snapshot, while the one-way
  intensity × colour output rule remains pinned by engine coverage.
- Show mutation compilation is limited to affected subgraphs rather than recompiling unrelated
  state.

### Control UI and operator behavior

- Oversized setup and window workflows were split into cohesive feature modules with explicit
  loading and actionable error state.
- Pane/window infrastructure, Console screens, patch boundaries, special Programmer controls,
  command history, software and attached encoders, hardware simulator controls, Playback selection,
  Highlight, Update, Matter, Sound-to-Light, files, Help, Desk Lock, and recovery workflows have
  focused ownership and regression coverage.
- Exact `DELETE` can remove a configured pane through an accessible title action while respecting
  unsaved-state guards and clearing the shared command line.
- Hardware encoder labels wrap rather than silently truncating operator-facing attribute names.
- The generated product-demo show was migrated to the current schema and the complete Full HD demo
  workflow passes.

### Semantic Playwright bench

- `apps/control-ui/e2e/bench/` is organized by operator area: command/selection, encoders,
  Groups/Presets, hardware, output, Playbacks, Programmer, show, show setup, specific features, and
  window system.
- Scenario authors use typed semantic intents for shows, Desktops, panes, selection, encoders,
  Groups, Presets, Cues, Playbacks, pages, Preload, time, DMX, OSC, screenshots, and recovery. They
  do not need CSS selectors, raw route strings, fixture UUIDs, pointer coordinates, physical encoder
  slots, or mutable show internals.
- Visible UI, touch, API, OSC, wire, restart, and harness routes are explicit. Unqualified routes
  use reproducible seeds and report their chosen adapter.
- The migration inventory covers **309 root cases with zero pending rows**. The generated semantic
  catalog covers **110 marked scenarios** and has self-contained JSON and HTML outputs.
- The final 22 pending operator-control rows were migrated into area-owned helpers; the orphaned
  software-encoder wrapper was removed, and only narrow protocol/exhaustive boundaries remain
  supplemental.

## Final verification evidence

The figures below record the final refactoring run rather than claiming that every optional future
feature exists.

| Gate | Final evidence |
| --- | --- |
| Rust workspace | Full workspace test run passed. A first sandboxed run hit only the expected media socket bind restriction; the complete rerun with loopback permission passed, including the server suite (469 passed, 1 ignored). |
| Control UI unit tests | 283 files, **2,007 tests passed**. |
| TypeScript/build | Control UI `tsc --noEmit` and Vite production build passed repeatedly. The root unit wrapper once stopped because a concurrent tooling change temporarily removed root `tsc`; the equivalent app-local typecheck passed. |
| Architecture/tooling | Cargo lint inheritance, semantic-doc compiler, dependency directions, source-size ratchet, command boundaries, closed bench bindings, and semantic-world boundaries all passed through `npm run test:architecture`. |
| Source-size ratchet | **0** legacy files above 1,200 lines and **0** legacy functions above 150 lines. The non-blocking design goals still report 167 production files above 400 lines and 5,960 functions above 20 lines. |
| Semantic documentation | Compiler tests **8/8 passed**; generated documentation covers **110** scenarios. |
| Migration inventory | **309** root cases covered; **0 pending**. |
| Focused final migration | **25/25 passed** across system integrations, hardware simulator/operator controls, and special-dialog/hardware Playback selection. |
| API E2E | **86 passed, 1 skipped**. |
| UI E2E | Broad four-worker run: **131 passed, 3 timing-sensitive failures**. The three failing areas were rerun together with four workers and passed **14/14**, including the compatibility fix found by the broad run. |
| Supplemental E2E | **110 passed, 6 intentionally skipped**. |
| Product demo | Full narrated Full HD scenario **1/1 passed**; the H.265 artifact was encoded successfully. |
| Visual catalog | The serial capture reached **197 passed, 7 skipped** and assembled 64 videos before it was stopped after five capture-only five-second interaction failures; 129 cases did not run. GROUP-005 and both TIME-002 cases were already documented suite flakes; UPDATE-002 and retained PLAYBACK-SELECT supplemental coverage also missed capture windows. Every affected normal API/UI/supplemental path passed. |

The visual-catalog result is the only incomplete final gate. It is recorded as test-infrastructure
timing debt rather than hidden or misreported as a product regression. The normal parallel suites,
focused reruns, semantic inventory, and product demo provide the binding acceptance evidence for
the refactoring.

## Final tooling and handoff

- `docs/testing/README.md` is the concise test-author guide and the semantic catalog is generated
  deterministically.
- The source-size scanner now examines production JavaScript/TypeScript, Rust, and Python beneath
  `apps/`, `crates/`, and `packages/`, excluding documentation, assets, artifacts, and experiments.
- The repository includes a Rust-for-TypeScript engineering tour, updated architecture/code-tour
  documentation, extension recipes, API rules, build/test commands, and a test map.
- Release automation was moved to clearly named GitHub Actions jobs and the product remains marked
  as pre-release software.
- Completed feature plans were archived, superseded plans were consolidated, and future product
  work (Dynamics, Media Server, visualizer, relative encoders, attribute activation, Highlight
  look, fixture-sheet improvements, Playback auto-Off, and related roadmap items) remains separate
  from the completed refactoring.

## Remaining work outside this refactoring

- Improve or redesign serial visual recording so normal five-second interaction polls remain stable
  under high-bitrate video capture, then run the complete 338-case catalog without interruption.
- Continue reducing the non-blocking 400-line/20-line source-size design goals where a cohesive
  boundary exists; the hard ratchet is already clean.
- Implement only the separately approved product plans under `docs/plans/Next/` and
  `docs/plans/Later/`. Their presence is not unfinished refactoring.
- Continue real packaged-desktop and operator-hardware verification where a browser harness cannot
  truthfully prove native window/process ownership.

## Historical working rules

- Read `AGENTS.md` first — persistence/compatibility rules are load-bearing. `docs/help/**` is the
  source of truth for operator behavior; `docs/testing/**` + root `tests/` are the acceptance
  contract; read `docs/acceptance-criteria.md` before any persisted-data change.
- Small chunks: smallest relevant check first (`npm run test:unit`, `cargo test -p <crate>`,
  `tools/test.sh e2e --grep <ids>`), then the full `tools/test.sh e2e` at the end of each chunk.
  Land only with **no net new regressions**.
- Prefer fixing **engine/backend**; a UI-only gap may be skipped-with-reason after one genuine fix
  attempt — but first confirm it is genuinely UI (its `@api`/engine surface passes) and not a small
  bug (see "how OSC-001 fooled us" below).
- Rust: `cargo fmt`. Do **not** push. Commit each chunk with a clear message.
- Do **not** touch pre-existing user-dirty files: `tests/product-demo.spec.ts`,
  `tests/support/plannedDemoState.ts`, `apps/control-ui/src/windows/builtInStageModels.ts`,
  `apps/control-ui/src/windows/stage3dScene.test.ts`, the `assets/**`, `package*.json`, `.gitignore`.

## First completion milestone (2026-07-22 — P1–P4 executed)

- Baseline (pre-refactor) 174 passed / 88 failed → **266 passed / 20 skipped** now. The only
  failing test is the user-dirty `product-demo` whole-app run.
- **P1 (DEGRP) is done**: `SelectionExpression::FrozenGroup` is removed; keypad `GROUP GROUP`,
  WS `group.select frozen`, HTTP frozen select-group, and the frozen-group refresh all dereference
  to `Sources` of individual fixtures, identical to the double-click gesture. Persisted programmers
  carrying the legacy `frozen_group` expression (including undo/redo snapshots) migrate on restore.
  GROUP-004 asserts the dereferenced model on every surface.
- **P2 is done**: two root causes fixed — the HTTP selection-gesture environment now also covers
  the groups referenced by the open gesture (they previously resolved against an empty map and
  silently dropped), and the UI profile-mode definition conversion restores the abstract
  virtual-dimmer intensity parameter like the server projection (RGB LED selections assign
  Enc 1 · Dimmer again). PROG-001 and DIM-001 are green on all surfaces.
- **D4 is done**: the command line no longer carries a trailing space (engine + software keypad);
  digits typed after a bare `+`/`-` insert their own separator. OSC-005 @osc is unskipped.
- **P3 is done**: CMD-002 @ui unskipped (the v1 tap/double/half/pause route now publishes v2
  speed-group change events via `SpeedGroupService::record_external_change`); the playback-topology
  writer no longer drops queued saves on a same-show authority re-hydration and retries once after
  a 409 whose repaired object expectations still hold — this fixed CUE-012 @ui and SHOW-001 @ui
  (both unskipped) and PLAYBACK-SELECT-001 @ui. Remaining skipped-with-reason residues, each
  re-verified after the fixes: CUE-011 @ui/@supplemental-ui (window-header revision lag after a
  retried save + one silent extra cue_list revision), PRELOAD-004 @ui/@supplemental-ui (preload
  lifecycle view stale after PRELOAD GO), PBK-005 @supplemental-ui (held-Swap request never
  issued), SHOW-005 @ui (revision-copy identity label), PLAYBACK-SELECT-001 @supplemental-ui
  (hardware fader-bank stability), plus the D1 feature gaps (MANUAL-019, SOUND-001,
  COLOR-RANGE-001).
- **D3 (derive-only) is implemented**: the schema-v2 snapshot identity check compares the raw
  profile identity (definition-level id/revision are derived shape), SHOW-004
  virtual-dimmer-metadata is unskipped and green (legacy fixtures self-heal by re-derivation,
  byte-stable — the serde defaults already produce the abstract virtual-dimmer metadata), and an
  engine guardrail test pins the intensity×colour one-way multiply
  (`virtual_dimmer_intensity_multiplies_reacting_channels_one_way`). DMX-006 stays
  skipped-with-reason: its installer now patches cleanly, but the scenario still mutates the
  *derived* heads (16-bit component layouts, byte order, parameter defaults), which the schema-v2
  engine deliberately ignores in favour of the raw profile snapshot — it needs re-authoring
  against profile channels, including deciding how the profile schema expresses LSB-first
  component order (it currently cannot). DMX-008 stays skipped on its independent, unimplemented
  minimum-universe-size padding/defaults output contract.
- **P4 is done**: typed Group management (GroupPropertiesDialog/GroupContextMenu) and the scoped
  Patch reads were already migrated; the last broad `useServer().readVisualization` readers
  (`useCueThumbnails`, the Position special dialog) now use the scoped
  `useVisualizationRuntimeRead`. All ratchets (`check-architecture`, `check-source-size`,
  `test-command-boundaries`) pass.

### Historical state (session start)

- Baseline (pre-refactor) 174 passed / 88 failed → session start 231 → 258 passed.
- The facade-removal / authority-scoping refactor introduced **no net new regressions**; the 5 it
  did introduce (Patch reader migration) were fixed. Every remaining failure passes on its
  `@api`/engine surface unless noted.
- The bench is **flaky**: a worker occasionally crashes → "N did not run" and spurious failures.
  Always re-run a suspected failure in isolation before treating it as real. `FIXTURE-002 @restart`
  is flaky (passes in isolation). `product-demo` is a user-dirty whole-app test — leave it.

### Recently fixed (this session)
TEXT-015 (split-view CSS collapse), TIME-001/002 (mode-aware test helpers), **OSC-001/006 page
change** (the writer's action methods were called unbound — bound them in the
`PlaybackRuntimeActionWriter` constructor, `b793e56`), SHOW-000 (stale `role`/`title` selectors).

**How OSC-001 fooled us:** the page change "silently failed" and looked like a load-bearing
scoped-store scope-guard problem. It was actually a one-line unbound-method call
(`const f = actions.setActivePage; await f()` → `this` lost). **Lesson for the remaining skips:**
several of them may likewise be small bugs, not unimplemented features. Instrument with a forwarded
browser console (`page.on("console", ...)` + `console.warn` in the suspect path) before assuming a
gap is architectural.

## Operator selection model (authoritative — align docs/help + tests to it)

- Two selection **modes**: Group and Fixture. The default selection type follows the current mode
  unless FIXTURE or GROUP is typed explicitly. Fixture = Shift+GRP. Toggle mode = GRP + ENTER.
- Operator **precedence**: THRU → +/- → DIV → DIVADD (a `+` after DIV).
- THRU is **same-type**: `GROUP 1 THRU 5` == `GROUP 1 THRU GROUP 5`.
- `+`/`-` may **mix types**; a bare number takes the current mode's type:
  `GRP 1 THRU 5 - 1` == group mode: Group1..5 minus GROUP 1; fixture mode: Group1..5 minus FIXTURE 1.
- Group references stay **live**: `Group1 - Group2` = Group1 without Group2's current members (live);
  `Group1 - Fixture2` = Group1 without fixture 2 (whether or not it is a member).
- **DEGRP** (double-tap GROUP, or double-click a group card) = the fixtures currently in that group,
  dereferenced to individual fixtures, with **no reference back**. A group recorded from a DEGRP'd
  selection stores the individual fixtures (no `frozen_from`).

## Historical final-session chunks (all completed)

The following sections preserve the diagnosis and execution instructions used during the first
completion milestone. They are historical context; none remains in the active queue.

### P1 — DEGRP / group-selection consistency (engine bug; must fix)
DEGRP is implemented two ways. Keypad `GROUP GROUP <id>`
(`crates/server/src/runtime/programmer_group_commands.rs:163` sets `frozen=true`) →
`SelectionExpression::FrozenGroup` (keeps a group reference). Double-click
(`SelectionGestureSource::DereferencedGroup`) → `Sources` of individual fixtures (no reference).
Per the model, the **keypad path is wrong**.
1. Make keypad DEGRP dereference to `Sources` (individual `Fixture` refs) — identical to the
   double-click gesture; the two must match.
2. Stop producing `SelectionExpression::FrozenGroup` (nothing should after step 1), then retire it.
   **Persistence check result:** `selection_expression` is NOT in the show file, but it IS
   serialized in the **durable programmer** snapshot for restart recovery — `ProgrammerState` and
   `ProgrammerSnapshot` derive `Serialize/Deserialize` over `selection_expression`
   (`crates/programmer/src/state.rs:26,49`) — and it is in the `crates/wire` interaction schema. So
   a hard enum-variant removal would break deserialization of a persisted programmer that holds a
   `frozen_group`, and change the wire contract. Retire it **tolerantly**: on deserialize, map a
   legacy `frozen_group` selection to the dereferenced `sources` form (custom `Deserialize` or a
   post-load normalize), keep the wire regenerated (`cargo run -p light-wire --example
   generate-contracts`), and test restart recovery of an old programmer snapshot. Only then delete
   the variant's producers/handling (`selection.rs`, `group_recording.rs`, `update_capture.rs`,
   `highlight/selection.rs`). Do **not** touch the unrelated `groups::FrozenGroup` struct
   (`frozen_from` on recorded groups — a legitimate referenced/derived-group concept).
3. Fix GROUP-004 @supplemental (`tests/support/foundational/supplementalGroups.ts`): expect
   `selection_expression.type == "sources"` after DEGRP and **no** `frozen_from` on the recorded
   group. Keep `docs/help/30-Programmer/01-command-line.md` consistent (it already says dereference
   stores individual fixtures).
Verify: `cargo test -p light-programmer -p light-application`; then
`tools/test.sh e2e --grep "GROUP-004"`.

### P2 — PROG-001 & DIM-001 (selection contents)
`PROG-001 @supplemental-ui` (`supplementalSurfaces.ts` — drag-marquee/Preset/mixed-source, expects
`selected == [1,2,3,4]`) and `DIM-001 @supplemental` (`supplementalDimmers.ts` — API
add/subtract/delete ordering). Trace each against the selection model above; decide per case whether
the **engine** (`crates/programmer` / `crates/application`) or the **test** is wrong, and fix
accordingly. Verify per test then full suite. See DECISION D2 (default mode).

### P3 — Re-investigate the documented skips toward green
Each was skipped-with-reason but `@api`-green. Re-examine before assuming "unimplemented" (OSC-001
lesson). Buckets:
- **Likely real bugs (fix):** SHOW-001 @ui fader-bank assignment (`assignPlayback` no-ops — check
  binding/hydration like OSC), CUE-011/012 @ui cuelist settings/renumber (topology-writer path),
  PRELOAD-002/004 @ui, PLAYBACK-SELECT-001 @supplemental-ui, CMD-002 @ui, SHOW-005 @ui, PBK-005
  @supplemental-ui.
- **UI-feature gaps (see DECISION D1 — build or leave skipped):** the MANUAL-019 reworked surfaces
  (Cues responsive editor, Outputs route editor, DMX monitor rework, Stage scenery model, Shows &
  recovery browser), SOUND-001 (browser analyzer → Sound source), COLOR-RANGE-001 (shift-drag range),
  Speed Group / PRELOAD on-screen controls.
- **Fixture-schema (see DECISION D3):** DMX-006/008, SHOW-004 virtual-dimmer-metadata / 16-bit output.
- **Contract contradiction (see DECISION D4):** OSC-005 @osc command-line trailing space.

### P4 — Remaining architecture work (from the design plan)
Finish the ownership migrations that are still on the broad facade:
1. **Typed Group management end-to-end** — `GroupPropertiesDialog`/`GroupContextMenu` (`updateGroup`,
   `undoGroup`, `refreshFrozenGroup`, `detachDerivedGroup`).
2. **Patch read ownership** — remove the broad Patch bootstrap; move Channels/DMX, Parameter/Color/
   Position/Control dialogs, Highlight, System Controls, Stage/Fixture-sheet projections, group-window
   fixture resolution, cue thumbnails, and `PatchFeatureBoundary`'s `initialFixtures` onto scoped reads.
3. **Visualization ownership** — `useCueThumbnails`, special dialogs' position, remaining broad reads.
Gate each with the size/architecture ratchets (`node tools/check-source-size.mjs`,
`check-architecture.mjs`, `test-command-boundaries.mjs`) and the full suite.

## DECISIONS

- **D1 — scope of "done": Fix bugs, leave genuine features skipped.** Re-investigate every skip and
  fix anything that is actually a bug (many are — see the OSC-001 lesson). Do **not** build new
  features; genuinely-unimplemented reworked-UI features stay skipped-with-reason. "Done" = all
  implemented behavior correct + documented skips.
- **D2 — default selection mode at desk start: Fixture.** A bare number selects a Fixture; Group
  requires the GROUP key; toggle with GRP+ENTER. Align PROG-001/DIM-001 expectations to this.
- **D3 — virtual-dimmer-metadata / 16-bit fixture output (DMX-006/008, SHOW-004): derive-only.**
  Model: a virtual-dimmer head has no physical dimmer channel; its colour channels are marked
  `reacts_to_virtual_intensity` (persisted in the profile snapshot — this is how we know it has a
  virtual dimmer and how it scales DMX). Outside the fixture the operator has an independent
  `intensity` and `color`; at DMX output the fixture multiplies intensity × colour onto the reacting
  channels (one-way — neither value derives from the other). **This value flow already exists in
  `crates/engine/src/profile_projection.rs` and MUST be preserved unchanged.**
  The schema fix is only about the persisted definition *shape*: the synthesized `intensity`
  parameter (`abstract_virtual_dimmer_intensity()`, `definition_projection.rs`) must be **re-derived
  from the channels at load/resolve, never treated as authoritative in the stored snapshot**. So:
  (1) a legacy show whose baked `heads[].parameters[].metadata/capabilities` are absent must
  self-heal by re-derivation with **no byte-mutating migration** (SHOW-004 idempotent/byte-stable);
  (2) the schema-v2 snapshot identity check (`crates/fixture/src/definition.rs:43`) must compare the
  **raw profile identity**, not the derived shape (unblocks DMX-006's `installSixteenBitMatrix`);
  (3) do not persist the synthesized parameter as authoritative. Then unskip DMX-006/008 + SHOW-004
  virtual-dimmer-metadata. Read `docs/acceptance-criteria.md` and test old-show load; keep the
  `.agents/skills/build-light-fixtures` contract in mind. Add/keep an engine test asserting the
  intensity×colour multiply and its one-way independence so the schema change can't regress it.
- **D4 — OSC-005 command-line trailing space: no trailing space (`"G7 +"`).** Command-line spaces are
  cosmetic only. Change the engine's command-line formatting to not append a trailing space (search
  `crates/programmer/src/command_line.rs` `format!("{command} {next_token} ")`), update the
  `softwareKeypad` unit test and `docs/help/30-Programmer/01-command-line.md` to match, and unskip
  OSC-005 @osc. Verify no other command-line assertion depends on the trailing space.
