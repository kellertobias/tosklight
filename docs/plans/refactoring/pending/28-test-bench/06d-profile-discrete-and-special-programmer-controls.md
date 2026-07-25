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
