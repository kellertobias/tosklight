# Virtual Playback Card Color Hierarchy

## Status

Completed on 2026-07-26 as the color-hierarchy follow-up to the completed
[`virtual-playbacks-pool-grid-layout.DONE.md`](virtual-playbacks-pool-grid-layout.DONE.md) plan.

## Goal

Use the established pool-card hierarchy for Virtual Playbacks:

- configured color appears as a complete card outline while inactive;
- icon or image sits at the bottom right;
- an active/running playback fills the complete card with its configured color; and
- runtime/workflow labels live at an edge, not in the middle of the playback name and action.

Preserve the accepted Record and Update target presentations and interactions.

## Current evidence

- `VirtualPlaybackBox` maps `running` to the generic `active` pool-card state and also inserts the
  word **Running** into the card's center details.
- `configurationTarget` inserts **Configure Playback** into the same central details list.
- Configured playback color currently produces a full outline plus a faint vertical background
  tint; running adds another inset outline instead of filling the card.
- `PoolCard` already owns bottom-right icon and color-indicator placement, but Virtual Playback
  image backgrounds occupy the complete card.
- Record/assignment and Update are separate application-owned target workflows and callbacks.

## Configured color

1. An assigned inactive playback with a configured color uses that color as a complete outline on
   all four sides.
2. Use one consistent outline width and radius. Do not show only a left, top, or other single
   colored edge.
3. Keep the inactive interior on the normal pool-card background. A configured color alone must
   not look active.
4. Empty and unavailable cells have no configured-color outline.
5. A playback without a configured color uses the normal neutral outline and background.
6. Selected/exclusion/workflow outlines must remain distinguishable from configured playback
   color; do not stack several unexplained outlines at the same inset.

## Active and running presentation

1. When the playback is active/running, fill the complete interior of the grid cell with the
   configured playback color.
2. The fill reaches the card's inner edge rather than appearing as a small center patch or another
   inset outline.
3. Preserve the full configured-color outline so the card retains a crisp boundary.
4. Choose contrast-safe title, action, cue, number, icon, and status colors from the actual
   configured color. Light playback colors must not produce unreadable white text.
5. Running state is still shown textually, but **Running** moves to a compact edge-aligned status
   location. It must not occupy the middle of the card.
6. The runtime status may be a small top/bottom badge or status line, subject to operator review.
   It must not cover the playback name, action, cue, icon, or image.
7. When running stops, remove the full fill immediately and return to the outlined inactive state.
8. Held FLASH/SWAP feedback may use the same full-card active principle only while the action is
   actually held, while remaining distinguishable from an ordinary running cue-list state.

Do not use **Running** as the playback's primary label. The playback name remains primary.

## Icon and image placement

1. Place a configured icon at the bottom right using the shared pool-card icon geometry.
2. Place a configured image/thumbnail at the bottom right in the same semantic artwork area,
   rather than silently replacing the complete card background.
3. Give icon and image variants a stable maximum size that leaves room for the playback name,
   action, cue, and status.
4. Preserve the image's aspect ratio and crop deliberately within its bottom-right frame.
5. If both icon and image exist, define one deterministic priority; do not overlap them.
6. Artwork must not intercept the card's pointer/touch action.
7. Supply accessible alternative text or mark purely decorative artwork appropriately.
8. Active color fill must remain visible around/behind the artwork without destroying image
   legibility.

## Text hierarchy

Use stable, non-overlapping regions:

- slot/page number;
- playback name;
- configured action such as **GO**, **FLASH**, or **SWAP**;
- current cue where applicable;
- compact runtime status at an edge; and
- icon/image at bottom right.

Long text truncates according to an explicit rule. Do not move **Running**, **Configure Playback**,
or another state into the center simply because a title is long.

## Configure Playback

1. Keep configuration targeting obvious but move **Configure Playback** out of the card's central
   content.
2. Use a compact edge-aligned workflow badge/strip or a clearly scoped perimeter treatment.
3. Preserve the configuration callback and exact SET/configuration routing.
4. The configured playback color remains identifiable while configuration is armed.
5. Configuration state must not look like running, held Flash/Swap, Record, or Update.

## Preserve Record and Update

Record and Update are explicitly accepted:

1. Keep their existing operator wording.
2. Keep their existing strong target treatment and clear whole-card hit target.
3. Keep the current application-owned callbacks and command-line routing.
4. Keep Record and Update visually distinct from configured color, running, configuration,
   exclusion selection, and held actions.
5. Do not simplify, rename, move, or remove Record/assignment and Update as part of this visual
   hierarchy correction.
6. Add regression coverage that compares their accepted classes, labels, geometry, and callback
   delivery before and after this change.

If the current story calls the Record target **Assign source** while the live operator path says
**Record**, use the real production wording in the application-owned story rather than inventing a
third label.

## Storybook states

Add or update deterministic application-owned stories for:

- inactive colored playback with complete outline and neutral interior;
- active/running playback with full configured-color fill;
- colorless playback;
- light and dark configured colors with readable text;
- bottom-right icon;
- bottom-right image;
- long playback/action/cue text with artwork;
- edge-aligned Running status;
- edge-aligned Configure Playback state;
- accepted Record target;
- accepted Update target;
- held FLASH and held SWAP;
- exclusion membership/selection; and
- empty and unavailable cells.

Do not combine every exceptional state in one box. Each story must make the state under review
unambiguous.

## Verification

- Package geometry test: configured color produces a four-sided outline, not a one-edge marker.
- Package computed-style test: inactive color leaves the interior neutral.
- Package computed-style test: active/running color fills the complete card interior.
- Contrast test: representative light/dark playback colors retain readable content.
- Geometry test: icon and image occupy the bottom-right artwork area and do not overlap text.
- Geometry test: Running and Configure Playback occupy edge-aligned regions and never the central
  title/action area.
- Interaction test: ordinary action, held FLASH/SWAP, Configure, Record, Update, and exclusion
  callbacks retain their current semantics.
- Regression test: Record and Update retain their accepted labels, target geometry, and visual
  priority.
- Storybook test: running transition changes outline-only → full fill → outline-only without
  network traffic.
- Application adapter tests retain current-page addressing, stable slot positions, configuration,
  assignment/Record, Update, and held-action behavior.

## Acceptance

- Inactive colored playback: full colored outline, neutral interior.
- Active/running playback: full colored interior.
- Icon or image: bottom right.
- Running: visible, but not in the middle.
- Configure Playback: visible, but not in the middle.
- Record and Update remain exactly available and prominent.
- No state is communicated by an unexplained single colored side.

## Non-goals

- Do not change the configured rows, columns, slot limit, or page addressing.
- Do not add faders to Virtual Playbacks.
- Do not change Record, Update, SET/configuration, FLASH, SWAP, or exclusion semantics.
- Do not reopen the completed pool-grid migration.

## Result

Implemented the hierarchy in the shared `VirtualPlaybackGridView` and the production
application adapter:

- inactive configured playbacks retain the neutral pool-card interior with a complete
  configured-color outline;
- running and held actions fill the complete card with the configured color and select
  contrast-safe content colors;
- Running, Configure Playback, Record, Update, and exclusion feedback now occupy compact
  edge regions instead of the central label/action/cue area;
- icons and images share a bounded bottom-right artwork region, with images taking
  deterministic priority and carrying alternative text; and
- the production playback-color stylesheet now excludes Virtual Playback cards from the
  fader-card tint/gradient rules, so those rules cannot override this hierarchy.

Application-owned stories cover inactive, running transition, light/dark/colorless,
icon/image, long-label, configuration, Record, Update, held action, exclusion, empty, and
unavailable states. Package tests cover state rendering and callback preservation, while
the Storybook browser check verifies outline/fill transitions, edge status, and artwork
geometry without network traffic.

Focused verification passed:

- `npx biome check apps/ui-library/src/pools/PoolCard.tsx apps/ui-library/src/playback/VirtualPlaybackGrid.tsx apps/ui-library/src/playback/VirtualPlaybackGrid.test.tsx apps/light-desktop/src/components/control/virtualPlayback/VirtualPlaybackGrid.stories.tsx`
- `npm test --workspace @tosklight/ui -- --run src/playback/VirtualPlaybackGrid.test.tsx src/pools/PoolCard.test.tsx`
- `npm test --workspace @tosklight/light-desktop -- --run src/windows/VirtualPlaybacksWindow.test.tsx`
- both UI-library and desktop TypeScript checks
- `npm run test:architecture`
- the focused Storybook Playwright hierarchy scenario
- `npm run storybook:build --workspace @tosklight/ui`
- `git diff --check`

The complete Storybook-change batch passed all 209 integrated browser checks. Browser review
confirmed neutral inactive cards, full-color active cards, contrast-safe text, compact edge
status, and deterministic icon/image placement.
