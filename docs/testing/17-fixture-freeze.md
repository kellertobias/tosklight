# Fixture Freeze

## Purpose

Prove that Freeze retains resolved fixture output without rewriting its underlying Programmer,
Playback, Dynamic, direct-control, or master state, and that every operator surface presents the
same persisted result.

## Status and coverage

Executable coverage is `tests/124-fixture-freeze.spec.ts` (Playwright, bench server, run by
`npm run test:e2e`), with scenario IDs `FREEZE-FULL-00n`, `FREEZE-PARTIAL-00n` and
`FREEZE-PERSISTENCE-00n`. The bench rig is `tests/bench/specific-features/fixtureFreezeScenario.ts`:
three intensity fixtures driven by the Programmer, a running Cue and a running Dynamic, a ROBE Robin
600X LEDWash (multi-head colour/position) and a Cameo ROOT PAR 6 (single-head colour) in Group 10,
under a Group Master and the Grand Master below Full. Physical output is read from the bench Art-Net
receiver and checked against the logical frame of the same manual-clock step.

- Full Freeze, steps 1–5: `FREEZE-FULL-001` drives the touch keypad (`[^CLR]`, `[GRP] 1 0`,
  `[ENT]`; `[^CLR][^CLR]` for Unfreeze), checks the Fixture Sheet `❄ FREEZE` / `❄ FREEZE INSIDE`
  markers, held DMX and visualization through every source, both masters and Blackout, and that
  Unfreeze reveals the untouched Programmer and Cue. Known defects are kept as expected failures:
  `FREEZE-FULL-002` (multi-head Master Pan/Tilt/Intensity not held), `FREEZE-FULL-003` (physical
  colour of semantic-colour fixtures not held) and `FREEZE-FULL-004` (the Freeze itself changes a
  virtual-dimmer colour fixture's output).
- Partial Freeze, steps 1–6: `FREEZE-PARTIAL-001` (keypad `[^1][^2]` grammar, family labels,
  retained Intensity/Colour semantic values, Position and Beam follow, masters and Blackout
  followed) and `FREEZE-PARTIAL-002` (repeating the family live action removes it; full over
  partial restores no partial metadata). Step 5 uses the Toggle family live action; repeating the
  explicit `FREEZE … INTENSITY COLOR` command line is idempotent. `FREEZE-PARTIAL-003` is an expected
  failure for the defect that Group and Grand Master apply twice to a partial Intensity Freeze.
- Persistence and parity: `FREEZE-PERSISTENCE-001` (steps 1–2, saved show file reopened and an
  unrelated Show Patch edit), `FREEZE-PERSISTENCE-002` (step 3, touch keypad, OSC and attached
  hardware each produce the same stored Freeze and exactly one Patch revision per action),
  `FREEZE-PERSISTENCE-003` (step 4, `[UND]` ordering and no Redo) and `FREEZE-PERSISTENCE-004`
  (step 5, the canonical older `compact-rig.show`).
- Not in Playwright: the bench server hosts a single desk, so desk-local Undo history is covered by
  the Rust transport tests in
  `crates/light/adapters/headless/src/runtime/fixture_freeze/native_transport_tests.rs`, which also
  cover native Position holds. Direct-control contributions are not exercised because no shipped
  rig fixture exposes a control action. Native Stage rendering of a frozen fixture remains manual.

## Full Freeze

1. Patch an intensity fixture and a multi-head color/position fixture, place them in a Group, and
   select the Group through the ordinary Group-selection path.
2. Establish visible Programmer, Cue, Dynamic, and direct-control contributions, with Group Master
   and Grand Master below Full and Blackout off.
3. Press **SHIFT + CLEAR**, enter the Group selection, and press **ENTER**. Confirm the Fixture Sheet shows `❄ FREEZE` on each resolved fixture or
   head, with `INSIDE` on a master-only row where applicable.
4. Change every contributing source, move Group Master and Grand Master, and enable Blackout.
   Confirm the frozen physical and visualization output remains exactly at the captured frame.
5. Hold **SHIFT**, press **CLEAR** twice, release **SHIFT**, enter the same selection, and press
   **ENTER**. Confirm the command line showed `UNFREEZE`, the marker disappears, and the current underlying state is
   visible immediately; no captured value has been written into the Programmer or Cue.

## Partial Freeze

1. Enter **SHIFT + CLEAR**, the mixed fixture selection, **SHIFT + 1**, **SHIFT + 2**, and **ENTER**
   to apply Intensity and Color.
2. Confirm the Fixture Sheet names both families and does not show the full `FREEZE` state.
3. Change Intensity, Color, Position, and Beam sources. Confirm only Intensity and Color retain their
   captured semantic values.
4. Move Group Master and Grand Master and enable Blackout. Confirm partial Freeze output follows all
   three masters.
5. Repeat the same family action. Confirm those families and their retained values are removed.
6. Apply a full Freeze over an existing partial Freeze, then remove it. Confirm no partial-family
   metadata is restored.

## Persistence and parity

1. Save, close, and reopen the show with full and partial fixtures. Confirm retained values, family
   names, Fixture Sheet status, and output are unchanged.
2. Make an unrelated Show Patch edit and repeat the reload check.
3. Exercise the full FREEZE and UNFREEZE grammar from touch/software keyboard, OSC, and attached
   hardware wherever that surface exposes the chord. Confirm every path reaches the same server-owned live action and its
   ordered portable-show transaction.
4. Apply a Freeze, make a newer Programmer value edit, then press **UND** twice. Confirm the first
   Undo reverses the value edit and the second restores the exact pre-Freeze state. Confirm this
   history is desk-local and Freeze does not create a Redo action.
5. Load an older show with no Freeze fields. Confirm it opens with no frozen fixtures and can be saved
   without recovery warnings.
