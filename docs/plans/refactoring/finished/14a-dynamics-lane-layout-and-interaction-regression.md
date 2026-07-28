# Dynamics Lane Layout and Interaction Regression

## Queue position and status

**Completed before Dedicated Virtual Playbacks.**

Move this file to `doing/` before changing implementation code, then follow the state,
verification, Result, finished-plan, and semantic-commit workflow in
[`../README.md`](../README.md). This document explains the required fix only; its
presence does not implement or verify the repair.

This is a regression repair for the completed
[Dynamics implementation](../finished/16-dynamics/README.md), not a reopening of that
finished plan. Keep this focused plan as the authoritative record for the repair.

## Problem

The Curves workspace still gives each lane row the available horizontal width, but the
lane identity and curve preview no longer lay themselves out across that width. The
curve is compressed to roughly its content minimum and centered beside the identity,
leaving most of the lane's apparent width unused.

At a 1280×720 Storybook viewport, the current production Dynamics story measured:

- Dynamic lane list: 1,126 px wide;
- lane row: approximately 1,113 px wide;
- lane selection surface: approximately 1,003 px wide; and
- actual curve preview: approximately 190 px wide.

The row therefore owns the expected width while the operator-visible curve uses only a
small fraction of it. This is a CSS/markup regression, not a Dynamic definition,
runtime, phase, or lane-width-value problem.

## Cause

Commit `230fb3d8` (`fix(dynamics): use shared controls throughout editor`) changed the
whole lane selection surface in
`apps/light-desktop/src/windows/DynamicsWindow.tsx` from a native button to the shared
`Button`.

The lane surface is intentionally a two-column CSS grid:

```css
.dynamic-lane-select-surface {
	display: grid;
	grid-template-columns: 7.4rem minmax(12rem, 1fr);
}
```

The shared control also adds `.ui-button`, whose general contract uses
`display: inline-flex`, centered content, an 8 px gap, shared padding, and a 50 px
minimum height. In the current rendered cascade, the shared rule wins for `display`.
The lane surface itself stretches as a grid child, but its identity and curve become
centered flex items at their content widths. The curve's internal `width: 100%` then
fills only that compressed flex item, not the available lane width.

The same markup also contains keyframe buttons inside the whole-lane button. The
browser reports invalid nested `<button>` elements. This invalid interactive hierarchy
existed in native-button form before the shared-control conversion and must be removed
as part of the repair rather than preserved behind a stronger CSS override.

## Required fix

Rebuild the lane row so layout ownership and interaction ownership are explicit:

1. Keep `.dynamic-lane-overview` as the full-width row with the lane content and the
   trailing Lane action menu as separate grid columns.
2. Introduce a dedicated full-width lane-content grid for the fixed identity column and
   the flexible curve column. This grid must not itself receive the generic shared
   button layout contract.
3. Keep lane selection reachable from both the identity area and the non-keyframe
   curve background. Use real buttons for these selection targets, but make them
   siblings of keyframe buttons rather than ancestors.
4. Keep draggable keyframe controls as independent shared buttons layered over the
   curve. No button, link, or element with button semantics may contain another
   interactive control.
5. Preserve normal click selection, Shift-click additive selection, the software
   Shift latch, primary-lane state, keyframe selection and dragging, the Preview
   playhead, the repeat boundary, and the Lane action menu.
6. Preserve the current 7.4 rem identity column, trailing action width, lane height,
   curve axes, curve drawing, selected/primary borders, and touch-friendly controls.
7. Do not change Dynamic lane data, the operator-facing Curve width parameter,
   keyframe timing, phase spread, speed, persistence, API contracts, runtime behavior,
   or output.

A selector-specific `display: grid !important` patch on the shared `Button` is not a
complete fix. It may restore width temporarily but would retain invalid nested
interactive elements and leave the lane layout vulnerable to generic button styling.

## Implementation boundaries

The expected implementation is limited primarily to:

- `apps/light-desktop/src/windows/DynamicsWindow.tsx`;
- `apps/light-desktop/src/windows/DynamicsWindow.css`;
- `apps/light-desktop/src/windows/DynamicsWindow.test.tsx`; and
- focused production Storybook/browser acceptance for
  `ToskLight/Windows/Dynamics`.

Change the shared `Button` contract only if a repository-wide control defect is proven.
Do not weaken or special-case shared controls globally to accommodate a composite
Dynamics row.

Do not modify Rust Dynamics domain/runtime code, generated wire contracts, show data,
help behavior, or screenshots unless the focused frontend repair proves that one of
those surfaces is genuinely affected.

## Acceptance criteria

### Geometry

- Every lane row fills the lane list's available content width.
- The lane content fills the row width left after the trailing Lane action menu.
- The curve column consumes all space left after the 7.4 rem identity column; it is
  not centered at a content-minimum width.
- At the 1280×720 production Storybook viewport, the curve width equals the lane
  content width minus the identity column within normal border/padding tolerance.
- The same relationship holds at the Full HD documentation viewport and at the
  supported compact breakpoint without horizontal clipping of the Lane action menu.
- Multiple lanes have identical identity, curve-axis, and trailing-action alignment.

### Interaction and semantics

- Clicking a lane identity selects that lane.
- Clicking unused curve background selects that lane.
- Shift-click and the software Shift latch add or remove lanes exactly as before.
- Clicking or dragging a keyframe does not trigger the curve-background selection
  control accidentally and retains pointer capture for the physical drag path.
- Lane action choices still change the attribute or delete the lane without selecting
  or dragging a keyframe.
- The rendered Dynamics lane list contains no nested interactive controls and
  specifically no `button button` descendants.
- Each selection and keyframe control has an unambiguous accessible name and visible
  focus state.

### Regression boundary

- Curves, repeat visualization, Curve width preview, Preview playback, Phase Spread,
  Speed, encoders, mutation writes, and runtime status remain behaviorally unchanged.
- Software-only and hardware-connected layouts preserve the same Dynamics editor
  geometry in their shared workspace.
- No generic shared-control behavior changes outside Dynamics.

## Verification

Start with the smallest focused checks:

1. Extend `DynamicsWindow.test.tsx` to cover normal and additive lane selection,
   keyframe interaction isolation, and the absence of nested buttons.
2. Run the focused Dynamics window unit test.
3. Render the production `FullApplicationDiscussion` Dynamics story at 1280×720 and
   the repository's Full HD documentation viewport. Record bounding boxes for the
   lane list, row, content grid, identity, curve, and action menu, and assert the
   relationships in the Geometry criteria.
4. Exercise the real pointer path for selecting a lane, Shift-selecting another lane,
   and dragging a non-initial keyframe.
5. Inspect both viewport renders visually for full-width curves, aligned axes,
   selection state, repeat markers, and action-menu placement.

Then run proportionate broader checks:

```sh
npm run test:unit
npm run storybook:build
npm run test:storybook
```

Run the focused root UI acceptance or `npm run open` if the implementation changes
production composition beyond the isolated lane markup/CSS. If `npm run open` is used,
verify readiness and inspect `.artifacts/runtime/light-data/light-headless.log` before
claiming runtime success.

Do not refresh the checked-in Dynamics help screenshot until the production geometry
has received explicit visual acceptance. Static source inspection, jsdom unit tests,
or a successful Storybook build alone are not rendered-geometry proof.

## Completion

Before moving this plan to `finished/`:

1. compare the implementation with every acceptance criterion above;
2. record exact focused and broader verification results;
3. record any unavailable desktop or hardware evidence honestly;
4. add a `## Result` section with the implementation summary, tests, limitations, and
   semantic commit; and
5. move this file with the implementation in a focused semantic-release commit.

## Result

### Implementation

- Replaced the whole-lane shared button with a non-interactive `.dynamic-lane-content`
  grid that retains the 7.4 rem identity column and gives the curve the complete
  remaining width before the Lane action column.
- Added separate, accessible native selection buttons for the lane identity and unused
  curve background. Keyframe controls remain independent shared buttons, so no
  interactive element contains another interactive element.
- Preserved normal selection, physical Shift selection, the one-shot software Shift
  latch, primary-lane and keyframe state, pointer capture and dragging, repeat and
  Preview overlays, and Lane actions without changing Dynamic data or runtime
  contracts.
- Kept a 50 px keyframe hit area while drawing the visible keyframe as a circular
  marker, avoiding the shared button minimum-height conflict without changing the
  shared control contract.
- Added a production Storybook regression that measures all three lane rows at
  1280×720, 1920×1080, and the 900×720 compact breakpoint in software mode, plus
  1280×720 and 1920×1080 in hardware-connected mode. It also exercises physical
  Shift selection and a real mouse drag of keyframe B.

### Verification

- `npm test --workspace @tosklight/light-desktop -- --run
  src/windows/DynamicsWindow.test.tsx` — 15 passed.
- `npm run typecheck --workspace @tosklight/light-desktop` — passed.
- `npm run storybook:build` — passed.
- Focused production Storybook Playwright regression — passed. At 1280×720 the
  measured list was 1,126 px, row 1,113.4 px, content 1,002.6 px, identity 103.6 px,
  curve 899.0 px, and Lane action 108.8 px. All five viewport/mode combinations
  retained identical lane alignment, no action clipping, and zero `button button`
  descendants.
- Visual evidence and geometry were reviewed under
  `.artifacts/test/visual-inspection/dynamics-lane-regression/`; full-width curves,
  circular keyframes, axes, repeat marker, selection border, and Lane action placement
  were correct in 1280 software, compact software, and Full HD hardware renders.
- The complete Storybook surface was accounted for: 243 of 248 tests passed, including
  this regression and all cases initially skipped by serial-mode failures.
- `git diff --check` for the plan-owned source, CSS, unit, and Storybook files —
  passed.
- Tosken Raider large-window checkpoints stayed at 86%, above the strict 80% Dynamics
  completion threshold.

### Limitations

- `npm run test:unit` stops in the concurrently edited Stage semantic catalog because
  `stage.selectFixture` and `stage.expectCanvasCapture` currently produce
  `unknown-narration`. No Dynamics lane file participates in that failure.
- The full `npm run test:storybook` command is not green because five independently
  reproduced, pre-existing or concurrent expectations remain outside this plan:
  reviewed help screenshots, reviewed marketing screenshots, the Command Section
  Dynamics-tab post-click assertion, the encoder modal's stale `THRU` assertion, and
  the Cuelist pool's stale 150 px minimum assertion. The remaining 243 tests pass.
- The checked-in help and marketing screenshots were not refreshed. Their candidates
  include this reviewed geometry change alongside unrelated concurrent UI changes;
  bulk acceptance would cross plan ownership.
- No packaged desktop or attached-hardware run was required because the repair is
  isolated to production lane markup/CSS and the same production editor composition
  passed in both software and hardware Storybook layouts.

### Commit

`fix(dynamics): restore full-width lane interactions` (this implementation and plan
move).
