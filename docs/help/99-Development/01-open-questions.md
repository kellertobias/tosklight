# Open Questions

## Effect Engine

Our effect engine will be called "Dynamics". We want it to be similar to MA's Phasers and likely will use the 2d stage view as the grid for effects where a grid is required.

You can add lamps to dynamics and use presets as start/ stop values/ steps and choose how they are modulated.

On top of that, Dynamics is still in planning

## Value Spreading Across an Even Selection

When a command supplies more than two spread control points, how should an interior control point map onto an ordered selection that has no single middle fixture?

For example, Group 1 contains ten fixtures in order and the operator enters `[GRP] [1] [AT] [1] [0] [0] [THRU] [0] [THRU] [1] [0] [0] [ENTER]`. The outside fixtures clearly receive `100%`, and the spread must be symmetric with equal value intervals on both sides. The unresolved choice is the center:

- Should the two middle fixtures both receive the interior control point exactly, producing two fixtures at `0%`?
- Should the interior `0%` control point sit at an imaginary position between the two middle fixtures, leaving both real middle fixtures at the same nonzero interpolated value?

The same decision must cover every even target count and every number of interior control points. It should define how fixtures are apportioned to segments, whether segment endpoints may be duplicated, and how rounding is handled. Until this is decided, `PROG-002` asserts endpoints, symmetry, monotonic direction, equal spacing on each side, and equal center-pair values, but deliberately does not assert the center value.

The smallest concrete example is a four-fixture selection with `100 THRU 0 THRU 100`: should its values be `100, 0, 0, 100`, or should the nonexistent midpoint be treated as `0` and the two real inner fixtures receive interpolated values such as `50, 50`? The final rule must state the exact expected sequence rather than relying on the visual idea of a midpoint.

Color and position spreading need additional decisions before their test cases can contain commands: the reference syntax for complex values, the interpolation space, color hue-path selection, pan/tilt wrap behavior, and whether fixture calibration is applied before or after interpolation.
