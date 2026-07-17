# Color Special Dialog Alignment

## Status and scope

Add range alignment to the Color special dialog while preserving its existing uniform-color gesture.

## Operator behavior

A normal click or tap applies one chosen color uniformly to the current selection. While Shift is held on either the normal keyboard or connected hardware, pressing or touching a start color and dragging to an end color defines a color range. On pointer or touch release, apply the range immediately to the current ordered selection.

The first selected fixture receives the start color and the last receives the end color. Every fixture between them receives an equally spaced interpolation in selection order. A one-fixture selection receives the chosen endpoint without division errors. The interaction must retain pointer capture outside the picker and apply exactly once on release or cancel safely if the gesture is aborted.

Interpolate along the straight drag line in the visible picker: hue and saturation use equal linear steps between the pointer-down and pointer-release coordinates, and every point uses the Brightness value shown by the dialog. Do not substitute the shortest circular hue path; the visible horizontal drag direction is authoritative. Convert each interpolated picker value through the normal color resolver for that fixture. The implementation must use the authoritative ordered selection rather than patch order or fixture ID order. The resulting values are ordinary programmer Color values and retain normal fade, undo, Blind, Preview, Preload, Record, and Update behavior.

## Acceptance criteria

1. Normal click/tap still applies one color uniformly.
2. Shift-drag from either software keyboard Shift or attached-hardware Shift applies both endpoints and equal intermediate values in current selection order.
3. Reversing selection order reverses the applied range without changing its endpoints.
4. One-, two-, mixed-capability, and logical-head selections behave deterministically, with unsupported Color attributes skipped safely.
5. Pointer/touch release applies once; cancellation or lost capture cannot leave a half-applied range.
6. The dialog visibly previews the start, end, and active range gesture without obscuring the normal picker.
