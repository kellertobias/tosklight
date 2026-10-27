# Position, rotate, and align fixtures

Select fixtures in CAD or Patch, then use **Info > Placement** for measured positions. The CAD gizmo moves and rotates the same placement. Selection order controls every spread and Placement Assistant layout.

## Position and rotate the selection

**Plans**, **Elements** and **Info** share the side panel at the right of the drawing. Each panel's
title row holds its name, its tabs and a **+** button whose menu lists what can be added there: on
**Plans** a **Fixture list** page, on **Elements** the drawings or objects of the open tab. Drag the
panel's left edge to make it wider or narrower, or focus the edge and use ← and →; the width is kept
for the next time.

**Info** follows the selection. Select one element in a view and it opens at the foot of the side
panel; with no panel open, it is the whole side panel. Its title row has two tabs, **Generic** and
**Placement**; the tab you chose stays open as the selection changes.

For one element, **Generic** edits its **Name** and **Notes** and, for a lamp, its **Patch** as
`universe.address`, one field per split, and an empty field unpatches it. **Placement** edits its
**Position** (X, Y and Z in metres) and **Rotation** (X, Y and Z in degrees). A generated Venue object, such as a truss, curtain, or stage element, shows its **Size** instead of a scale: only the measurements its
profile lets you set, such as a truss's width (its length), a curtain's width and height or a stage
element's height, in metres within the range the profile allows. A size outside that range is put
back without saving. A stage element's or stairs' **Position** is where its feet stand, the
middle of its footprint on the floor, so Z 0 stands on the stage floor, and changing its height
raises or lowers the deck without moving the feet. Every other generated object is positioned by
its centre. Its **Parameters** follow: a **Colour** as a hex value, empty for the kind's
default, and for a chain its **Chain top** and **Chain bottom**. A placed 3D model shows **Scale**. A
lamp shows its **Bracket angle** and **Barndoors** angle, empty when none are fitted. Every number
field names its unit.

An element keeps the exact version of its profile it was placed with, so a show always draws what it
drew when it was built. When this computer's library holds a newer profile version, **Placement** offers **Update to the newest version**.
The newer revision may correct a part such as a truss section or the size of a corner block. It changes nothing else: the
name, position, rotation, patch and the measurements you set stay as they are, and a measurement the
profile does not let you set, such as a truss's section, comes back at the corrected one. Elements
you do not update are left alone. The fields take typing from the keyboard: a change is written when you press
Enter or leave the field, and Escape puts back what was there.

With several elements selected, **Generic** lists them by **ID**, **Name**, **Model** and **Patch**, and
a multi-patched fixture says how many copies it has. Click one to select only that element.
**Placement** edits them together: **X**, **Y**, **Z**, **Rot X**, **Rot Y** and **Rot Z**, and for the
lamps among them **Bracket angle** and **Barndoors**. A field shows the value they share, or the ends of
an even spread as `1 THRU 5`. When the values follow no such line the field is empty and shows the range
they cover, lowest to highest, in the field's unit, such as `-1.2m THRU 1.2m`. Type one
value to set every element, or a range, `1 THRU 5`, `1 … 5` or `1 ... 5`, to spread it evenly from the
first selected element to the last. **Placement Assistant** lays the selection out in the order it was
selected: along a **Line** from a start to an end, in a **Grid** from a start with a number of columns
and a spacing across and deep, or around a **Circle** from its centre, radius, start angle and arc,
where 360° spaces the elements evenly around the whole circle. Nothing moves until **Apply**.

When every selected element was patched from the same model, **Shared model** follows the placement
fields and names it, `All 4 × Generic Stage Element 2 × 1 m`. It carries that model's own controls:
the measurements its profile lets you set, such as a stage element's **Height** or a truss's
**Width**, or a placed model's **Scale**, and the **Parameters** it is built with. They read and
spread like the fields above, one value for the whole selection or a range across it, and each
measurement is held inside the range the profile allows. Sameness is the model itself: four 2 × 1 m
stage elements share one, a 2 × 1 m beside a 1 × 1 m does not, and a mixed selection keeps the
placement fields alone.

The move gizmo stands on the selected element's own origin, or on the centre of a selected group.
Drag an arrow to move along that axis, or the square to move freely. Holding Shift while dragging an
arrow spreads the selection along it, from the first selected element to the last.

Hold **Option** (on Windows and Linux, **Ctrl**) while dragging the gizmo to place a copy instead:
the copy follows the pointer and the original stays where it is. The preview follows the key as you
press and release it mid-drag, and the key's state as you let go of the mouse decides: held, one copy
is placed where the preview shows it; let go of the key first, and the drag moves the selection as
usual, with no copy made. A copy is a new, unpatched element with its own identity and the next free number, as with **Duplicate**.
It becomes the selection, and one **Undo** takes it away again. Option-dragging anywhere but the
gizmo still pans the view.

The amber quarter arc between the two arrows turns the selection about the axis the view looks
along: Z in a plan, Y in a front or back view, X in a side view. Drag along the arc; the turn
follows the pointer round the gizmo in 15° steps, and holding Shift turns it freely in tenths of a
degree. Beside the gizmo, **Rotation Z +30°** (or X, or Y) says how far it has turned. A turn changes
the same rotation the Info panel's **Rotation** field for that axis sets, so the arc and the typed
field always agree. One element turns about its own origin; several turn together about the
gizmo, each carried round it as well as turned. Letting go commits the turn as one step, which
**Undo** puts back; letting go where you started turns nothing. The arc keeps its size on screen at
every zoom.

While you drag, the gizmo shows where its origin is on the two axes the view shows, X and Y in a
plan, the horizontal axis and Z in an elevation, in metres, the same figures as the Info panel's
position. Type a number while still dragging to place the origin there on the axis you are moving
along, or start it with + or − to move it by that distance along the axis's positive or negative
direction; the preview follows what you type, and Enter (or letting go) commits it. On a free drag
with the square, the underlined axis is the one you type into; Tab switches to the other. A comma
works as the decimal point. Text that is not yet a number, such as a lone sign, is marked and moves
nothing: Enter does nothing and letting go abandons the move. Escape clears what you typed, and a
second Escape abandons the move. A typed move neither snaps nor spreads.

### Snapping

With **Settings (⚙) → Enable snapping** on, a moved object that comes within about 12 pixels of a fit
lands exactly on it, and a magenta diamond marks the fit while the drag holds it. Snapping is part of
an ordinary drag of the move gizmo; no key needs to be held:

* **Trusses** join where their connectors meet: the ends of a straight truss, and the end of each
  arm of a corner, T-piece, cross or node, where the coupler egg sits. A corner block is 500 mm
  overall wherever it has arms. Connectors join
  only within one truss system: three-point to three-point, four-point to four-point.
* **Stage elements**, including decks on either kind of feet and stairs, butt against their neighbours.
  Each side aligns independently. Slide a deck along the front of a wider one to close the gap.
  Its end also snaps into line when it approaches the wider deck's end. Two sides that meet on both axes
  put corner on corner. A magenta line runs along each side that was lined up. This works for
  elements turned in quarter turns; one turned at another angle still puts its corners on another's.
  In a side or front view a stage element's feet land on another stage element's top. A stage
  element stands on its position: the position is the floor under the middle of its feet.
* **Curtains** hang their rail just under a truss or pipe, and line their ends up with the ends of
  the next curtain.
* **Handrails** land their foot line on the outside edge of a stage element, and their ends on that
  element's corners, so a run of rail closes a side of the deck.
* **Lamps** clamp onto the nearest pipe of a truss; with snapping on, a lamp dropped onto a truss is
  also recorded as mounted on it. What reaches the pipe is the lamp's own **mounting clip**, which
  its profile declares: for example, the hook clamp on a yoke or the omega bracket under a moving head.
  A short lamp catches a pipe only from the distance its real hardware reaches. A fixture
  whose profile says it hangs from nothing, such as a hazer or a floor can, is never picked up by a
  truss. See **Mounting** in the fixture profile editor to set the clip on a model you imported.

Only the axes the drag can move change, so a plan view never changes heights.
A truss, pipe, or rail more than half a metre off the view's plane is out of reach.
For example, a plan view cannot snap a lamp on the floor to a truss 6 m above it. **Measure** snaps its ends in the same way, onto connectors, stage corners,
curtain ends and the centre of every object. Hold **Shift** to place freely: while it is held, a drag
or a measurement snaps to nothing and a moved lamp is not mounted.

Snapping is on by default. Switch it off in CAD Settings when objects must be placed freely.

The trash button at the top of Info, **Delete** and **Backspace** delete the selection from the
show, with every multi-patch copy of each fixture. A single selected element goes at once; several
are always confirmed first, and **Cancel** leaves every one of them in place. A deletion is one step:
**Undo** in the CAD title, or ⌘Z, brings back everything it removed, same fixtures, numbers, patch
and place, and **Redo** deletes it again. While a field has the focus, Backspace edits the field,
and during a move it edits the typed position.

With **Select** in hand, right-click an element in a viewport for its menu: **Group**, **Ungroup**,
**Duplicate** and **Delete**. Lamps, trusses, stage parts and imported models all offer Duplicate and
Delete; **Group** and **Ungroup** stand above them and appear only when they would do something.
Right-clicking an element that belongs to the selection keeps the selection, so the menu acts on all
of it; right-clicking another element selects it (with its group) first. A right-click on empty plan
opens nothing, and while a drawing tool is in hand a right-click still finishes the line instead.
From the keyboard, press the Menu key or Shift+F10 to open the menu for the selection, the arrow keys to move, Enter to choose and
Escape to close it.

* **Group** and **Ungroup** are the same actions as ⌘G and ⇧⌘G and as the buttons at the top of
  **Objects**, described under [Groups](04-build-the-venue.md#groups). **Group** shows when two or more selected Venue
  elements are not already exactly one group, **Ungroup** when the selection touches a group.
* **Duplicate** (⌘D, Ctrl+D) adds a copy of each selected element beside it, edges touching, to the
  right in the viewport where you opened the menu, or the last-used viewport for ⌘D. A copy is a new element with its own identity and the next free
  fixture (or virtual) number above its original's, keeps its profile, mode, size, rotation and name,
  and is not patched. Give it a DMX address in Info when it needs one. The copies become the
  selection. **Undo** takes them away again.
* **Delete** is the trash button's delete: one element goes at once, several are confirmed first,
  and **Undo** brings the elements back.

A multi-patched fixture stands in the plan once for each copy. Click a copy and Info edits that copy
alone: its name, position and rotation change, and the fixture and its other copies stay where they
are. **Copy** at the top of Info switches between the original and each copy, named by its DMX
address. Notes and scale belong to the fixture, so their labels say **(all copies)**. Dragging in the
plan still moves the fixture with all its copies.
