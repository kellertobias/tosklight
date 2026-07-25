# Empty-selection Object Target Selection

## Status and source contract

Pending. Implement
[`../../Next/56-empty-selection-preset-and-effect-target-selection.md`](../../Next/56-empty-selection-preset-and-effect-target-selection.md).

Estimated effort: 0.5–1 Codex day.

## Required work

1. Add one authoritative action that resolves the ordered targets stored by a populated Preset or
   Effect/Dynamic-like object.
2. With an empty programmer selection, the first ordinary tap selects those targets and does not
   recall or apply the object.
3. A second tap, or any tap with an existing selection, keeps ordinary recall/apply behavior.
4. Preserve Store/Record, Update, and Set precedence.
5. Include unpatched fixtures; skip missing targets with an actionable, unobtrusive warning.
6. Keep Stage, Fixture Sheet, command line, OSC, attached hardware, and all pool surfaces on the
   same authoritative programmer selection.

## Acceptance and verification

- Cover Color, Position, and Mixed Presets, ordered target unions, empty slots, missing targets,
  unpatched fixtures, armed modes, first tap, and second tap.
- Add domain/action tests plus API, OSC/hardware, and real UI Playwright parity checks.
- Verify shared pool components emit intent only and do not hold local-only selection state.
