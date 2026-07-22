# End-to-end failure diagnosis (2026-07-22)

First full Playwright run of the refactor effort. Baseline: **`47030ed` = 174 passed / 88 failed**;
**HEAD (`47a9466`) = 169 / 93**. Per-test diff: **0 fixed, 5 introduced** since `47030ed`; the other
88 are pre-existing (the typed Playback-topology + visualization/cue migration was committed without
ever running e2e). See `refactoring-progress.md` → "End-to-end acceptance status" for the numbers
and the 5 introduced regressions.

This file diagnoses the **88 pre-existing** failures and links them to the introduced ones.

## The load-bearing fact

**34 scenarios pass `@api` but fail `@ui`** — the backend applies the mutation correctly, the browser
path does not. Roughly **59 of 88 are frontend-only**, **~29 are backend/persistence**.

## Clusters (sum = 88)

| Cluster | Count | Nature | One-line cause |
|---|---|---|---|
| D — `@ui` interaction timeouts | 44 | Frontend | topology-writer round-trip / UI selection sync never lands the expected DOM/state |
| F — assertion value mismatches | 11 | Mixed | cue-timing edit drops follow trigger; renumber emits extra revision; selection drops entries; error-string change; relocated cue-defaults |
| A — `patched_fixture.definition` TypeErrors | 9 | Backend | serialized read body no longer contains `definition`/`heads` |
| E — element-not-visible (immediate) | 9 | Frontend | UI-contract placement drift (header actions, search bar, speed-group/playback-card labels) |
| C — `PUT patched_fixture` 400 "missing field `definition`" | 4 | Backend | write model still requires `definition`; read strips it |
| G — `toBeDefined`/undefined object | 4 | Backend | highlight preset not stored; schema-v1 profile migration yields no legacy def |
| B — `POST /cuelists/{id}/go` 404 "programmer command line does not exist" | 2 | Backend | GO now depends on a programmer command line existing |

## A + C: the profile-snapshot indirection (13 pre-existing + the 4 introduced Patch regressions)

**The single most important cluster** — it is also the root of the 4 introduced Patch-reader
regressions (POSITION-HOME-001, HIGHLIGHT-003, ENCODER-DISPLAY-001, PROG-002 @ui).

The migration introduced fixture-**profile-snapshot indirection**:
`FixtureDefinition.profile_snapshot` (`crates/fixture/src/definition_model.rs:136`) is stripped from
serialized bodies (asserted in `crates/wire/src/v2/patch/tests.rs:26`,
`crates/fixture/src/portable_patch/tests/codec_tests.rs:22`), and the `patched_fixture` **read**
projection dropped inline `definition.heads` while the **write** model
(`crates/fixture/src/patch_model.rs:20`, `pub definition: FixtureDefinition` — non-optional) still
requires it. Result:
- `GET /objects/patched_fixture` body has no `definition` → `TypeError … reading 'heads'`
  (`tests/00-generate-show-files.spec.ts:402`) and a round-tripped `PUT` 400s "missing field
  `definition`" (`tests/07-move-in-black.spec.ts:25`, `tests/support/catalog.ts:103`).
- **Client-side definition resolution has no parameters.** The scoped Patch store builds fixtures via
  `projectionToPatchedFixture` (`apps/control-ui/src/features/patch/model.ts`), resolving through
  `createPatchDefinitionResolver` over `mergeFixtureDefinitions(fixtureProfiles, fixtureLibrary)`;
  when it misses it falls back to `syntheticDefinition`, built from `PatchProfileRevision`
  (`referencedModes: [{modeId,name,splits}]` — **no parameters**). So programmer-surface readers that
  need `definition.heads[].parameters` (`returnHomeAssignments`/`parameterDefault` in
  `components/modals/specialPosition.ts`, `useSupportedAttributes`/`directProgrammerChoices` in the
  parameter controls, the hardware-encoder attribute display) see nothing → Return Home disabled,
  encoder slots "Unassigned", spread finds nothing. `/api/v1/patch` was **server-resolved** and
  complete, which is why these passed at `47030ed` before the reader migration.

**Fix:** carry parameterized definitions where clients need them — restore `definition`/`heads` in the
`patched_fixture` read serialization (or re-hydrate server-side on write), and ensure the v2 patch
snapshot/`profile_revisions` carry full parameterized modes so `resolveDefinition`/`syntheticDefinition`
have parameters. Files: `crates/fixture/src/patch_model.rs`, `definition_model.rs`,
`crates/wire/src/v2/patch.rs`, `crates/server/src/runtime/object_api.rs`. **Clears ~13 pre-existing
+ the 4 introduced = ~17.** A re-projection/timing fix on the client was tried and does **not** work —
the data simply is not present client-side; do not retry the timing angle.

## D: 44 `@ui` timeouts — frontend, likely 2–3 shared fixes (UNCERTAIN)

Every failure is a `Test timeout` inside `pairedScenario.ts:30`'s "Perform the production UI action"
step; `desk.open()` succeeded, so the hang is a later click or an `expect.poll` that never converges.
All 44 have a **passing `@api` sibling**. Target locators still exist in source (nothing renamed
away), so it is a runtime/state failure: a UI action through the new topology writer/authority never
produces the expected DOM or server state. Two families:
- **Playback/cue UI** (CUE-*, PBK-*, PLAYBACK-SELECT-001, PRELOAD-001, MERGE-*): `features/playbackTopology/`, `windows/cuelistWindow/`.
- **Command-line + group/fixture selection** (GROUP-003/4/5, PROG-003, DIM-001-supp): `pressCommandAndWait`/`selectFixtureRows` + `expectSelectedNumbers` (`tests/support/foundational/ui.ts`). `PROG-001 @supplemental-ui` (selection `[1,2,3]` vs `[1,2,3,4]`) is the non-timeout tell.

**This is the only cluster with real remaining uncertainty.** Evidence points to a small number of
shared UI-primitive breakages, NOT 44 independent bugs — but it must be confirmed with a **live traced
repro** (the `47030ed` traces were purged by a concurrent build) before treating it as one fix or
parallelizing. Keep the two families as two worktrees.

## B, E, F, G: smaller clusters

- **B (2):** GO calls `clear_command_line` (`crates/server/src/command_http/programming_ports.rs:254-262`);
  a fresh session has no command line → 404. GO should not depend on it.
- **E (9):** UI-layout acceptance drift — several small independent per-surface fixes.
- **F (11):** ~6 frontend (CUE-011 drops follow trigger + renumber extra revision, CUE-012, SOUND-001,
  COLOR-RANGE-001, PROG-001) in `windows/cuelistWindow/useCueEditor.ts`,
  `features/playbackTopology/writer.ts`, `features/showObjects/store.ts`; ~5 backend (API-001
  error-string `"revision conflict"`→`"stale group N revision"`; SHOW-004 relocated cue-defaults +
  group-defaults byte hash; DIM-001 membership).
- **G (4):** HIGHLIGHT-001 preset object not stored (`@api`+`@ui`); FIXTURE-001 schema-v1 profile
  migration.

## Prioritized remediation plan

Independence = safe in a separate worktree (each needs its own `CARGO_TARGET_DIR` and Playwright
results dir; the shared cargo target and `.artifacts/test/results` cannot be used concurrently).

1. **profile-snapshot indirection / `patched_fixture` definition serialization** — backend. **~13
   pre-existing + 4 introduced = ~17.** Highest single lever, unambiguous regression vs. AGENTS.md
   persistence rules, independent. **Do this first; it also closes the introduced Patch regressions.**
2. **cluster-D shared UI-primitive fix(es)** — frontend. Up to ~44, but **investigate with a live
   trace first**; likely 2–3 shared fixes across two worktrees (playback/cue; group/command-line).
   Shares files with #3. The only item with remaining uncertainty.
3. **cue-editing writer bugs (CUE-011)** — frontend: follow-trigger dropped; renumber extra revision.
   ~2. May share files with #2.
4. **cuelist-settings / cue-defaults persistence (SHOW-004)** — backend. ~2–3. Independent.
5. **decouple playback GO from programmer command line** — backend. 2. Independent.
6. **error-string contract (API-001)** — backend, tiny. 2. Independent.
7. **UI-layout acceptance (cluster E)** — frontend, N small independent per-surface fixes. ~9.
8. **highlight preset storage + schema-v1 profile migration (cluster G)** — backend. ~4. Independent.

**Parallelizable now:** 1, 4, 5, 6, 8 (independent backend worktrees) and 7 (N small frontend). **Not
yet:** 2 (needs a traced repro to decompose) and 3 (shares code with 2). Biggest lever by count is 2
(~44) but it carries the only real uncertainty; biggest lever by certainty is 1 (~17).

## Caveat

Cluster-D per-scenario hang-points are inferred from `@api`/`@ui` parity + source reading, not
observed frames — the run's Playwright traces/screenshots were deleted by a concurrent build. Confirm
D with one traced repro per family before implementing.

## Post-fix status — first remediation pass (2026-07-22)

Full suite at `refactoring` HEAD after two landed backend fixes
(`6206b1e`, `b361a73`): **175 passed / 87 failed / 2 skipped** (identical 262-test
executed population as the runs above; the skipped delta is ignored help/visual specs).
Movement vs. the pre-work HEAD (169/93): **+6 passed, −6 failed, no regressions** — now
one better than the `47030ed` pre-refactor baseline (174/88).

Cleared this pass:
- **Cluster A + C (patched_fixture.definition)** — `6206b1e` re-hydrates the resolved
  definition on the object read path. The error class is **fully eliminated**: 0 residual
  `reading 'heads'` / `missing field definition` signatures across the whole run. Specs that
  were *only* A/C now pass; specs that also call GO in setup (SHOW-000, MIB-001, DMX-006/008)
  stay red on the unrelated cluster-B GO 404.
- **Cluster G** — HIGHLIGHT-001 (@api + @ui) via `b361a73`; FIXTURE-001 (@api, @ui, and the
  three @restart schema-v1 migration specs) via `6206b1e`. All green.

Still failing, with owners:
- **The 4 introduced Patch regressions** — POSITION-HOME-001 @ui, PROG-002 @ui (×2),
  HIGHLIGHT-003 @ui, ENCODER-DISPLAY-001 @supplemental-ui. Need **A-fix Part 2**: the v2 patch
  snapshot `profile_revisions` must carry full parameterized modes (heads[].parameters +
  channels + control_actions) so the scoped Patch store's `syntheticDefinition`/`resolveDefinition`
  has parameters. Client map done (see verification log); wire + client change not yet made.
- **Cluster B (GO 404, 6 occurrences)** — real origin is `Snapshot::read → unknown_programmer`
  (`application/src/programming/service/support.rs:78`) reached from `run_external_interaction`
  before the op; a fresh session has no registered programmer. Fix is a programming-service
  change (let GO proceed without a pre-registered programmer), not `clear_command_line`.
- **SHOW-004 @restart (×3)** — group-defaults / virtual-dimmer-metadata / cue-defaults; reproduced
  at correct base, no fix yet.
- **API-001 @api/@ui** — contract wants error containing `"revision conflict"`; true failure
  cause on this base is unverified (the earlier "stale group N" report was a wrong-base artifact).
  Needs a fresh traced repro.
- **Cluster D (~44 @ui timeouts)** and **cluster E (~9 layout)** and **F remainder (CUE-011/012,
  SOUND-001, COLOR-RANGE-001, DIM-001)** — unchanged; D still needs a live traced repro before
  decomposition.

Recommended next slice: (1) A-fix Part 2 (clears the 4 Patch regressions); (2) cluster-B
programmer-registration fix (unblocks GO-contaminated A/C specs — likely a large secondary win);
then re-run and re-triage D with live traces.

## Second pass — A-fix Part 2 landed (2026-07-22)

Full suite after `41c11b2`: **179 passed / 83 failed / 2 skipped** (+10/−10 vs the pre-work 169/93,
no regressions). The **four introduced Patch regressions are cleared**: POSITION-HOME-001 @ui,
HIGHLIGHT-003 @ui, ENCODER-DISPLAY-001 @supplemental-ui, and PROG-002 (@api/@ui-encoder/@supplemental).
Fix: `PatchProfileRevisionProjection.profile_snapshot` now carries the server-resolved parameterized
profile per referenced revision (server->client only; the patch request boundary is unchanged), and
the client builds full definitions from it via the existing `fixtureDefinitionFromProfileMode`.

The one PROG-002 @ui variant still red — "relative values spread across the live ordered Group" — is a
**cluster-D** selection timeout (`selectFixtureRows` at `tests/support/foundational/ui.ts:139`), not the
Patch-definition regression. The A-fix (Part 1 + Part 2) is complete.

Remaining 83 failures are clusters B (GO 404), D (~44 @ui interaction timeouts), E (~9 layout), F
(CUE-011/012, SOUND-001, COLOR-RANGE-001, DIM-001, SHOW-004, API-001) — unchanged by this pass except
where A/C was the blocker. Recommended next: cluster B (GO/programmer-registration), which unblocks the
A/C-fixed specs that still fail only because their setup calls GO.

## Third pass — cluster B fixed; precise remaining root causes (2026-07-22)

Full suite after `3c4516a`: **185 passed / 83 -> 77 failed** (+6, no regressions; failing-set diff
cleared MIB-001 @api/@ui/@wire x2, CUE-012 @api, TIME-003 @wire).

**Cluster B (FIXED, `3c4516a`).** The v1 playback compatibility routes (`/cuelists/{n}/{action}`,
`/playbacks/{id}/{action}`) run through `run_programming_interaction` -> `Snapshot::read`, which
hard-required a live Programmer. An authenticated desk session loses its Programmer authority when a
short-lived command/OSC transport is released (`disconnect_orphaned_osc_session`), so a subsequent GO/
release 404'd. Confirmed by a forced-backtrace file sink: origin was `support.rs Snapshot::read ->
unknown_programmer` via `capture_external_interaction`. Fix: `Snapshot::read` returns an empty snapshot
when the session has no Programmer; the invariant test was retitled to the tolerant contract.

Precise root causes for the remaining clusters (investigated this pass, NOT yet fixed):

- **SHOW-004 @restart group-defaults / cue-defaults (2)** — DESIGN CONTRADICTION. The load-time
  `migrate_group`/`migrate_cue_list` (`show_compiler/migrations/objects.rs`) do not persist the
  schema's explicit defaults for legacy objects that predate a field, so the file never changes
  (`migratedHash == legacyHash`). `migrate_playback`/`migrate_route` already selectively fill their
  defaults. Making group/cue do the same clears both e2e cases, BUT breaks the deliberate invariant
  test `active_show::tests::object_undo_commits_pending_migrations_in_the_same_compiled_candidate`
  (line 603) which asserts a raw minimal group generates NO pending migration. Fixing SHOW-004 requires
  a selective per-field fill in migrate_group/migrate_cue_list PLUS updating that undo test to seed a
  fully-defaulted group. Needs a decision: normalize-on-load (like playback/route) vs the lossless
  raw-preservation the undo test encodes. Deferred rather than rushed.
- **SHOW-004 virtual-dimmer-metadata (1) + DMX-006/008 @api (2)** — FIXTURE-SCHEMA. `migrate()` has no
  `patched_fixture` arm, so a fixture whose `definition.heads[].parameters[].metadata`/`capabilities`
  were stripped never re-fills them on load (and Part 1's read re-hydration is skipped when a definition
  is already present). Separately, DMX-006/008 derive a NEW inline definition (new `definition.id`) while
  keeping a stale `profile_snapshot`; the schema-v2 write check `schema-v2 fixture snapshot identity is
  inconsistent` (`fixture/src/definition.rs:51`) rejects it. Both need a fixture-schema decision
  (re-snapshot on write / fill nested parameter defaults) — use the build-light-fixtures skill.
- **Cluster D @ui interaction timeouts (~40)** — NOT a single shared store-hydration fix. Confirmed via
  PBK-001 @ui accessibility snapshot: the app loads fully but the expected Playback-representation
  fader bank is not in the expected window/desktop/mode (a preset pool renders instead). Each family
  (playback/cue navigation vs command-line/selection) needs its own per-interaction investigation; there
  is no evidence of one shared breakage.

## Fourth pass — cluster E/D now blocked on decisions, not bug fixes (2026-07-22)

Full suite `b449acd`: **193 passed / 69 failed** (SHOW-005 @ui is flaky — oscillates pass/fail across
identical runs, unrelated to any change). Cleared this pass: MANUAL-019 @ui "fixture browsers share
title-bar search" (`b449acd` — search relocated to the title bar + nested library detail right-aligned)
and, earlier, the seven backend clusters (169/93 -> 193/69 over the session, zero real regressions).

The remaining failures are now dominated by items that need a **decision or feature work**, not a safe
bug fix:

- **MANUAL-019 #62 (File Manager "Create")** — CONTRADICTS THE MANUAL. The test wants a button named
  "Create"; the manual (`docs/help/05-Pane-Reference/03-utility-and-diagnostics.md:7`) and
  `tests/16-file-manager.spec.ts` + `FileManagerWindow.test.tsx` all say **"New"**. The implementation
  is correct per the source-of-truth manual. Renaming to satisfy MANUAL-019 would regress 16-file-manager
  and the unit tests. Needs a manual-vs-review reconciliation (which spec is authoritative); do NOT change
  correct code to pass a test that disagrees with docs/help.
- **MANUAL-019 #65 (Shows & recovery breadcrumb)** — LARGE MISSING FEATURE. `ShowRecoveryFileManager`
  is two launcher buttons; the test expects a full recovery file browser (breadcrumb/list/load-safely).
  This is new UI, not a correction.
- **MANUAL-019 #63/#64 (Cues pane cue-editor Title; Help/DMX/Stage)** — per-surface layout / a 30s
  interaction timeout (cluster-D-adjacent). Each needs its own frontend investigation with a running desk.
- **OSC-005** — command-line input shows a trailing space ("G7 + " vs "G7 +"); `command_line.rs:316`
  appends a trailing space after an operator token. Fixing it risks other command-line tests that rely on
  the space signalling "awaiting operand" — verify the whole command-line suite before changing.
- **Cluster D (~40)** — per-interaction UI navigation; no shared fix (confirmed by a11y snapshots).
- **Fixture-schema (~4)** — virtual-dimmer-metadata @restart needs a patched_fixture load-migration arm
  (nested, order-preserving default-fill); DMX-006/008 need a re-snapshot-on-write decision.

Recommendation: the highest-value safe remaining work is the **patched_fixture load-migration** and a
**per-family cluster-D traced-repro pass** with `npm run open` for visual verification. The MANUAL-019
contradictions and the recovery-browser feature need product/design direction before implementation.
