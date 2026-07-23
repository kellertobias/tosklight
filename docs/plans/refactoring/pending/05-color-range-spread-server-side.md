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

1. Extend the v2 programmer-values contract (or reuse the `Spread` value kind from chunk 03)
   so the client sends the ordered selection + the two endpoint colors, and the server
   interpolates and resolves per-fixture color-channel values.
2. The color→channel resolution in `colorProgrammerAssignments` (which fixture has RGB vs
   CMY heads) is show logic — move it server-side with the interpolation. The UI keeps only
   the gesture (start/end picker positions) and display.
3. Delete `interpolatePickerRange` client-side once unused; keep any purely visual gradient
   preview if one exists (display logic may stay).

## Definition of done

- Shift-drag color range sends one request (selection + endpoints); no per-fixture color
  computed client-side before the write.
- Server test: 3-fixture ordered selection, endpoints red→blue, assert per-fixture
  interpolated channel values incl. a CMY fixture in the middle.

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
(sequence 03 → 05); otherwise independent.
