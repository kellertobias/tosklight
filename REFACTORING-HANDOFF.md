# Major Refactoring Handoff

This report is a restart point for the unfinished work in
[`docs/plans/major-refactoring.md`](docs/plans/major-refactoring.md). The detailed milestone ledger
remains [`docs/plans/refactoring-progress.md`](docs/plans/refactoring-progress.md); architecture and
code navigation live under [`docs/engineering`](docs/engineering).

## Checkpoint

- Inspected: **2026-07-21**
- Branch: **`refactoring`**
- Checkpoint HEAD: **`47030ed`** (`docs(refactoring): record visualization and cue milestones`)
- Branch state at inspection: **72 commits ahead of `origin/refactoring`**, not pushed
- Estimated completion: **97%**
- Estimated remaining Codex time: **25–45 active hours**, plus reference-hardware measurement time

The branch is buildable and the latest completed vertical slices are committed. The refactor is
not complete: broad frontend authority, residual v1 adapters, automatic runtime events, portable
mutations, and final operator/performance acceptance remain.

### Preserve the existing worktree

These two tracked files were already modified outside the completed refactoring milestones. They
must not be discarded, overwritten, staged, or included in a refactoring commit unless the user
explicitly assigns them:

- `docs/engineering/test-map.md`
- `tests/support/foundational/groupState.ts`

Always run `git status --short --branch` before editing and stage exact paths. Do not rewrite the
existing commits and do not push without explicit permission.

## What is already complete

The committed architecture now includes:

- thin Rust process composition with feature-owned server modules;
- `light-application` typed use cases, `light-wire` DTOs/schemas, and checked-in generated
  TypeScript contracts;
- revisioned active-Show transactions, lossless object projections, replay/idempotency, and scoped
  event publication;
- scoped Show Objects, Playback runtime/topology, Group runtime, Programmer interaction, normal and
  Preload Programmer values, capture mode, Priority, Preset recall, Preload lifecycle, Output, and
  Speed Group frontend authorities;
- typed Cue recording, Cue navigation, Cue transfer, whole-Cue deletion, Update, Group recording,
  Preset recording, Playback actions/Page topology, Output, Speed Group, Patch mutation, and
  Selective Show Import application boundaries;
- strict optimistic reconciliation for response-before-event and event-before-response ordering,
  replay/no-change, rollback, cursor gaps, scope replacement, and late responses across the
  migrated stores;
- removal of the broad `/api/v1/playbacks` frontend snapshot;
- public test-intent helpers for the migrated action families;
- a shared view-scoped Visualization runtime for normal and Preload projections; and
- architecture, command-boundary, private-boundary, generated-contract, and source-size ratchets.

Recent checkpoint commits:

- `de223b8 refactor(control-ui): share scoped visualization runtime`
- `5943bd7 refactor(test): migrate cue deletion actions`
- `39424d3 test: keep cue ownership probe v2-only`
- `47030ed docs(refactoring): record visualization and cue milestones`
- `72aeb00 refactor(test): migrate speed group actions`
- `0d94e7f feat(programmer): add revisioned cue deletion action`
- `91cff4f feat(control-ui): add scoped speed group authority`
- `7595473 refactor(test): migrate playback go actions`
- `95c95ec refactor(test): migrate programming selection actions`

## Remaining work, in recommended order

### 1. Complete typed Group management end to end

This is the best next coherent slice. Four production mutations still cross the broad server
facade:

- `GroupPropertiesDialog.tsx` → `updateGroup`
- `GroupContextMenu.tsx` → `undoGroup`
- `GroupContextMenu.tsx` → `refreshFrozenGroup`
- `GroupContextMenu.tsx` → `detachDerivedGroup`

Add one typed application/wire/server/frontend action family for property updates, undo, frozen
refresh, and derived detachment. It must preserve lossless Group fields, source revisions,
stored-empty Groups, ordered fixtures, replay, exact revision conflicts, and one Show event per
real transition. Frozen refresh must keep its source metadata and update the originating desk's
selection without deadlocking; selection events precede the owning Show event.

The frontend should be action-only and dormant: a strict transport and request-ordered writer use
the existing Show Objects store for authoritative Group installation. Migrate the two UI owners,
then remove the four methods from `ServerProgrammingContext` and their legacy feature adapters.

### 2. Finish Patch read ownership and remove broad Patch bootstrap state

Patch mutations are scoped, but many readers still use `server.patch`. Migrate them to narrow Patch
selectors with explicit activation. High-value consumers include:

- Channels and DMX windows;
- Parameter Controls and special Color/Position/Control dialogs;
- Highlight and System Controls;
- Stage and Fixture Sheet projections;
- Group-window fixture resolution;
- Cue thumbnails; and
- `PatchFeatureBoundary`, which still seeds `initialFixtures` from the broad facade.

Do not expose another broad Patch context. Add scalar or identity-based selectors for the exact
fixtures/routes/status required by each mounted view. Loading must not fall back to bootstrap data,
inactive views must not subscribe, and one Patch delta must not rerender unrelated consumers.

After the last reader migrates, remove broad `/api/v1/patch` startup/refresh state and its
`ServerContext` fields. Preserve the existing typed batch mutation, profile snapshot, stored-show,
and unpatched-fixture contracts.

### 3. Finish Visualization ownership

The shared normal/Preload runtime now owns ordinary polling, but two one-shot paths remain:

- `windows/cuelistWindow/useCueThumbnails.ts`
- `components/modals/specialDialogs/position.tsx`

Move them behind an explicit scoped snapshot/query capability, then remove `readVisualization`
from `ServerPlaybackContext` and the broad media facade if there are no callers. The transitional
v1 Visualization payload contains no Show/session identifiers: requested authority is validated
before the request and late responses are generation-dropped, but payload scope cannot be
independently verified until the wire endpoint is replaced.

### 4. Split configuration, installation, session, and shell authority

A live audit found **58 production files** importing `useServer()`. Not all are equally broad, but
the remaining calls cluster around:

- configuration/timing/Matter settings;
- active Show, users, clients, connection, and hardware status;
- stage and user-layout persistence;
- fixture library, files, media, desk lock, recovery, and setup workflows; and
- command/history/error/status presentation.

Create small capability stores and action providers rather than one replacement global context.
Mount network work only for visible owners where practical, and use scalar selectors for shell
status. Remove each facade field only after its last production and test caller migrates.

### 5. Complete remaining typed actions and portable mutations

The plan still names these application-owned gaps:

- standalone Playback `SET` grammar;
- bare command-line `UPDATE` routing;
- Preset delete and Preset MOVE/COPY transfer;
- Group property/undo/refresh/detach operations;
- output-route and user-layout mutations;
- residual standalone Playback/Page operations;
- remaining typed undo paths; and
- miscellaneous setup/portable-show mutations still using generic object APIs.

Each real batch remains one application action, one revisioned transaction, one lossless retained
projection, and at most one semantic event. Do not replace one legacy network action with N typed
actions.

### 6. Publish the remaining automatic runtime events

Add typed, scoped event ownership for externally visible transitions still discovered by polling or
broad refresh, including:

- Highlight movement;
- transition completion;
- output health and overload changes; and
- any remaining automatic Playback/runtime changes.

Manual and automatic origins must produce the same semantic event exactly once. High-rate
replaceable telemetry may coalesce; safety, errors, outcomes, and discrete transitions may not be
dropped. Gap repair must return to an authoritative snapshot.

### 7. Retire compatibility surfaces

The acceptance ratchet currently permits **12 direct v1 WebSocket calls across three files**, all
intentional probes:

- five API-004/CROSS-002 edit, target, unknown-action, or external Group-value probes;
- six CUE-navigation compatibility calls; and
- one whole-Cue-deletion compatibility call.

There are zero literal categorized public compatibility-family calls, but two shared dispatch
helpers can still route Preset delete/transfer and bare `UPDATE`. Keep the dedicated compatibility
specs; do not conceal new raw calls behind generic helpers.

After every production caller has a typed owner, remove the remaining REST/WebSocket v1 adapters,
generic frontend show-object mutations, `useServer()`, and DOM/custom-event routing for SET, Store,
Update, Group configuration, encoder actions, and other feature commands. Public operator behavior,
OSC paths/feedback, desk sharing, and persisted data remain compatibility surfaces.

### 8. Finish performance and operator acceptance

The refactor cannot be declared complete without evidence for:

- warm release Patch performance: one fixture below 250 ms server-side and 500 ms visible at p95;
  a 100-fixture batch below 500 ms server-side;
- output performance: 32 full universes at 100 Hz minimum, evidence toward 64 at 120 Hz, and the
  Raspberry Pi-class 4–8 universe/40 Hz goal;
- command edit/execution latency, backlog, persistence cost, snapshot repair, bootstrap traffic,
  and rerender counts;
- old/malformed show recovery, Save As/export, layouts, unpatched fixtures, stored-empty Groups,
  ordered selections, Cue Phaser, Highlight, Preload, Update, Move in Black, route termination,
  shutdown, and first output after restart;
- unrestricted socket tests, including CITP/output cases;
- desktop smoke; and
- authoritative `./build open`, readiness, log, and real operator-path verification.

### 9. Refresh final documentation

Keep these documents synchronized as the last facades disappear:

- `docs/engineering/architecture-overview.md`
- `docs/engineering/architecture-boundaries.md`
- `docs/engineering/state-ownership.md`
- `docs/engineering/code-tour.md`
- `docs/engineering/extension-recipes.md`
- `docs/engineering/build-and-test-commands.md`
- `docs/engineering/refactoring-test-boundaries.md`
- `docs/engineering/test-map.md`

Repair stale plan links and make the final code tour follow one current frontend mutation and one
backend action through validation, application service, persistence, event publication, optimistic
reconciliation, and repair.

## Current verification evidence

At this checkpoint:

- full frontend Vitest: **1,927 passed in 268 files**;
- focused Visualization contract/consumer tests: **61 passed**;
- acceptance-intent Vitest: **73 passed**;
- migrated Cue API scenarios: **3 passed**;
- dedicated retained-v1 Cue specification: **1 passed**;
- Cue-deletion application tests: **3 passed**;
- Cue-deletion server contract tests: **4 passed**;
- generated wire-contract verification: **passed**;
- frontend typecheck and production Vite build: **passed**, with the existing large-chunk advisory;
- Rust formatting: **passed**;
- dependency-direction and command-boundary checks: **passed**;
- source-size hard limits: **0 files above 1,200 lines and 0 functions above 150 lines**;
- design-goal debt: **138 production files above 400 lines and 5,833 functions above 20 lines**;
- `git diff --check`: **passed**; and
- generated Rust/TypeScript code still prints the known non-fatal `ts-rs` warning for
  `deny_unknown_fields`.

These checks prove the checkpoint is coherent, not that the repository-wide refactor or real
desktop/performance acceptance is complete.

## Copy-paste prompt for the next agent

```text
Continue the major refactoring in /Users/keller/repos/light on the existing `refactoring` branch.

First read:
- AGENTS.md
- REFACTORING-HANDOFF.md
- docs/plans/major-refactoring.md
- docs/plans/refactoring-progress.md
- docs/acceptance-criteria.md

Verify the branch, HEAD, and worktree before editing. The handoff checkpoint is:
47030ed docs(refactoring): record visualization and cue milestones

The branch may be ahead of that checkpoint; never reset or rewrite newer commits. Preserve all
unrelated work. At the checkpoint these tracked files were already modified and must not be
discarded, staged, or edited unless explicitly assigned:
- docs/engineering/test-map.md
- tests/support/foundational/groupState.ts

Next coherent slice: implement typed Group management end to end.

Backend:
- Add one explicit application action family for Group `update_properties`, `undo`,
  `refresh_frozen`, and `detach_derived` operations.
- Carry request/correlation identity and exact Show, Group storage ID, Group revision, source Group
  identity/revision where applicable, desk, user, and session authority.
- Use the existing Active Show/application boundary. Preserve unknown fields, ordered fixture
  membership, stored-empty Groups, derived/frozen metadata, audit/persistence behavior, and exact
  request replay.
- Return changed/no-change state, replay status, authoritative lossless Group projection, Show and
  object revisions, event sequence only when emitted, and persistence warning.
- Emit exactly one Show event per semantic mutation and none for no-op/replay. A failed conflict or
  invalid source must mutate nothing.
- Frozen refresh must resolve the source under the same Show transaction, store the refreshed
  frozen snapshot with source revision/timestamp, and leave the originating desk selection as the
  frozen source. Publish any resulting selection event before the owning Show event without nested
  desk-lock deadlock.
- Keep existing v1/WebSocket/UI compatibility adapters routed through the typed application
  service until all callers migrate.

Frontend:
- Add feature-owned strict wire decoding, authenticated HTTP transport, action-only provider, and
  request-ordered writer. Do not add a broad snapshot or another global context.
- Scope authority by server, session, active Show, authenticated desk/user, exact Group storage ID,
  and revision. Reject undeclared fields, foreign scope, stale outcomes, and late responses after
  replacement.
- Reconcile optimistic state safely whether the Show event or HTTP response arrives first. Support
  rollback, replay/no-change, revision conflict repair, retry where safe, and scope replacement.
- Install authoritative Groups into the existing ShowObjectsStore in one notification.
- Migrate GroupContextMenu.tsx and GroupPropertiesDialog.tsx off `useServer()`.
- After the last caller migrates, remove `updateGroup`, `undoGroup`, `refreshFrozenGroup`, and
  `detachDerivedGroup` from ServerProgrammingContext and delete their superseded legacy frontend
  adapters.
- Mount composition through the feature/provider composition layer, not ServerContext. Keep it
  dormant until an action owner exists; no snapshot, socket, bootstrap reload, or unrelated React
  rerender is allowed.
- Do not mix in Patch-reader migration or remove `light:group-configuration` in this slice.

Verification must cover:
- property update, undo, frozen refresh, derived detach, no-op, replay, rollback, and revision/source
  conflict;
- stored-empty and ordered membership preservation;
- lossless unknown fields;
- exactly one authoritative Show event per real mutation;
- selection-before-Show ordering for frozen refresh;
- same Show across desks, foreign desk/session rejection, and replacement races;
- response-before-event and event-before-response reconciliation;
- no bootstrap reload, no dormant network work, and no unrelated rerender;
- legacy compatibility behavior through the typed service.

Run at minimum:
- cargo fmt --all -- --check
- focused light-application, light-wire, and light-server tests
- cargo test -p light-wire --test generated_contracts
- focused frontend tests
- frontend typecheck and production build
- node tools/check-architecture.mjs
- node tools/check-source-size.mjs
- node tools/test-command-boundaries.mjs
- git diff --check

Keep production files below 400 lines where practical and never above 1,200; keep functions below
20 lines where practical and never above 150. Prefer small feature-owned modules and explicit
types. Update docs/plans/refactoring-progress.md with completed work, exact verification, remaining
work, progress percentage, ETA, and known limitations. Commit each coherent milestone with a
semantic commit message. Do not push.

Stop after the Group-management slice is committed and green. Report commit hashes, exact test
results, remaining uncommitted files, limitations, and the recommended next slice (Patch readers).
```
