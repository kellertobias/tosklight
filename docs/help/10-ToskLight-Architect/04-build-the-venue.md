# Build the venue

Use the CAD title's add tools for stage decks, trusses, scenery, and imported venue models. These objects travel with the show and appear in Control's Stage and the Visualizer.

A Venue object uses a `0.N` identifier and has no DMX address. Its dimensions and placement describe the rig. Fixtures that control physical effects or moving scenery use their own DMX personalities.

![Architect CAD with rig views and Placement controls](../assets/screenshots/workflows/architect-cad.png)

## Add venue objects

The **CAD** screen's own title holds its tools, as icon buttons grouped and divided like every other
title's buttons. Rest the pointer on a button, or reach it with the keyboard, and its name appears in
a tooltip just below it, above the side panel and the viewports. A drawing tool's tooltip also shows
the key that picks it, such as **Draw line · L**. From left to right:

* **Undo** and **Redo** step back and forward through moves and deletions in the drawing. ⌘Z
  (Ctrl+Z) undoes too, and ⇧⌘Z (Ctrl+Y) redoes.
* The add group places venue objects. **Add truss**, **Add stage element**, **Add scenery** and
  **Add primitive** each place their part at once when pressed. A small caret in the button's
  bottom-right corner opens a menu of the parts that button can place, each shown by its picture; the
  part the button places now is highlighted. Choosing a part places it and makes it the part the
  button places from then on, and this computer remembers that choice for the next time you open the
  CAD screen. Every part in the menu has a small button at the right of its row, which can be
  reached with Tab. Beside a truss or a stage deck it is **Place Multiple** (a grid of four
  squares), described below. Beside every other part, the stairs, the handrail, scenery and
  primitives, it is **++**, **Add Several**: it holds that exact part, with its options and size,
  without placing one first, and every click on a CAD view then places another copy where you
  click, until you press Escape or **Done** on the banner.
  * **Add truss** (a truss segment) lists these sections: **Pipe**, **2-point**, **3-point deco**,
    **3-point regular**, **4-point** and **4-point large**, with the straight truss and, for
    **3-point regular** and **4-point**, the corner pieces made for that section. It places a
    **3-point regular** straight truss until you choose another.
  * **Add stage element** (a deck raised off the floor) lists the decks on **Regular feet** and the
    decks on **Scissor feet**, each by platform size, the one **Stairs** and the **Handrail**.
    **Stairs** is one entry. It places a flight without rails. With the flight selected, choose its
    rails on the **Placement** tab in **Info**, under **Parameters → Handrails**: **None**,
    **Left**, **Right** or **Both sides**, left and right as seen climbing it. The drawing shows the
    change at once. Either kind of deck is built
    to the height set in **Info**, so there is no part per leg height. Stairs climb to the height set
    there in 200 mm steps, and a handrail is a stage edge guard 1 m high, set to any length from
    0.4 m to 24 m. It places a 2 × 1 m deck on scissor feet until you choose another. From above, a
    flight is drawn as stairs rather than a deck: a line across it at every nosing, an arrow pointing
    up the climb and its rails along the chosen sides. A flight climbs along its longer side, as it
    does in the Visualizer. An elevation that looks across the climb shows the steps in profile,
    rising the way the arrow points. One that looks along it shows the flight end on, at its full
    height. Turning the flight turns this with it.
  * **Add scenery** (a drape on its rail) lists Curtain, Chain, Disco ball, Stage railing, and Flight rack.
    Curtain is generated at the width and drop set in **Info**. Set a disco ball's **Diameter** from
    0.2 to 1.5 m and its **Chain** up to 3 m. A Flight rack starts at 8U and 0.6 m deep.
    Its **Units** can be 1 to 24 and its **Depth** 0.4 to 1 m, both adjustable after placement.
    Every view draws its road-case shape with rounded corners, ball caps, and a panel per unit.
    The button places Curtain until you choose another part.
  * **Add primitive** (a box, a ball and a cylinder) lists **Box**, **Cylinder** and **Ball**. Each fills the width, height, and depth set in **Info** and uses the chosen **Colour**, neutral grey by default.
    A cylinder stands upright, so its height is its length. **Load model…**
    at the end of its menu opens the file picker for a 3D model, glTF, GLB, 3MF or OBJ, and places
    it at the stage origin, selected, while **Loading the 3D model…** shows until it is in. Place,
    turn and scale it from **Info** like any Venue object; the show carries the model itself, so it
    opens again with the show. A file that cannot be read is refused with the reason and changes
    nothing; closing the picker loads nothing.
  * **Place Multiple**, beside each truss and stage deck in those two menus, opens a dialog that
    lays out many of that exact part at once and writes them in one step. A plan of the
    arrangement under the fields shows every element as it will be placed and follows each change.
    Nothing is placed until **Place**; **Cancel** places nothing at all, and everything placed is
    left selected so you can move, turn or size it as one.
    * For a **truss**, type the **First point** and **Last point** of the run (X, Y and Z in
      metres) and how many **Sections** it is made of. The sections are spaced evenly from the first
      point to the last, each centred on its share of the run, and each is turned about Z only to
      the run's heading on the plan, never pitched or rolled. When the two points stand at
      different heights, each section is raised to the height of its own place along the run, so
      the run is stepped, not tilted. A straight section's **Section length** starts at the length
      that fills the run end to end and can be typed; a corner piece keeps its own size.
    * For a **stage deck**, choose whether the decks lie **Long side across** or **Long side
      deep**, how many go **Across (X)** and **Deep (Y)**, and the **Grid centre** (X, Y and the
      height Z the decks are placed at). The decks butt against each other at their own footprint,
      so the grid has no gaps and no overlaps, and the dialog names the size it will cover.
  * **Add venue element** (a box) opens a dialog listing the Venue objects in this computer's fixture
    library that no button above places, crowds, PA and backline, figures and imported venue
    models, each shown by its picture on a dark ground. The trusses, decks, scenic elements and
    primitives are left out on purpose: place those from their own buttons and part menus. The
    **PA Speaker** is set in **Info**: **Pole stand** puts it up on a pole, and **Pole** sets the
    pole's height up to 2 m under its 0.6 m cabinet. The **Line Array** is set by its **Elements**,
    1 to 24 under its flying frame. Type in the dialog's search to narrow the list by name or type. Choosing an object
    only selects it; **Add** in the title bar (or a double-click on the object) places one. Each object's **++** button, **Add Several**, closes the dialog and prepares repeated placement.
    Click a CAD view to place each copy. A plan places it on the floor; an elevation uses the clicked height.
    Press Escape or **Done** on the banner to stop.

A placed part goes to the stage origin with the next free virtual ID, the drawing shows it at once,
and it is selected so **Info** opens to place it and set its size. A Place Multiple takes the next
free virtual IDs one after another, in the order it lays the elements out. A part whose profile is not in this
computer's fixture library is listed but cannot be chosen, and pressing a button whose part is missing
says so instead of placing anything. When the show refuses a placement, the reason appears at the top
right of the drawing.

## Draw walls and fit openings

**Draw wall**, beside **Draw line**, draws a connected run: click the start and each corner, then
Escape to finish. It snaps to existing corners, points along walls, and horizontal/vertical/45°
directions. Hold Shift to disable corner, wall, angle and loop-closure snapping.

Use its dropdown for windows, doors, pillars and stairs. With a window or door active, hover a
wall to preview the opening, then click to attach it. Escape ends repeated placement.
Select the wall or opening and use **Info > Placement**:

| Object | Main settings |
| --- | --- |
| Wall | Length, height, thickness, colour and room ceiling height. |
| Window | Width, height, sill and rectangular, round or arched shape. |
| Door | Width, height, single/double leaves, inward/outward swing and hinge side. |
| Pillar | Round, square or rectangular footprint and dimensions. |
| Floor, roof or balcony | Outline, elevation above floor and slab thickness. |
| Stairs | Width, rise, run and step count. |
| Furniture | Table shape/size, chair style, or bar dimensions. |

Openings move with their wall. Overlapping openings or openings outside it are refused.
**Snap endpoints to nearby walls (25 cm)** can close a nearby endpoint when placing/applying a wall;
turn it off for a deliberate gap. Connected segments appear as continuous walls in plans and 3D,
while remaining individually selectable and colourable. Enclosed walls create a ceiling at the
lowest requested **Room ceiling height**.

Slab outlines need at least three non-crossing points. Edit local X/Depth points, add/remove points,
then position and rotate the whole slab. Elevation is measured from its placement's floor datum;
duplicate and raise balconies for stacked levels. Stairs rise along positive local Depth; rotate
them with **Angle**. Stairs intersecting a floor or balcony create a cutout automatically, which
follows later stair edits.

Top views draw wall outlines. Side views outline obstructing near walls so you can see and select
the interior. Exported plans retain the venue. In 3D, a camera outside an enclosed room hides
obstructing near walls and ceilings; from inside they remain visible. Cutaway surfaces still block
light and cast shadows. Windows and doors are real openings. Balconies remain visible.

## Objects in the Elements panel

**Objects**, the second tab of **Elements**, lists what the venue is built from apart from the lamps:
**Venue items**, every shipped Venue object, generated like trusses, curtains and stage elements or
drawn from a model like truss corners, decks on legs and the disco ball, and **3D models**, which
lists only the models imported into this show. Each row names the object, its ID, its kind and its
size in metres.

### Groups

Select two or more Venue elements, such as the trusses of a rig or the decks of a stage, and press
**Group** at the top of **Objects**, ⌘G, or **Group** in the right-click menu of a viewport. The
group is saved in the show as **Group 1**, **Group 2** and so on, and is listed under **Groups**
above the other objects; its elements move there from the lists below. **▸** opens a group to show
its members.

A click selects the whole group: its row, one of its members in the list, or any of its elements in a
view, and so does a selection rectangle that catches one of them. Hold **Shift** to select elements individually. Shift-click in a view adds or removes just that element, and Shift-click on a
member in the list selects only that member. A selected group moves together with the gizmo, like any
selection. Every element of a selected group is drawn in violet, in the CAD views and in the 3D
PreViz alike. An individually selected element keeps the ordinary blue selection colour, including one Shift-picked out of its group. **Ungroup**, at the top of **Objects**, in the right-click menu, or ⇧⌘G, dissolves
every group the selection touches and leaves its elements where they are. An element is in one group
at most: grouping it again takes it out of its old group. A deleted element leaves its group, and a
group with no element left goes with it. **+** adds more: the same truss, stage element, curtain,
primitive and venue element as the title, and **Import 3D model…**, which places a GLB, glTF, 3MF or OBJ file as described below and selects it. Select a row to select the object in the
views; **Info** below then sets its position, rotation and scale.

## Place your own venue models

With **Show all** on, the **Patch** sheet's title bar has **+ Import 3D model** beside **+ Add fixture**, for a venue
the fixture library does not have: the hall you are playing, a stage build, a set piece. Choose a
GLB, glTF, 3MF or OBJ file on this computer and it is placed at once, at the stage origin, on the
layer that is open, or the default layer when **All fixtures** is. It gets the next free `0.x` ID
and is selected, so set its **Location** and **Rotation** in the sheet like any other Venue object.

The model is kept in the show, not in the fixture library. It is listed under the manufacturer
**Imported models**, travels with the show when it is saved, copied or opened on a desk, and is
drawn by the Visualizer and the desk's Stage exactly like a shipped Venue object. While one copy
of it is in the show, **+ Add fixture** on the Patch sheet offers it again for another; import the
file again to replace a model with a changed version, which is placed as a new object.

Whatever format you choose, the show keeps the model as one self-contained GLB, so it needs no
other file once it is imported and moving the originals later changes nothing.

- **GLB** is kept as it is. It must be a GLB 2.0 file with its buffers and textures inside it.
- **glTF** (`.gltf`) may keep its `.bin` buffers and its textures beside it, as most tools write
  it; leave them in the folder they were exported to, next to the `.gltf`. A file that needs mesh
  compression such as Draco or meshopt is refused; export it again without.
- **3MF** follows the file's build placement and declared unit, which defaults to millimetres.
  It is turned upright from Z up to Y up, as the same CAD
  tool's own glTF export would, and its base material colours are kept.
- **OBJ** says nothing about units, so it is read as metres with Y up; scale it in the modelling
  tool before exporting if it was built in millimetres. Its `.mtl` material library must be beside
  it, and each material's diffuse colour is kept.

glTF and GLB are read in metres with Y up, as glTF defines. A model is drawn at the size it was
built, so a 20 m hall is 20 m wide, and it may have at most 2,000,000 triangles and 64 MB once
packed together. Surface colours are drawn and textures are not. A file that breaks any of these
rules, or names a file that is not beside it, is refused with the reason, and nothing is added.

Use [Position, rotate, and align fixtures](03-position-and-align.md) for the gizmo, typed placement, duplication, and snapping.
