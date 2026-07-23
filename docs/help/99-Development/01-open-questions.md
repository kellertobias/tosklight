# Open Questions

## Effect Engine

Our effect engine will be called "Dynamics". We want it to be similar to MA's Phasers and likely will use the 2d stage view as the grid for effects where a grid is required.

You can add lamps to dynamics and use presets as start/ stop values/ steps and choose how they are modulated.

On top of that, Dynamics is still in planning

## Value Spreading Across an Even Selection (resolved)

Resolved by the deterministic anchor rule (see `docs/plans/Next/50-deterministic-multi-point-value-spreading.minor.md`, implemented in `light_core::resolve_spread`): every explicit control point lands on a real selected item. The first and last points anchor the first and last items; an interior point whose ideal ordered position is an integer anchors that item exactly; an ideal position exactly halfway between two items anchors **both** adjacent items; any other position anchors the nearest item. Items between anchors interpolate in equal steps.

Normative examples for `100 THRU 0 THRU 100`: 4 fixtures → `100, 0, 0, 100`; 5 → `100, 50, 0, 50, 100`; 6 → `100, 50, 0, 0, 50, 100`; 10 → `100, 75, 50, 25, 0, 0, 25, 50, 75, 100`. A spread with more control points than selected items is rejected with a visible error and no partial mutation.

Color and position spreading need additional decisions before their test cases can contain commands: the reference syntax for complex values, the interpolation space, color hue-path selection, pan/tilt wrap behavior, and whether fixture calibration is applied before or after interpolation.
