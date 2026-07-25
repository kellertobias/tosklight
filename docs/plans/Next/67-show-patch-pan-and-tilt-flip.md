# Show Patch Pan and Tilt Flip

## Status

**Specification only.** This plan records a future per-instance Show Patch transform. It does not implement patch UI, output mapping, Stage behavior, show persistence, migration, help changes, or executable tests.

## Goal

Add **Flip Pan** and **Flip Tilt** columns to **Show > Show Patch** so an operator can reverse the movement direction of one patched physical fixture without editing its transferable fixture profile or changing stored programmer and Cue values.

For a flipped axis, the normalized physical result is reversed:

```text
physical = 1 - programmed
```

Therefore `0%` produces the axis's highest physical value, `100%` produces its lowest physical value, and `50%` remains the midpoint.

## Operator workflow

Each physical fixture row exposes independent **Flip Pan** and **Flip Tilt** values. The same values are available on a multi-patch instance because two physical units sharing logical programming may be hung in opposite orientations.

An ordinary click continues to select the fixture or instance without changing show data. `[SET]` followed by the cell opens the normal patch-cell editor. The editor clearly shows **Normal** or **Flipped**, applies only the chosen axis, and saves as one mutation and one Undo step.

Fixtures without the applicable axis show an unavailable value rather than accepting a meaningless setting. Unpatched fixtures retain their configured flips so the behavior resumes when they are patched again.

## Resolution and profile interaction

Pan/Tilt flip is a show-patch property of the physical instance. It is separate from:

- the fixture profile's channel-level raw-DMX inversion;
- the fixture's physical Pan/Tilt range;
- 3D mounting rotation;
- programmer values, Presets, Cues, and tracking; and
- encoder direction preferences.

The engine applies the patch flip to the normalized Pan or Tilt request and then uses the profile's authored range, resolution, fine-byte layout, and raw inversion to encode the result. A profile-authored raw inversion and one enabled patch flip therefore reverse the direction twice and cancel physically; neither inversion may be accidentally skipped or applied more than once.

Every projection of that physical instance must agree with its output. Normal output, Preload, Cue transitions, Move in Black, Highlight if Position is ever included, DMX inspection, and 2D/3D Stage motion all use the same resolved flipped axis.

## Ownership and compatibility

The two flags belong to the portable show's physical patch data. They default to `false` for existing shows. Copying a physical fixture or multi-patch instance copies its flip flags; changing universe or address does not.

Changing the fixture profile or mode preserves a flip when the replacement still provides that axis and retains it as dormant compatible data when the axis is temporarily absent. It must not convert the patch flag into a fixture-library revision.

## Acceptance coverage

1. Show Patch exposes independent Flip Pan and Flip Tilt cells for each applicable physical fixture and multi-patch instance.
2. Normal maps `0%` to the low endpoint and `100%` to the high endpoint; Flipped maps `0%` to the high endpoint and `100%` to the low endpoint.
3. The midpoint remains unchanged and 8-, 16-, 24-, and 32-bit channel encoding preserves the reversal without coarse/fine corruption.
4. Flipping one axis does not change the other axis or any stored programmer, Preset, Cue, or tracking value.
5. Profile raw inversion and patch flipping compose exactly once.
6. Normal output, Preload, transitions, Move in Black, DMX inspection, and Stage visualization agree.
7. Multi-patch instances can use different flips while retaining one shared logical programmer value.
8. Fixtures without the relevant attribute cannot be given an effective flip.
9. Existing shows migrate with both flags off, and unpatched fixtures retain their settings.
