# Consistent Pool-object Colors

## Status and source contract

Pending, blocked on plan 02's accepted shared pool-card and pool-grid primitives. Implement
[`../../Next/55-consistent-pool-object-colors.md`](../../Next/55-consistent-pool-object-colors.md)
after shared pool primitives exist.

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
