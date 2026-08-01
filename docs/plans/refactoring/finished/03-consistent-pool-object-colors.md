# Consistent Pool-object Colors

## Status and source contract

Finished after plan 02's accepted shared pool-card and pool-grid primitives. Implemented
[`../../Done/55-consistent-pool-object-colors.DONE.md`](../../Done/55-consistent-pool-object-colors.DONE.md)
through the shared pool primitives.

Do not run this plan concurrently with the active Storybook lane: it changes the same pool
presentation contracts, styles, stories, screenshots, and component tests.

Estimated effort: 0.5–1 Codex day.

## Required work

1. Add one shared color resolver covering object type, Preset family, pane mode, item override,
   selection, focus, Store/Record/Update, disabled, and empty states.
2. Use the documented defaults: Dynamics cyan, Cuelists/Sequences lime, future Macros dark red,
   Groups pale orange-yellow, and Presets grey.
3. Add per-pane type-color versus individual-color mode. Individual mode uses grey for items
   without an explicit color.
4. Persist user/default palette choices as desk/user presentation settings, not portable show
   content.
5. Reuse or generalize item presentation colors without parallel per-object storage paths.
6. Apply the resolver to every pool, shortcut, assignment, and hardware-connected software surface.

## Acceptance and verification

- Test every type default, Preset family setting, individual override, reset action, and mode
  switch.
- Verify readable contrast and non-color indicators for selection, focus, armed, disabled, and
  empty states.
- Run package/component tests, settings persistence tests, focused UI Playwright, screenshot
  regeneration, and manual verification.

## Result

Implemented in the semantic-release commit
`feat(ui): unify pool object colors`.

### Changes

- Added one shared resolver and palette for Groups, Presets by family, Cuelists, Sequences,
  Dynamics, and future Macros. Type mode uses the documented defaults; individual mode shows
  only an explicit item color and otherwise falls back to grey.
- Added desk-persistent, typed pool presentation configuration with validation, generated wire
  declarations, an isolated API patch, replay-safe mutation handling, and show-qualified pane
  and item keys. The settings do not enter portable show data.
- Generalized legacy Preset button presentation into the shared item store and added a one-shot,
  non-destructive migration from browser-local storage.
- Added per-pane mode and palette controls, one-color and whole-palette reset actions, and saving
  feedback. Applied the resolver to full Group, Preset, and Cuelist pools, Group shortcuts,
  Virtual Playbacks, and touch and hardware-connected software playback layouts.
- Preserved portable Group and Playback colors as read-only individual-mode compatibility
  fallbacks while routing new Preset presentation edits to desk data.
- Added non-color focus, selected/active, Record/Store, Update, disabled, and empty treatments.
  Removed the legacy hard-coded Preset-family tile override; family selector buttons remain
  distinct while Preset tiles use the configured family palette.
- Updated operator help and regenerated the affected Group, Preset, and Cuelist screenshots.

### Tests

- Rust pool-presentation route tests passed: 2 typed validation, replay, and desk-persistence
  tests. `cargo check -p light-wire -p light-headless-runtime` and `cargo fmt --all --check`
  passed.
- Shared UI tests passed: 16 resolver, Pool Card, and Virtual Playback tests. Focused desktop
  tests passed: 98 settings, migration, configuration-action, Preset, Cuelist, Virtual Playback,
  and touch/hardware-connected playback tests.
- Both desktop and UI-library TypeScript typechecks passed.
- The complete Storybook Playwright suite passed: 226 tests. After removing the legacy Preset
  override, the focused default/mode/contrast/state, Preset story, and configured-marker checks
  passed, and the reviewed help-screenshot regeneration passed.
- `npm run manual` rebuilt and verified the 142-page PDF manual and offline HTML manual.
- `npm run open` built both Tauri applications and launched the desktop path. The real service
  reported `ready`, with the active show loaded, no active-show error, and recovery mode off.
  Manual review of the real Preset surface exposed the legacy family override; the corrected
  grey-default result was then reviewed in the regenerated application screenshot.

### Limitations

- Dynamics currently has only its documented empty future-feature surface, and Macros have no
  implemented pool, so their colors are covered by the shared resolver, settings, and tests
  without fabricating unavailable runtime UI.
- Palette and item maps use the existing desk-configuration last-write-wins behavior between
  simultaneous clients. The dedicated patch prevents pool edits from overwriting unrelated desk
  configuration.
- A concurrently edited, unstaged shared Icon Picker no longer exposes the legacy `Use ★`
  button expected by one unrelated Group-properties test. That picker work and its test mismatch
  are not part of this plan or commit; the plan-specific Group surface passed before that
  concurrent change and the current pool-color acceptance remains green.
