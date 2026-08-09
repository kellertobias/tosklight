# Groups and Presets

Groups store ordered fixture selections. Presets store reusable attribute values.

## Groups

Select fixtures, press `[REC]`, and choose a Group target or enter its number. Normal record overwrites; `[REC] [+]` merges; `[REC] [-]` subtracts. Intentionally empty Groups remain valid stored objects and differ from absent Group numbers. Missing Groups in a range are skipped.

A Group reference remains connected to its source; dereference it when a frozen fixture list is required. Derived Groups retain their ordering rule and source relationship. See [Command Line Reference](../30-Programmer/01-command-line.md) for exact syntax.

Plain-click a Group to select its live membership. Double-press it quickly to select the current
members as frozen fixtures. Right-clicking a populated Group is the pointer shortcut for `[SET]`
then tapping that Group: it selects the Group as the source for a Playback assignment. Touch-hold,
or enter `[SET] [GRP] <number> [ENTER]`, to open the Group settings modal. **General** contains only name, icon, and color. **Projection** maps
Stage X/Y/Z positions into a 2D plane. **Phase** ranks that plane into the one-dimensional order
each fixture takes its phase from, as Grid, Radial, or Radar. Fixtures that share a rank receive the
same point of a `THRU` spread; members without a valid Stage position remain visible and receive
individual fallback ranks.

Every projection is placed by one position and one direction, whichever kind it is. **Planar** looks
along the direction and ranks across the plane at right angles to it; Rotation turns that plane, the
position is not read, and the named Top/Front/Back/Left/Right presets remain. **Cylindrical** runs
its axis along the direction through the position, and Rotation is where the spread starts around
that axis; the spread leaves that angle in both directions and the two sides meet 180° away on the
far side. **Spherical** points the direction from the position to the centre of the spread, which
reaches its opposite 180° away, and Rotation turns the meridian that spread is measured around.
With the direction along Stage +Z and no Rotation, a cylinder stands upright and starts its spread
at Stage +X. Cylindrical and Spherical are aimed by **Azimuth** and **Elevation** rather than by
direction components: Azimuth swings the direction about Stage +Z and Elevation lifts it off the
horizon. Both tabs show what they set: **Projection** draws the shape in the Stage,
drag to orbit it, and **Phase** plots the fixtures on the plane the projection ranks them in, shaded
from black for the first rank to white for the last.

A Group inherits its mapping from referenced Groups until it is given one of its own; changing
anything on **Projection** or **Phase** makes the mapping this Group's own. Membership remains live
either way. Mixed referenced mappings use visible source order until the Group receives its own
mapping. There is no general Layout pane or selection grid: edit fixture positions in Stage, Group
ordering here, and a Dynamic-only override in that Dynamic's **Projection** tab.

Group settings never contain a master. To create a Group Master, press `[SET]`, choose the explicit
Group, then press the destination physical or Virtual Playback. Every Playback targeting the same
Group controls one shared master; Playbacks targeting different Groups remain independent HTP
contributors. Changing a Playback to **None** removes that assignment without changing the Group.

![Group pool with populated ordered Groups](../assets/screenshots/panes/groups.png)

## Presets

Preset families are Mixed, Intensity, Color, Position, and Beam. Intensity stores only intensity attributes. Color stores RGB, CMY, color-wheel, and other Color attributes. Position stores only Position attributes. Beam stores Beam and Focus attributes. Mixed stores any attributes the operator chooses; it does not mean a combined list of every preset family. Recalling a Preset applies compatible values to the current selection while retaining the relationship needed for later updates where supported.

Each family is a separate pool with its own local preset numbers. The command-line address combines type and number: `0.1` is Mixed 1, `1.1` is Intensity 1, `2.1` is Color 1, `3.1` is Position 1, and `4.1` is Beam 1. The dotted address is not a global preset ID, so all five presets numbered 1 can coexist.

Use pane settings to choose the displayed family and whether tiles use type colors or individual colors. The desk defaults are pale orange-yellow for Groups, lime for Cuelists and Sequences, cyan for Dynamics, dark red for future Macros, and grey for every Preset family. Desktop settings can customize or reset each default. Selection, focus, Store/Record/Update, disabled, and empty states also use borders, outlines, markers, labels, or dashed geometry so color is never their only indication. Test Presets on representative fixture modes before building Cues from them.

![Preset pool and family-specific tiles](../assets/screenshots/panes/presets.png)
