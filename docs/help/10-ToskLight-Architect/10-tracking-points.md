# Tracking points

A **3D Point** gives the show a named position. Fixtures can aim at it, and fixtures or scenery can use it as a Position Reference. Plan its resting position in the rig before programming movement in Control.

Use Architect to place the point as a **ToskLight > 3D Point** fixture. The Points view and Position Reference assignments described here are Control's operator controls.

## Points

**Show Patch > Points** lists every 3D Point of the show in fixture ID order: the places the Position encoders aim at.
A Point is an ordinary **ToskLight → 3D Point** fixture, so it keeps one identity whether it has a
DMX address, has none, or is moved by a tracker, and it is saved with the show like any fixture.

**+ Create Point**, beside the view tabs, adds an aim Point without a DMX address at the stage
origin. It takes the next fixture ID after the highest one in the show and is named **Point 1**,
**Point 2** and so on; the new row is highlighted. The same happens when you choose **Create Point**
from the Position **Point** encoder, which opens this view.

Each row shows:

| Column | Meaning |
| --- | --- |
| **ID** | The Point's fixture ID. Change it in **Fixtures** with **Set fixture ID**. |
| **Name** | Type a new name and press Enter, or use the keyboard button. The Point encoder and the Position Reference column use this name. |
| **X**, **Y**, **Z** | Where the Point rests on stage, in metres. **Point X/Y/Z** on the Point's own encoders, a cue or a tracker move it from there. |
| **Patch** | **Unpatched**, or the `universe.address` it sends its pose to. Patch or unpatch it in **Fixtures**. |
| **Tracking** | The PosiStageNet tracker bound to it, or `—`. Bind trackers in **Tracking**. |

**Delete** asks once more with **Confirm delete** before the Point is removed. Programming that still
aims at it keeps the reference and reads **Missing point** on the Point encoder until you choose
another Point; fixtures hung on it read **None** in Position Reference.

## Position Reference: attach objects to a 3D Point

A **3D Point** (manufacturer **ToskLight**) is a reference object you patch like any fixture and
move from the Position encoders: **Point X**, **Point Y** and **Point Z** in metres and **Point Rot
X**, **Point Rot Y** and **Point Rot Z** in degrees. Every axis rests at the centre of its range, so
a freshly patched point sits exactly where you placed it.

As soon as the show holds a 3D Point, Show Patch draws a **Position Reference** column. Select any
fixture or Venue object, press `[SET]` and touch its **Position Reference** cell, or right-click the
cell, and choose a point by its fixture ID and name; **None** places it against the stage again. A
truss and every lamp hung on it can reference the same point: when that point is moved or rotated,
everything referencing it moves with it, relative to the point's own origin. The location and
rotation stored for a referencing fixture describe where it sits while the point rests at its
origin, and they do not change when the point moves. A 3D Point shows a dash in the column and
cannot reference another point, and nothing can reference itself. The column disappears again when
the last point leaves the show; a fixture whose point was deleted reads **None** and is drawn
against the stage.

The Visualizer and the desk's Stage draw a referencing fixture or Venue object where the point puts
it, and `Fixture 1 AT Fixture 5` aims at the moved object. The Architect's 2D CAD keeps drawing it
where it was rigged, because the plan uses the rigged placement; **Settings > General > Show fixture IDs** also shows **Follows 3D Point** labels. Select an object to read its full reference in **Info > Placement**.

Left unpatched, a 3D Point is a programmer-only object. Patched, it sends its pose over DMX so a
hoist controller or another visualizer can follow the same point. Its modes are **Position 16
bit**, **Position 24 bit** and **Position 32 bit** (X, Y and Z at that width, no rotation) and
**Full 16 bit**, **Full 24 bit** and **Full 32 bit** (the same position axes followed by the three
rotations at 16 bit each). Position is offset binary: the middle value of the channel is no offset,
values below it move the point negative and values above it positive, reaching ±100 m at the ends of
the range. Rotation is ±180° about its middle value.

## Live tracking

A 3D Point can exist without a tracking system. PosiStageNet supplies live positions after an operator binds a tracker to a point in Control. Architect's CAD shows the rigged placement while the Visualizer displays the live result.

See [Tracking with PosiStageNet](90-Advanced/04-posistagenet.md) for receiving trackers, binding them, calibrating the stage, and handling stale input.
