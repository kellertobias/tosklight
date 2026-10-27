# Draw and print plans

Open **CAD** to prepare plans and elevations of the rig. Use **Elements > Drawings** for venue underlays and annotations, and **Plans** for printable pages. Set the title-block information and company logo on the [Show screen](01-create-and-open.md#the-show-screen).

## Prepare a print page

Open **Plans**, then add a plan page from the viewport you want to print. Each page keeps that viewport's orientation and cut planes. Use the **+** menu in Plans to add a **Fixture list**. Tick the pages to include, then choose **Export to PDF**.

The page's cogwheel controls fixture IDs, DMX patch labels, mounting hardware, drawings, and cut planes for that page. Drawing tools are unavailable while print pages are open.

## Compose and export plans

Plan frames remain visible when you turn their CAD viewport. Each sheet keeps its saved view
rotation, so the frame, labels and title block turn together relative to the rig. The Plans list
shows that rotation. Use **View rotation** in the selected page settings to change the printed
view while keeping its centre. **Rotate page** changes the paper between portrait and landscape.

Plans use the same fixture poses, wall joins, and drawing geometry as CAD. Each page retains its own drawing visibility and mounting-hardware settings.

Set **Page title** in the selected page settings. Pages, titles, inclusion, framing and view
rotation are saved in the portable show document and travel with **Save As**. Existing pages
from this computer are adopted once when the document has no saved pages. Wait for page saving
to finish before exporting; a failed save offers **Retry saving pages**.

Use **Delete page** to remove the selected sheet. **Fixed scale** locks a plan to its **Scale 1:**
denominator, including after paper rotation. The frame can still move; unlock it to resize freely.

The title block prints the page title above the project or show name, then the venue and
designer's details. Vertical **Project** and **Designer** labels identify the two left sections.
Missing phone and email rows are omitted. The right side holds the **Printed** timestamp,
fixture and universe counts, scale, grid spacing and logo. The company logo fits its box without
changing its proportions. A separate gray footer on the left identifies ToskLight Architect and
www.tosklight.de.
The application shows white text on a dark block; PDF output uses black text on white.
The default vector logo retains its blue accent in both.

PDF plans retain vector fixture outlines and text, so zooming or printing does not depend on a
bitmap export resolution. A supplied company logo retains the resolution of its uploaded image.
Fixture IDs and DMX labels use transparent, collision-aware placement that keeps text clear of
the lamp drawings as well as other labels, with leader lines back to their lamps when needed. Print mode shows labels for the selected sheet, or the first included sheet when
none is selected, while retaining the other page frames. If labels cannot fit at a readable size,
the preview warns and export asks you to use closer framing or split the plan into pages.

## Plan orientation

The top-down plan reads like a map: **+X** runs to the right and **+Y** up, so upstage is at the
top of the drawing and the audience at the bottom. The orientation mark in each viewport names the
axes it shows. Rotating a top-down viewport turns the whole plan with it; the mark follows, so a
quarter turn clockwise shows **+Y** to the right and **−X** up.

A lamp's technical drawing keeps the shape of its selected top, front or side view. Rotation
about that view's normal turns the drawing; rotations out of its plane do not squash or shear it.
Live model projections and physical Venue elements retain their full 3D placement.

## Cut planes

Seen from the side, a stage with curtains both sides is a wall: the near one hides the rig the
drawing is about. Each CAD viewport therefore carries a pair of cut planes, set beside its view
selector, that limit how far into the drawing it looks.

Give a **From** and a **To** depth in metres. Either may be left empty, which is what "everything
upstage of the house curtain" means: a near limit and no far one. The axis being cut is the one
the view looks along, so a top-down plan cuts by **Height** and the four elevations cut by
**Depth**. The control shows the drawing's own near and far ends as placeholders, and **Show all**
returns the whole drawing.

An element has thickness, so a plane passing through one still shows it; only an element lying
wholly beyond the cut is dropped. A pair given the wrong way round reads as the same slice. What
the cut takes away is also unselectable, since you cannot pick what the drawing does not show.

A print page added from a viewport takes that viewport's cut planes with it, so a PDF prints the
slice you composed. A viewport without cut planes shows the whole drawing.

## Drawings under the plan

A plot is drawn over the venue, so the venue's own drawing can be placed under it. **Elements** in
the CAD title opens a side panel whose title row carries its two tabs, **Drawings** and **Objects**,
and a **+** button that adds to the open tab. On **Drawings**, **+ → Import drawing (DXF, SVG)…**
takes a DXF or an SVG, reads it, and
tells you what it found, what the file says its units are, how large the drawing is in metres, and
how many lines it holds, before anything is placed. Nothing is written into the show until you
press **Place Drawing**.

Every drawing belongs to one axis, the one it was drawn for: a ground plan to the top-down view, a
section or a front elevation to the matching elevation. It appears on the views that show that
axis and nowhere else, and a rotated top-down view turns its drawings with the rig.

Placement is in metres of the show, not of the file: give the drawing's origin an **X** and **Y**,
a **Scale** if the file was not drawn at full size, and a **Rotation** if it does not face the way
the rig does. Check the result against something you know, a stage width, a truss span, and the
rest of the drawing follows. **Show** takes a drawing off the views without removing it; **Remove**
deletes it from the show.

The show stores the drawing as lines. A placed drawing travels with the show: save it, copy it to another machine, open it there, and
the plan is still under the rig with no file to go looking for. Re-importing the same drawing after
the CAD file changes places it again as a new drawing.

Lines, polylines, circles, arcs and ellipses are drawn, and so are the blocks a plan is assembled
from. Text, dimensions, hatches and splines are not: a drawing under a plot is there to say where
the walls are, and the lettering of somebody else's title block is not part of that. Cut planes do
not apply to drawings either, so a view cut to a slice of the rig still shows the venue it stands
in.

**Drawings** lists every placed drawing and every line, box, text and measurement drawn on a view,
as a tree you arrange freely. **+ → New folder** adds a folder, inside the selected folder, or at the
top level, ready to be named. Drag a row onto a folder to put it inside, or onto another row to put it
before that row. The selected row carries the same moves as buttons: **↑** and **↓** move it among its
neighbours, and **⇤** takes it out of its folder. A selected folder also shows **✎** to rename it and
**✕** to delete it, which hands what it held to the folder around it, so no drawing is lost. The arrangement is saved in the show and every Architect window follows it. Drawings without a folder open at the top level. Select a placed drawing to set its
**X**, **Y**, **Scale** and **Rotation** below the tree, or a drawn item to **Erase** it.

Each print page carries its own switches for the drawings on its axis, beside the page's cut
planes. A page prints every drawing of its axis unless you switch one off.

## Grid, annotations, and shortcuts

A pale grey grid is drawn over every view. Its lines are one step of the scale indicator apart, so
zooming changes the grid with the scale. **Settings (⚙) → Grid** switches it off, sets its colour, or
fixes its spacing to a length that no longer follows the scale. Type the colour as a hex value such as
`#c9d1d9` and press Enter, pick it from the colour panel, or choose one of the presets. The grid is a
setting of this computer's Architect, not of the show: every show opened here uses it, every open
Architect window follows a change at once, and a show carried to another machine is drawn with that
machine's grid. **Show sub-grid** adds small plus
signs at the scale indicator's quarter steps between the lines. A grid too dense to read at the
current zoom is left out until you zoom in.

What the drawing tools draw is saved in the show, shown in every Architect window, and printed on that
view's plan pages:

* **Draw line**: click each point. The length of the segment you are drawing is shown on it until
  you click. Double-click, press Enter or right-click to finish an open line, a right-click adds no
  point where the pointer is; click the first point again to close it.
* **Draw box**: click one corner, then click the opposite corner.
* With snapping on, line and box points snap onto nearby truss corners, connectors, or stage corners.
  Without an object to snap to, a line nearly horizontal or vertical becomes exactly so.
  Other coordinates snap to whole 10 cm. Hold Shift to draw exactly where the pointer is.
* **Place text**: click where the text starts, type, and press Enter. Text is sized for the zoom it
  was placed at and grows and shrinks with the plan.
  With **Select** in hand, click placed text to pick it: it is outlined in cyan and picked on its
  own, so the element drawn under the words stays unselected. Drag the words to move them, or use
  the gizmo that stands on the text's start, the square moves it freely, an arrow along one axis.
  While text is picked, **Info** shows its **Text**, its **Height**, its **Font** and its
  **Position**, X and Y on a plan, **Across** and **Height** on an elevation, and typing a value
  there moves or rewords it. **Font** sets the words in the screen's own typeface (the default),
  **ISO 3098 (osifont)** for lettering to the technical-drawing standard, or one of Allen V.
  Hershey's plotter alphabets: **Hershey Simplex** (single stroke), **Hershey Duplex** (heavier)
  or **Hershey Complex** (serif). The Hershey alphabets have no umlauts, ß or some symbols; those
  characters are drawn in osifont. The show keeps only the font's name, so it opens anywhere, and
  a font a later release adds shows in the screen's own typeface here. The fonts ship with
  ToskLight under their own open licences, listed with the third-party notices. A printed plan
  embeds each font its text is set in, so the PDF shows the same lettering on any computer and its
  words can still be searched and copied; text in the screen's own typeface and every measurement
  print in Helvetica.
  Every move or change is one step **Undo** puts back, and it is kept with the show. Click anywhere
  else to put the text down.
* **Measure**: drag from one point to another. The measurement is drawn in amber with a tick at
  each end and its distance, in millimetres below a metre, in metres above. With snapping on, each
  end snaps onto the nearest connector, stage corner, curtain end or object centre; hold Shift to
  measure from exactly where you press.
* **Erase**: click a line, box, measurement or text to remove it.

Escape drops a line, box or text still in progress; pressing it again returns to **Select**. The
middle mouse button or Alt still pans while any tool is in hand. On a rotated top-down view, what
you draw turns with the rig. Drawing is off while the print pages are open.

Single keys work the CAD screen without the pointer. They act on the viewport you last clicked in,
and are ignored while you type in a field or a dialog is open:

| Key | Does |
| --- | --- |
| **V** | **Select** |
| **L** | **Draw line** |
| **P** | **Draw box** |
| **T** | **Place text** |
| **M** | **Measure** |
| **R** | **Erase** |
| **1** to **5** | **Top down**, **Left to right**, **Right to left**, **Front to back**, **Back to front**, framed on the rig |
| **+** and **−** | Zoom in and out |
| **W**, **A**, **S**, **D** | Move the view up, left, down and right |
| **⌘D** (Ctrl+D) | **Duplicate** the selection beside itself and select the copies |
| **⌘G** (Ctrl+G) | **Group** the selected Venue elements |
| **⇧⌘G** (Shift+Ctrl+G) | **Ungroup** the selected groups |

Set project paperwork under **Show information** on the **Show** screen.

## Fixture drawings in plans and documents

Plans and the Visualizer use the top, left, right, front, and back drawings carried by the fixture profile. The SVG's millimetre coordinate
space, origin, and orientation place it at physical scale. A fixture whose package carries no model
of its own is drawn in the CAD from the line drawing of the shipped model the Visualizer shows for
it, at the fixture's size; generated Venue objects keep their own drawing. The Visualizer's own
plan and elevation views draw such a fixture from the same line drawings, its top drawing on the
plan and its front and side drawings on the elevations, mirrored for the back and the right, and
fall back to the model's generated silhouette for a view it has no drawing of. Only if neither exists
is the renderer's fixture-type vector used, followed by a plain box for an unknown type.

Every CAD view shows the stage the way the Visualizer's own views do: the plan has downstage at
the bottom, **Left to right** (standing house left) has downstage on the right, **Right to left**
has it on the left, and the front elevation is seen from the audience. A fixture's front, its lens
and its direction indicator land on the same side of the stage as in the 3D view.

Blinders, flat LED PARs, strobes and floods are drawn facing the audience from above and from the
front, and PARs and Fresnels point forward from above. From the side, every lamp that hangs in a
frame is drawn exactly as the Visualizer poses it for the fixture's **Bracket** angle, 0
included: the body turns about its hinge, the clamp and the hanging frame stay where they are and
are drawn in front of it. A Fresnel at 45° is drawn with its body turned 45° nose-down about the
bracket bolts while its hanging frame and coupler do not move, and the 3D view shows it the same.
An elevation shows whichever side of the lamp its rotation turns toward it, to the nearest quarter
turn: a lamp turned 90° shows its side, and its bracket angle, on the front elevation. At 0 a
blinder hangs face-down like any other lamp; turn the bracket to point it at the audience. A
multi-patch instance uses its own bracket angle. A fixture drawn from its own 3D model turns as a
whole by its bracket angle in every view. The plan (top) view keeps each drawing's own pose. LED
wash moving heads are drawn from the front looking straight at you. **Export MVR** writes a
bracketed lamp the same way, turned about the same hinge, so its light leaves the lens where the CAD
and the 3D view show it; see [Shows, Revisions, and MVR](../20-ToskLight-Control/10-Show-Setup/10-shows-revisions-and-mvr.md).

Each lamp's direction indicator starts where its light leaves it, the lens of the body it is
drawn with, and points where the lamp points: its bracket angle and its rotation, as the
Visualizer aims it, with moving heads at their home pan and tilt. A lamp with no known lens starts
its indicator at its position. The viewports and printed pages draw the same indicator; a viewport
draws it in a half-transparent yellow, so it stands apart from the lamp's outline.

A print page's cogwheel has a **Mounting hardware** switch, on by default. Switch it off to omit clamps wherever the model provides a drawing without them. The viewports always show the hardware.

On-screen plans and printed plans use the same fixture drawings. Parts nearer the view hide parts behind them.
