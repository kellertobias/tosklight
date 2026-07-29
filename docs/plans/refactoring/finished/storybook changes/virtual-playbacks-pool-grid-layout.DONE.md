# Virtual Playbacks Pool-Grid Layout

> **Superseded identity note (2026-07-29):** The pool-grid visual contract remains
> valid, but current-page stable-cell identity and pane-owned exclusion persistence
> below describe the former model. Plan 18 now owns 300-number page banks and
> show-owned, cross-desk exclusion zones keyed by Virtual Playback number.

## Status

Complete.

## Operator contract

1. Virtual Playbacks use a pool-grid-style layout of playback boxes.
2. Do not represent the Virtual Playbacks window as a bank of vertical playback faders or physical
   hardware playback cards.
3. Every visible grid position is a box, including empty and unassigned positions.
4. Each assigned box remains a one-button, faderless virtual playback target.
5. Preserve the configured row and column count and the exact stable cell position on the current
   playback page.
6. Changing the current playback page keeps the grid positions and changes the assignments shown
   at those positions.
7. Hardware connection must not replace the virtual grid with hardware-reduced playback cards;
   Virtual Playbacks remain an on-screen pool-style surface.
8. Preserve box presentation for slot number or icon, name, action, current Cue/runtime state,
   configured color, optional image background, and empty/unavailable state.

This is a pool-grid visual and composition requirement. It does not implicitly change the current
Virtual Playback slot limit or force the generic 200-slot pool-window contract onto a configured
Virtual Playbacks surface. Any change from the documented configurable rows×columns and current
slot compatibility requires a separate explicit decision.

## Package and application boundaries

Keep the functional Virtual Playbacks grid in `apps/light-desktop`, compose it from the shared
package grid/box primitives where useful, and load that application component into Storybook. Its
application-owned typed state must cover:

- stable slot number and grid position;
- assigned, empty, or unavailable state;
- label, icon, color, and optional background image;
- action label and held-action behavior;
- running and current-Cue feedback;
- configuration, assignment, update, exclusion-member, and exclusion-selection states; and
- pointer-down, pointer-up/cancel, click, configure, assign, update, and zone-selection callbacks.

The Storybook harness must replace playback services, page stores, command-line state, server APIs,
Tauri, and virtual-playback zone persistence with deterministic in-memory application providers.

The `apps/light-desktop` adapter retains:

- current-page playback addressing;
- configured rows and columns;
- playback and Cuelist projections;
- runtime action execution;
- SET/configuration/update routing;
- flash/swap press and release semantics;
- exclusion-zone membership and persistence; and
- assignment and page-change behavior.

## Storybook correction

- Replace the `playback-fader-bank`, `TouchPlaybackCardView`, and
  `HardwarePlaybackCardView` composition in the Virtual Playbacks story in
  the relocated mock complete-window story before that file is deleted.
- Render a deterministic pool-style grid of virtual playback boxes instead.
- Add dedicated stories for:
  - a sparse grid containing assigned and empty boxes;
  - running, configuration, assignment, update, and exclusion-zone states;
  - held FLASH/SWAP press and release behavior;
  - a different current page using the same grid positions; and
  - narrow and wide touch layouts.
- The story's hardware control must not change the grid into physical playback cards. If retained,
  it may only demonstrate app chrome outside the virtual grid.

## Verification

- Storybook test: Virtual Playbacks contains pool-style grid boxes and no vertical fader inputs.
- Storybook test: changing the mock page preserves box positions and changes their assignments.
- Storybook test: empty positions remain visible.
- Storybook interaction test: GO-like actions execute once; FLASH/SWAP-like actions emit matched
  press and release callbacks.
- Storybook state tests cover assigned, empty, unavailable, running, configuration, assignment,
  update, exclusion-member, and exclusion-selected boxes.
- Touch-viewport checks prove usable box geometry and scrolling or fitting at configured sizes.
- Focused live-app tests preserve existing row/column settings, current-page addressing, SET and
  Update behavior, held actions, zones, and persisted surface partitioning.
- Update neither the documented slot limit nor persisted virtual-playback layout compatibility in
  this visual refactor.

## Result

- Kept the functional `VirtualPlaybackGrid` in `apps/light-desktop` and migrated its live rendering
  to the shared `@tosklight/ui` grid/card view. The desktop adapter still owns page-slot lookup,
  Playback/Cuelist/runtime projection, SET configuration and assignment, Update targeting,
  momentary Shift selection, exclusion-zone membership, and zone persistence.
- Preserved configured rows and columns, stable positions across page changes, the 127-slot desk
  limit, assigned/empty/unavailable cells, presentation icon/color/image data, current Cue and
  running feedback, and faderless on-screen behavior.
- Preserved one-shot actions and ordered held FLASH/SWAP press-release behavior, including pointer
  cancel and unmount safety release. The shared view now reports click modifiers to the
  application adapter and exposes deterministic exclusion-zone metadata without owning runtime or
  persistence behavior.
- Added application-owned stories for sparse, running, configuration, assignment, Update,
  exclusion-zone, unavailable, held FLASH/SWAP, page-switching, narrow-touch, and wide-touch
  states. These stories render the production desktop adapter with deterministic in-memory domain
  data and no server, WebSocket, OSC, filesystem, Tauri, or mutable-show dependency.
- Removed only the Virtual Playbacks substitute from `CompleteWindows.stories.tsx` after the
  application stories existed. Storybook interaction checks now target the application story IDs
  for fader absence, visible empty cells, stable page positions, action/held semantics, adapter
  states, unavailable cells, and touch geometry.
- Added focused live-adapter coverage for configured dimensions, Update routing, momentary
  Shift-click selection, and held SWAP, alongside the existing page, SET, FLASH, zone persistence,
  authority replacement, pane partitioning, and slot-limit tests.
- Verification passed: UI-library and desktop typechecks; 22 focused desktop Virtual Playbacks
  tests; four shared-grid tests; deterministic Storybook build; and 15 focused Virtual Playbacks
  browser checks plus two focused state checks after the state-story expansion. A full Storybook
  run reached 100 passing checks before stopping on an unrelated modal-title search overlap owned
  by the separate search/modal correction lane.
