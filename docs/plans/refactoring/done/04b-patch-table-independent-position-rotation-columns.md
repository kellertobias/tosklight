# 04b — Patch table: six independent location/rotation columns

## Context (maintainer requirement, 2026-07-23)

The Show Patch table must not present position and rotation as single combined columns.
Verified: `apps/control-ui/src/components/setup/fixturePatch/PatchTable.tsx:22-35` defines
one `"Location X/Y/Z"` and one `"Rotation X/Y/Z"` column; the cells render the whole
triple (`:262` `formatRotation(fixture.rotation)`, multipatch at `:345-348`) and arm the
edit for the whole triple (`armEdit(controller, fixture, "rotation")` `:260`,
`beginMultipatchEdit(…, "rotation")` `:345`).

Required instead: **six independent columns** — Location X, Location Y, Location Z,
Rotation X, Rotation Y, Rotation Z — each independently configurable: a cell edits only
its own axis and never resubmits the sibling axes.

## Work

1. Split the two combined columns into six in `PatchTable.tsx` (fixture rows and
   multipatch instance rows both); each cell shows one axis value and arms an edit for
   that axis only (`editSession.ts` / `editSave.ts` / `PatchEditSurfaces.tsx` currently
   model location/rotation as triples — extend the edit model to a single-axis target).
2. Single-axis writes go through the server as a partial intent (one fixture, one axis,
   one value) — align with chunk 04's stage-layout intent route so patch-table edits and
   stage-view moves share the same write path rather than two layout-save mechanisms.
   Sequence this chunk **after 04**.
3. Keep touch-target sizing workable with the extra columns (desk surface rule); check
   the table still fits its pane at default width or scrolls per the window-kit pattern.
4. Operator vocabulary: keep the existing "Location"/"Rotation" terms (help uses them) —
   i.e. "Location X" … "Rotation Z" — unless the maintainer prefers "Position"; confirm
   in passing, non-blocking. Update `docs/help/20-Show-Setup/**` where the patch-table
   columns are documented, and refresh help screenshots only if that chapter shows the
   table (`npm run test:help-screenshots` deliberately).

## Definition of done

- Six columns, each cell independently editable; editing one axis provably leaves the
  other five untouched (test asserts the persisted layout delta touches one axis).
- Multipatch instance rows behave identically.
- Help updated if the table is documented there.

## Verification

```sh
npm run test:unit
npm run test:e2e -- tests/<patch spec>
npm run test:e2e   # full suite gate
npm run manual     # if help changed
```

Manual: `npm run open`, edit Rotation Y on one fixture, confirm X/Z unchanged and the
stage view reflects only that axis.

## Decisions

None blocking. Column label wording (Location vs Position) — confirm with the maintainer
in passing; default to the existing "Location" vocabulary.

## Result

**What changed.** The Show Patch table's two combined transform columns became six —
Location X/Y/Z and Rotation X/Y/Z — for fixture rows and multi-patch instance rows alike
(`PatchTable.tsx`). Each cell arms an editor for exactly that axis: `armEdit`/
`beginMultipatchEdit` carry an axis, the edit dialog renders a single autofocused field
titled e.g. "Set fixture rotation Y", and the dirty/close-confirm checks compare only that
axis. A save recomposes the triple over the record's **current** siblings
(`editSave.ts`/`multipatchActions.ts`), so a single-axis edit can never resubmit a stale
sibling value. `formatRotation` became dead and was removed. Help documents the six
columns; screenshots refreshed (`show-patch.png` now shows the new header).

**Write-path decision (chunk item 2).** The planned alignment with chunk 04's
stage-layout intent route is moot after 04d: patch placement (`patched_fixture`
location/rotation, mm) is now the only positioning surface, and its sanctioned server
path is the v2 PatchFixtures intent — so there is exactly one layout-save mechanism.
Single-axis semantics are enforced at the edit model with contract tests asserting a
one-axis delta over the current record for both fixture and multipatch rows.

**Suite numbers.** `npm run test:unit` 275 files / 1984 tests green (two new one-axis
contract tests); full `npm run test:e2e` 280 passed / 12 skipped / 1 failed —
the failure was FIXTURE-002 @restart, on the README's known flaky-in-suite list and
green in isolation per the standing rule, so the gate is at baseline. `npm run manual`
verifies; architecture/source-size checks green. Manual acceptance against the live
desk: editing Rotation Y to 33° changed exactly one fixture's rotation.y across the
whole patch (locations and x/z untouched), then reverted.

**Surprises.**
- `apps/control-ui/e2e/operator-output.spec.ts` (and `deterministic-bench.spec.ts`) are
  orphaned: the root Playwright config's `testDir` is `./tests` only, so no suite ever
  runs them. The header assertion there was updated anyway; candidates for chunk 23
  housekeeping (delete or re-home).
- `tests/product-demo.spec.ts` drives the vector editors; its helpers were rewritten to
  the per-axis flow (spec remains deliberately skipped pending chunk 22).

**Follow-ups filed.** None new — the orphaned-spec finding is noted for chunk 23.
