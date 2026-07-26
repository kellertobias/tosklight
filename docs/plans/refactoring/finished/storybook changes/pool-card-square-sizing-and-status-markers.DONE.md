# Pool Card Square Sizing and Status Markers

## Status

Completed on 2026-07-26 as the verified geometry and status-marker follow-up to
[`generic-pool-window.DONE.md`](generic-pool-window.DONE.md).

## Goal

Restore genuinely square cards in the Generic Pool window and every adopted Group, Preset, and
Cuelist pool. Make card height follow the measured grid-cell width, fit the pool viewport
correctly, and scroll by complete square rows.

Also remove unexplained one-edge-only color markers. Configured object color should use a complete
outline; special states such as Derived or Frozen need an explicit readable marker if retained.

## Current evidence

- Package `PoolCard` declares `aspect-ratio: 1`.
- Package `ButtonGrid` measures the first rendered cell width and writes it to
  `--grid-row-size`.
- Several legacy application styles still force pool cards to `142px × 94px`, set 94 px grid rows,
  or apply minimum dimensions of `142px × 94px`.
- Those broader application selectors can override the shared package grid and recreate the
  rectangular-card regression.
- Package `.pool-card.derived` uses only a blue top border.
- Package `.pool-card.frozen` uses only an amber top border.
- A single colored side does not explain either state and can be mistaken for an incomplete
  configured-color outline.

## Square geometry

1. Every normal pool card has equal computed width and height within one device pixel.
2. Treat the measured grid column width as authoritative for the row height.
3. Do not use fixed `142×94`, `132×88`, `94px` row, or other rectangular legacy dimensions in a
   Generic Pool, Group Pool, Preset Pool, or Cuelist Pool grid.
4. `aspect-ratio: 1` is not sufficient by itself if a fixed height/min-height overrides it.
   Remove or scope every conflicting rule along the adopted pool path.
5. Set both the grid auto-row size and card block size from the measured cell width so browser
   grid stretching cannot distort the cards.
6. Preserve the configured minimum touch width, but never satisfy it by stretching only one axis.
7. Recalculate after window resize, pane resize/maximize, settings-panel changes, font readiness,
   scrollbar appearance, and responsive column-count changes.
8. Avoid resize loops: update the row-size variable only when the measured width materially
   changes.

## Viewport height and scrolling

1. The window header and settings chrome consume their own height; the pool scroll area receives
   the remaining height.
2. Pool content rows keep their square size and align from the top. Do not stretch a small number
   of rows vertically to fill unused space.
3. When rows exceed the available height, the pool scroll area owns vertical scrolling.
4. Cards must not overflow behind the window header, footer, settings, or neighboring pane.
5. Horizontal scrolling must not appear at supported widths unless the configured minimum card
   size genuinely cannot fit one column.
6. The final row may be partially visible during scrolling, but its cards remain square.
7. Empty 200-slot pools and populated pools use identical geometry.
8. Resizing narrower or wider changes the column count and square size deterministically without
   retaining a stale height from the previous layout.

## CSS ownership and cleanup

1. Make the package `ButtonGrid`/`PoolGrid` sizing contract authoritative.
2. Remove or narrowly scope legacy application rules that set fixed pool-card width, height,
   min-width, min-height, grid-template columns, or grid-auto rows.
3. Do not rely on selector specificity fights or `!important` to preserve square geometry.
4. Group shortcuts and other deliberately compact non-pool surfaces may keep their own dimensions,
   but their selectors must not match full pool windows.
5. Group, Preset, Cuelist, and Generic Pool adapters must consume the same square sizing contract.

## Configured color and one-edge markers

1. When a pool object has a configured color, show that color as a complete outline on all four
   sides.
2. Do not use only the left, right, top, or bottom edge to mean configured color.
3. Keep icon/image artwork in its defined corner area rather than substituting a colored edge.
4. Selected, active, Record/store, Update, and Set target states must remain distinguishable from
   the configured-color outline.

The current one-side markers mean:

- blue top edge: **Derived**; and
- amber top edge: **Frozen**.

Those meanings are not self-evident. If Derived and Frozen remain required:

- replace the lone colored top border with an explicit icon, badge, or short text label;
- provide an accessible state name;
- keep the configured object's full outline visible independently; and
- document the state in the corresponding story.

If a pool type cannot actually produce Derived or Frozen, omit that state from its story instead
of showing an unexplained decorative edge.

## Card content

Square restoration must retain:

- stable object/slot number;
- primary and secondary labels;
- optional icon or image;
- configured color;
- selected/active state;
- Record/store, Update, and Set targets;
- empty and disabled state; and
- click and press-hold behavior.

Text must truncate within the square; it must not increase card height.

## Storybook states

Add focused Generic Pool stories for:

- empty 200-slot pool;
- sparse pool;
- narrow/tall viewport;
- narrow/short viewport;
- wide/tall viewport;
- wide/short viewport;
- live resize between those sizes;
- configured-color cards with complete outlines;
- Derived and Frozen with explicit markers;
- Record, Update, Set, selected, and active states; and
- 240+ slots with vertical scrolling.

Repeat square-geometry checks against application-owned Group, Preset, and Cuelist pool stories so
package-only success cannot hide an application CSS override.

## Verification

- Package geometry test: every rendered pool card has equal width and height within one device
  pixel.
- Package resize test: changing container width recalculates columns and row height without stale
  dimensions.
- Package height test: changing only viewport height does not distort card aspect ratio.
- CSS audit/test: no full pool-window path is matched by legacy `142×94`, `132×88`, or 94 px row
  rules.
- Storybook geometry test: Generic empty, sparse, narrow, wide, short, and tall stories remain
  square and scroll vertically.
- Application Storybook test: Group, Preset, and Cuelist cards remain square after all desktop
  styles load.
- Storybook test: no unexplained one-edge-only color marker remains.
- Accessibility test: Derived/Frozen and configured color are distinguishable without relying only
  on color.
- Interaction regression: click, press-hold, Record, Update, Set, selected, and active states
  retain their existing callbacks.
- Focused live-app geometry check reproduces pane resize/maximize and confirms the same square
  behavior as Storybook.

## Acceptance

- Generic Pool cards are square again.
- Card height always matches current card width.
- Pool cards fit the available window height through correct scrolling, not rectangular
  compression.
- Group, Preset, and Cuelist adoption cannot override the package geometry.
- Configured color uses a complete outline.
- Derived/Frozen use explicit markers or are omitted where inapplicable.
- No unexplained single colorful side remains.

## Non-goals

- Do not reduce the 200-slot minimum.
- Do not change stable slot identities, numbering, ordering, or pool callbacks.
- Do not resize group shortcuts or other intentionally non-pool card strips unless their selector
  leaks into a full pool.
- Do not reopen or edit the actively owned Generic Pool plan.

## Result

Implemented shared measured square geometry for the full Generic, Group, Preset, and Cuelist
pool paths:

- `ButtonGrid` measures the rendered column width and applies it as the row and child block size,
  remeasuring through resize, child replacement, and font readiness with a 0.25-pixel loop guard;
- compact consumers can opt out explicitly, and the Stage Group strip retains its accepted
  rectangular compact cards;
- legacy desktop rules that forced 142×94, 132×88, or fixed 94-pixel pool rows were removed or
  scoped away from full pool surfaces;
- configured color is a complete four-edge card outline, while store/update/set selection keeps a
  separate inset/perimeter treatment; and
- Derived and Frozen are explicit visible, accessible badges rather than unexplained single-edge
  color markers.

Generic and application-owned stories cover narrow/short, narrow/tall, wide/short, wide/tall,
live resize, sparse/empty, and 260-slot overflow states for the shared and live Group, Preset, and
Cuelist surfaces. Unit coverage exercises remeasurement, compact opt-out, status badges, and
configured-color ownership. Storybook browser coverage verifies square geometry, vertical
scrolling, live resize, CSS ownership, and accessible markers across the production adapters.

Focused verification passed:

- both UI-library and desktop TypeScript checks;
- 22 focused UI-library and desktop unit tests;
- the production Storybook build;
- three focused Storybook geometry/scroll/resize browser scenarios; and
- repository architecture and diff checks.

The narrow/short and wide/tall production Group stories were also reviewed visually. The root
integrated Storybook and screenshot gates passed all 217 Playwright checks.
