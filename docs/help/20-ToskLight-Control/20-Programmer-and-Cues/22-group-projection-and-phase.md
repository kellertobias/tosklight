# Group projection and phase

Open a Group's settings and choose **Projection** to map
Stage X/Y/Z positions into a 2D plane. **Phase** ranks that plane into the one-dimensional order
each fixture takes its phase from, as Grid, Radial, or Radar. Fixtures that share a rank receive the
same point of a `THRU` spread; members without a valid Stage position remain visible and receive
individual fallback ranks.

A **Grid** ranks by the lines its direction makes across the plane, and every fixture standing on
one of those lines shares a rank, so the phase advances from line to line and never along one. At
0° the lines are the columns and a ten-by-ten grid runs in ten steps across it; at 90° they are the
rows and it runs in ten steps down it. A direction between the axes makes diagonal lines the same
way, so the same grid at 45° runs corner to corner in nineteen steps, each anti-diagonal moving as
one. Fixtures standing close enough together to be on one line are treated as being on it, so a rig
measured to the millimetre still ranks by the rows and columns it was built as.

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

Set fixture positions in the shared [Patch](../../10-ToskLight-Architect/02-Fixtures-and-Patch/02-add-and-patch-fixtures.md). A Dynamic can [override this mapping](09-dynamics.md#projection-and-phase) without changing its Group.
