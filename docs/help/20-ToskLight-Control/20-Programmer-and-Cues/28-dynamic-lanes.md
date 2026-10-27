# Dynamic lanes

Tile names use the tile's text area without an inner scrollbar, so long
names such as **Beam Auxiliary Show Circle** and **Beam Auxiliary Show Waterfall** stay
distinguishable. The top-right number and bottom-right artwork sit above the name with
backgrounds that fade into the tile.

The symbol at the bottom right of a Dynamics tile shows its lane families.
The diagonal X divides it into four parts: **I** at the top for Intensity, **P** on the left
for Position, **C** on the right for Color, and **O** at the bottom for Other. Filled parts
use the Dynamic's colour; grey parts mean that family has no lane. Several filled parts
show a combination. The bottom-left number is still the count of fixtures currently running
that Dynamic.

The symbol follows the configured lanes, including when the Dynamic is stopped. Pan, Tilt
and Target lanes belong to Position; all colour lanes belong to Color. Other covers lane
families such as Focus, Zoom, Beam and Gobo. Editing lanes updates the symbol automatically;
you do not need to choose an icon for the pool tile. For example, a Pan/Tilt Dynamic fills **P**, a colour chase fills **C**, and a Dynamic combining dimming with movement fills **I** and **P**. A coloured sector means the lane exists, not that it is currently outputting or running. Any saved custom artwork remains stored, but the Dynamics pool uses this automatic lane symbol.

Each lane of a Dynamic animates one attribute. Tap an empty Dynamics tile, or **+ Add Lane** in the
editor, to open the lane chooser: it lists attribute groups first, and the attributes sit inside
their group.

Position, Color and Zoom lanes are fixture-independent, like every other Position, Color and Zoom
value: **Pan** and **Tilt** animate Angles in degrees, **Zoom** the beam opening in degrees, and
the Color group offers **Red**, **Green**, **Blue** and **Amber** recipe components, **Hue** in
degrees, **Saturation**, **White Blend**, **Temperature** in kelvin and **UV**. The same Dynamic
therefore runs on any mover or colour mixing system. Other attributes, such as Intensity or Focus,
run between 0 and 100 %. The lane encoders, keyframe values and curves show each lane in its own
units, so a Pan amplitude reads `45°`, not a percentage; enter a value in those units on the
encoder's value modal.

A new Pan or Tilt lane swings around **Current** on a Sinus: 45° for Pan, 30° for Tilt. Every
Angle Dynamic moves Pan and Tilt together, so adding Pan also adds a **Tilt** lane that follows
Current, and adding Tilt adds a Pan lane. To animate that partner, select it and choose a curve
function, or add the axis yourself with **+ Add Lane**, which replaces the partner. For a circle:
add Pan, add Tilt, select the Tilt lane and choose **Cosinus**.

**Current** is the fixture's Position before the Dynamic, read again on every frame. When the
fixtures aim at a Target (Point or X/Y/Z), Current is the Pan and Tilt that aim at it, so the circle
runs around the Target. The Programmer keeps the Target: move the Point or edit X, Y or Z while the
Dynamic runs, and the circle's centre follows. Releasing the Dynamic shows the Target's own aim.
If the Target cannot be reached, for example because its Point was deleted, the Dynamic holds its
Pan and Tilt instead of guessing.

Within one Dynamic, the Red, Green, Blue and Amber recipe lanes and the Hue and Saturation lanes
are two different ways of writing the base colour; a Dynamic uses one of them. White Blend,
Temperature and UV combine with either.

Return to [Dynamics](09-dynamics.md) for targets, activation, speed, and run mode.
