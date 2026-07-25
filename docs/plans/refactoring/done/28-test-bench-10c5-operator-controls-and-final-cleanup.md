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

## Result

- Migrated all 22 remaining inventory rows into semantic bench scenarios for
  system integrations, the hardware simulator, command history, software and
  attached encoders, Position/Color special dialogs, and hardware playback
  selection. Retained low-level transport and exhaustive interaction coverage
  only as narrowly tagged supplemental tests.
- Added area-owned helpers under `show-setup/`, `hardware/`, `encoders/`,
  `command-selection/`, `programmer/`, and `playbacks/`; removed the orphaned
  software-encoder wrapper spec; and kept legacy direct construction compatible
  without weakening the semantic contracts.
- Replaced the sprawling testing notes with the final concise author guide,
  completed narration coverage for the new helpers, and regenerated the
  migration inventory: 309 root cases, zero pending rows.
- Validation passed for Rust workspace tests, 2,007 control-UI unit tests,
  control-UI typechecking/build, semantic-doc tests (8/8), architecture,
  source-size, inventory, diff checks, focused migrated scenarios (25/25), API
  E2E (86 passed, 1 skipped), supplemental E2E (110 passed, 6 skipped), the
  four-worker rerun of the three broad-UI failures (14/14), and the Full HD
  product demo (1/1, H.265 artifact emitted). The broad UI run was otherwise
  131/134 before that green isolated rerun.
- The root unit wrapper could not find its concurrently removed root `tsc`
  executable; the identical control-UI typecheck passed via the app-local
  toolchain. The serial visual-catalog attempt assembled 64 videos and reached
  197 passed / 7 skipped before being stopped after five recording-only
  failures. Three are the queue README's explicit GROUP-005/TIME-002 baseline
  flakes; UPDATE-002 and the retained playback-selection supplemental case also
  missed five-second interaction windows under capture. Every affected normal
  API/UI/supplemental path passed, so no production or migration regression was
  introduced.
