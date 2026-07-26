# Single Table Row Separators

## Status

Done.

## Defect

Populated rows in the shared `DataTable` appear to have two horizontal separators.

The duplicate is caused by two independent separator systems in
`apps/ui-library/src/styles/window-kit.css` after the workspace move:

1. `.ui-data-table` paints a repeating 42/43-pixel horizontal line background.
2. Every `.ui-data-table-row` also paints `border-bottom: 1px solid var(--line)`.

Because populated rows are transparent, both lines remain visible at slightly different vertical
positions. Empty rows can conceal one of them with their background, which makes the defect appear
content-dependent.

## Required correction

1. Every boundary between adjacent table rows has exactly one one-pixel separator.
2. Use one separator owner. Prefer the row `border-bottom` because DataTable already renders
   explicit filler rows for stable empty geometry.
3. Remove the repeating separator gradient from `.ui-data-table`; retain only the intended table
   background color.
4. Preserve the sticky header, current row heights, filler rows, selection backgrounds, active-row
   outline, Fixture Sheet step-state styling, and column geometry.
5. Do not add compensating cell borders or feature-specific overrides.
6. Apply the correction to the shared generic table so Fixture Sheet and every future table inherit
   the same behavior.

## Verification

- Generic populated-table story shows one separator between every two adjacent rows.
- Generic empty/filler rows use the same single separator rhythm.
- Fixture Sheet rows show one separator in normal, selected, active, nested, and step states.
- Header-to-first-row transition has one separator.
- A focused visual check at native scale and a high-density screenshot show no adjacent one-pixel
  lines or two-pixel band.
- Shared table interaction and Fixture Sheet tests remain green.
- No feature-specific table stylesheet reintroduces a second horizontal separator.

## Result

- The shared DataTable now uses its solid table background plus the existing one-pixel row
  `border-bottom`; the repeating 42/43-pixel separator gradient is removed.
- The desktop compatibility stylesheet no longer restores the removed gradient later in the
  application cascade.
- Package and desktop compatibility stylesheet tests protect both entry points: the table cannot
  paint a horizontal gradient or table border, each row owns exactly one one-pixel separator, and
  cells do not add another horizontal border.
- Generic populated and filler rows, the sticky header transition, and Fixture Sheet normal,
  selected, active, nested-head, and step-state rows retain their existing geometry and state
  styling.

Verification completed:

- `npm test --workspace @tosklight/ui -- --run src/window-kit/DataTableStyles.test.ts src/tables/FixtureSheetTable.test.tsx`
  — 2 files and 3 tests passed.
- `npm test --workspace apps/light-desktop -- --run src/window-kitStyles.test.ts`
  — 1 file and 1 test passed.
- `npm run typecheck --workspace @tosklight/ui` — passed after the concurrent typed-search
  migration was completed.
- `npm run storybook:build --workspace @tosklight/ui` — passed.
- The rebuilt Generic Table story was inspected in Chromium at device scale factors 1 and 2:
  the table computed `background-image: none`, had no top or bottom border, and its 40-pixel header
  and 43-pixel populated/filler rows each computed one one-pixel bottom border.
- The rebuilt Fixture Sheet step-selection story was inspected at device scale factor 2. Normal,
  selected, active, nested-head, and step rows each retained one one-pixel bottom border without a
  table separator gradient or adjacent two-pixel band.
- The integrated Storybook browser suite now repeats the Generic Table and Fixture Sheet checks
  at device scale factors 1 and 2. It verifies the rendered table owns no gradient or outer
  horizontal border, every row owns exactly one one-CSS-pixel bottom border, cells own no
  horizontal border, adjacent row bounds meet without a gap or overlap, and both rendered table
  captures are nonblank.
