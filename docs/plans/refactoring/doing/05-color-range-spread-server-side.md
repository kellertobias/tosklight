# 05 — Color-range spread: move the shift-drag interpolation server-side

## Context (api-rules §4 violation, verified 2026-07-23)

`apps/control-ui/src/components/modals/specialDialogs/color.tsx:123-126` — on a shift-drag
color-range gesture the client calls
`interpolatePickerRange(selectedFixtureIds.length, gesture.start, end)` and applies
**per-fixture interpolated colors**. The math lives in
`apps/control-ui/src/components/modals/specialColor.ts:28-46` (`interpolatePickerRange`);
`colorProgrammerAssignments` (`specialColor.ts:49-59`) then resolves RGB/CMY per ordered
fixture/head and writes through `normalizedFixtureMutations` to the same
`programmer-values/actions` endpoint as chunk 03.

Note: the §5 feature list mentions "COLOR-RANGE-001: shift-drag Color range apply" as a
deferred feature — the gesture itself already exists in this modal; what remains deferred
is a separate scenario surface. This chunk only relocates the existing computation; it
does not build new UI.

## Work

1. Extend the v2 programmer-values contract using **chunk 03's shared fan-out vocabulary**
   (this is the same family: two color endpoints are the control points) so the client
   sends the ordered selection + the two endpoint colors, and the server interpolates and
   resolves per-fixture color-channel values (scalar resolution via 03b's shared rule
   where `AttributeValue::Spread` is stored).
2. **Wraparound and multi-revolution hue interpolation (maintainer 2026-07-23):** the
   interpolation must run smoothly through a hue-aware color space and support going the
   long way around the wheel and even **multiple full revolutions** across the selection
   (e.g. red → red via one or more complete rainbow cycles). Concretely: the wire payload
   carries winding information from the gesture (signed hue travel / revolution count),
   not just two endpoint colors — two endpoints alone cannot express direction or turns.
   The gesture layer derives the winding from the drag (display/interaction concern, stays
   client-side per §4); the server interpolates hue with wraparound. Pin with a test:
   3 fixtures, red → red with one revolution → hues at 0°, 120°, 240°.
3. The color→channel resolution in `colorProgrammerAssignments` (which fixture has RGB vs
   CMY heads) is show logic — move it server-side with the interpolation. The UI keeps only
   the gesture (start/end picker positions + winding) and display.
4. Delete `interpolatePickerRange` client-side once unused; keep any purely visual gradient
   preview if one exists (display logic may stay).

## Definition of done

- Shift-drag color range sends one request (selection + endpoints + winding); no
  per-fixture color computed client-side before the write.
- Server tests: 3-fixture ordered selection, endpoints red→blue, assert per-fixture
  interpolated channel values incl. a CMY fixture in the middle; plus the wraparound case
  (red→red, one revolution → 0°/120°/240°).

## Verification

```sh
cargo test -p server -p application
npm run test:unit
npm run test:e2e   # full suite gate
```

Manual: `npm run open`, select 3+ color fixtures, shift-drag in the color dialog, confirm
the visual gradient across fixtures matches pre-change behavior.

## Decisions

None. Depends on chunk 03 landing first if it reuses the extended spread mutation shape
(sequence 03 → 03b → 05); if the color interpolation ends up storing
`AttributeValue::Spread`, its ordered resolution must use 03b's shared anchor rule
(Next/50 requirement).

## Decision addendum (maintainer, 2026-07-24)

Scope clarified while executing: **all attribute spreads entered via encoders must be
computed server-side, never client-side** — not only the color range. Audit every
encoder-entered write path for remaining client-side per-fixture computation and either
fix it in this chunk (if it is spread math) or file it as a follow-up chunk.
