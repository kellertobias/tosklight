# 10c5 — Operator controls and final cleanup

## Outcome

Migrate the remaining operator-control UI cases, remove orphaned compatibility
helpers, and close the root UI migration.

## Scope

- `11-update-highlight-fixture-profiles-and-matter.spec.ts`
- `14-sound-to-light.spec.ts`
- `21-completion-coverage.spec.ts`
- `25-return-home-position-special-dialog.spec.ts`
- `26-color-special-dialog-alignment.spec.ts`
- `28-hardware-connected-playback-selection.spec.ts`
- `30-command-line-history-panel.spec.ts`
- `31-hardware-connected-encoders.spec.ts`
- `32-software-encoder-value-modal.spec.ts`
- orphaned root UI helpers and final author guidance

## Done gate

- All 22 pending inventory rows in scope are migrated or narrowly justified, and
  no inventory row anywhere remains pending.
- Hardware/software mode boundaries, encoder NAV/turn/held-turn/click semantics,
  touch and OSC parity, Highlight, Update, color, and playback ownership remain
  exact.
- Orphaned wrappers and compatibility helpers are removed.
- `docs/testing/README.md` is the concise final author guide.
- Full unit and browser suites, architecture, inventory, recording catalog,
  product demo, and parallel stress pass.
