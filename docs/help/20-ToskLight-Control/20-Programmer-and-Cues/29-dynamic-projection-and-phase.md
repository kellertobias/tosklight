# Dynamic projection and phase

The Dynamics editor separates target ordering from phase distribution. **Projection** places
fixtures in a 2D plane and nothing else; **Phase** turns that plane into the one-dimensional order
each lamp takes its phase from.

**Projection** starts from the saved live Group's mapping. **Inherit** follows the Group;
**Override** enables the local controls and a **Projection** toggle choosing Planar, Cylindrical,
or Spherical. All three are placed by one position and one direction, and each offers only what it
reads: Planar takes a direction and a Rotation and reads no position, while Cylindrical and
Spherical take a position and are aimed by Azimuth, Elevation, and Rotation. Azimuth swings the
direction; Elevation lifts it. Rotation rolls about the resulting direction and decides where the
spread starts. Angles are set in whole degrees. A simplified Stage view beside the values shows the
shape, its axis, and where the spread starts, with the selected lamps as dots (or every lamp when
nothing is selected), so the shape can be placed against the real rig; drag it to orbit. Changes are saved
as they are made, so there is no Apply button; if the Dynamic changed elsewhere, authority reloads
and your values are kept. Frozen or targetless Dynamics instead show **Selection order (no Group
mapping)**. On the encoders, projection takes two pages: the kind and its position, then its
direction and rotation.

**Phase** owns the ordering (**Inherit**, Linear, Grid, Radial, Radar, or Random) alongside
offset, span, anchors, blocks, repeats, wings, and uniform/per-lane phase behavior. **Inherit**
takes the Group's ordering and appears only for a Dynamic bound to a live Group, because anything
else has nothing to inherit from. **Linear** follows source selection order even when the Group has a spatial mapping. In per-lane mode, ordering edits affect only the selected lane; Projection remains shared. **Random** ignores fixture positions and projection.
