# 06d — Profile-derived discrete and special Programmer controls

## Outcome

Complete truthful semantic helpers for discrete fixture functions and existing Programmer special
actions without pretending that fixture-specific values form one global enum.

## Scope

- Derive typed discrete choices from the selected fixtures' portable profile functions and stable
  semantic IDs.
- Keep normalized Media values such as Opacity and Speed numeric; expose profile-specific Layer,
  Clip, Control, wheel, and similar values only when the active patch actually supplies them.
- Add semantic set, release, and clear assertions for normalized and discrete values.
- Add helpers for the documented Position Return Home dialog, Position alignment modes, Beam and
  Shapers dialogs, and compatible Control actions.
- Preserve ordered selection, Programmer Fade, capture mode, LTP ownership, and one-step Undo.
- Do not expose modal strings, physical slots, raw fixture UUIDs, or raw profile channel values to
  scenario authors.

## Verification

- Catalog/type tests for mixed compatible and incompatible fixture selections.
- Focused visible and API scenarios for one profile-derived discrete value and each existing
  special-action family.
- TypeScript, architecture, and full Playwright regression gates.

## Result

- Added a selection-scoped discrete catalog derived from fixed and indexed functions in the
  portable profile snapshot. Scenario authors use semantic IDs and labels; compatible fixture
  identities remain internal and mixed incompatible selections do not receive unsupported values.
- Added semantic discrete set, API and visible release, and clear controls, plus normalized encoder
  release and Programmer clear operations. Writes preserve the current Programmer Fade timing.
- Added `special` helpers for visible Position Return Home and alignment, profile-available Beam
  and Shapers pointer controls, and compatible Control actions. API alignment and Control routes
  follow the production command boundary, with fixture Control mutations serialized across the
  active-show transition barrier.
- Added pure catalog coverage for mixed fixtures and stable logical-head identity, plus focused
  browser scenarios backed by a synthetic portable profile. This covers fixed/indexed values even
  though the currently shipped fixture packages do not yet contain such functions.
- Architecture, catalog/type tests, the frontend build, and both focused browser scenarios pass.
  The full Playwright regression completed with 314 passed, 9 skipped, and no failures.
