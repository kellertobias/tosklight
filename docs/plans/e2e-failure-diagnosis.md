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
