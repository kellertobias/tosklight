# ToskLight refactoring — handoff & remaining work

Single source of truth for finishing the refactoring on branch `refactoring`. The architectural
intent lives in [`major-refactoring.md`](./major-refactoring.md); this file is the current state,
the remaining chunks, and the decisions needed to finish independently. Older trackers
(`refactoring-progress.md`, `refactoring-verification-log.md`, `e2e-failure-diagnosis.md`, the root
`REFACTORING-HANDOFF.md`, `PLAYBACK-RUNTIME-DESK-SCOPE-REFACTOR.md`) are superseded and folded in
here; their detail is in git history.

## Working rules

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

## Current state (2026-07-22)

- Baseline (pre-refactor) 174 passed / 88 failed → session start 231 → **258 passed** now.
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

## Remaining chunks

### P1 — DEGRP / group-selection consistency (engine bug; must fix)
DEGRP is implemented two ways. Keypad `GROUP GROUP <id>`
(`crates/server/src/runtime/programmer_group_commands.rs:163` sets `frozen=true`) →
`SelectionExpression::FrozenGroup` (keeps a group reference). Double-click
(`SelectionGestureSource::DereferencedGroup`) → `Sources` of individual fixtures (no reference).
Per the model, the **keypad path is wrong**.
1. Make keypad DEGRP dereference to `Sources` (individual `Fixture` refs) — identical to the
   double-click gesture; the two must match.
2. Remove `SelectionExpression::FrozenGroup` and its handling (`selection.rs`, `group_recording.rs`,
   `update_capture.rs`, `highlight/selection.rs`) once nothing produces it. **First** check whether
   `selection_expression` is persisted in show files: if yes, add a load-migration
   (`frozen_group` → dereferenced `sources`) and test old-show load; if it is live-only programmer
   state, remove cleanly. Do **not** touch the unrelated `groups::FrozenGroup` struct (`frozen_from`
   on recorded groups — a legitimate referenced/derived-group concept).
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
- **D3 — virtual-dimmer-metadata / 16-bit fixture output (DMX-006/008, SHOW-004): pending — the
  maintainer will decide after reviewing context (being prepared).** Keep these skipped until then.
- **D4 — OSC-005 command-line trailing space: no trailing space (`"G7 +"`).** Command-line spaces are
  cosmetic only. Change the engine's command-line formatting to not append a trailing space (search
  `crates/programmer/src/command_line.rs` `format!("{command} {next_token} ")`), update the
  `softwareKeypad` unit test and `docs/help/30-Programmer/01-command-line.md` to match, and unskip
  OSC-005 @osc. Verify no other command-line assertion depends on the trailing space.
