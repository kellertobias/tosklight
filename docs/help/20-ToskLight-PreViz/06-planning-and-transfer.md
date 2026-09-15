# Plan a Rig and Move It

The PreViz Rig Editor is a rig-planning window: the same patch sheet the desk uses, over a
show file rather than over a running desk. A rig planned there and a show running on a desk are
the same rig, and neither side should have to go looking for a file to get from one to the other.

> [!danger] Missing graphic
> Add an authority and transfer diagram showing copied `.show` documents between Control and the Rig Editor, renderer-local overrides, configured show DMX inputs, derived Desk routes, and live Art-Net or sACN values.

## Open Demo Show

The editor's file bar has an **Open Demo Show** button, and it needs nothing else: no file to find,
no desk on the network, and no rig to patch first. It opens a full demonstration rig — front-of-house
profiles and PAR cans, moving washes and profiles, beams, strobes, scanners, Sunstrips, blinders, a
hazer, three lasers, six cold-spark fountains, four flame jets and two media servers — as an
ordinary show of your own. The venue includes its trusses, stage decks, curtains, a dancing crowd
and a disco ball over the dancefloor. One media server feeds the two projection screens at the
stage sides; the other feeds the three LED panels around the Sunstrips.

On a new installation the editor opens a writable copy of this Demo Show immediately. The Desk's
**Default Stage Show**, the editor copy, **Open Demo Show**, and Visualizer demo mode all come from
this same portable template.

What opens is always a **copy**. The demo that ships with ToskLight is a template and is never
opened, never written to, and never changed by anything you do. The copy is written into this
installation's own shows folder and named after the demo it came from: **Demo Show** the first
time, **Demo Show 2** the next, and so on. The file bar's status line says which copy it is and
where it was written.

So a demo copy is yours. Patch it, repatch it, save it, rename it, delete it. Pressing **Open Demo
Show** again gives you a fresh copy of the shipped rig rather than reopening whatever you did to
the last one.

The demo is built from the fixture packages this version of ToskLight ships, so its fixtures carry
the same profile revisions, models and modes the fixture library does. It is the quickest way to
see what the Visualizer draws, and the rig the product demonstration video is shot from.

## Rename the show

A show has a name of its own, separate from its file. The **Show** page of **Settings** shows it over the rig
overview, with a pencil beside it. Press the pencil, type the new name, and press Enter or click
away to keep it; Escape leaves the name as it was, and an empty name changes nothing. The name is
what a desk's **Load Show** menu offers, so a renamed show is offered under its new name at once.
The file keeps its name; **Save As** writes the show to a new one.

## Configure the fixtures the rig is made of

**Fixtures** is a page of **Settings**, at the foot of the dock, rather than one of the show's own
screens, because the fixture library belongs to the computer rather than to the document. The page
is available with no show open, and every show planned here afterwards patches from what is in it.
Its **Create fixture** action and search sit left of the Settings pages in the one title.

The library reads in three columns — **Manufacturer**, **Fixture**, and **Fixture info** — so each
column offers only what the column to its left has already chosen. Choose a manufacturer to see its
fixtures, and a fixture to read what it is: type, modes with their footprints, size, weight, power,
connectors, light source, lens, colour temperature, luminous output, beam angle, and its
photograph. The search bar narrows every column at once. **Create fixture** opens a blank profile;
**Edit as new revision** opens the chosen one. This is the same editor ToskLight Control uses, so a
profile authored here is the one the desk reads, with the same Identity, Simulation and Modes tabs
and the same rules. Saving stores the next immutable revision: the library
assigns the number, and a save is refused rather than silently overwriting work if another window
has revised the same fixture in the meantime. A show already patched against an earlier revision
keeps its own embedded snapshot and is unaffected.

Two differences from the desk are deliberate. The Architect reads the operator's own filesystem
when choosing a photograph, icon, or GLB, because it has no configured file roots to confine a
chooser to. And the Geometry tab has no live 3D preview here: the parts, transforms and emitters
are edited and saved exactly as they are on the desk, but confirming the Stage appearance belongs
in ToskLight Control, which owns the Stage renderer. The tab says so rather than showing an empty
frame.

Channel work is the same on both products. Channels are ordered by dragging a row or by its move
buttons, and that order is the DMX slot order within the split. A channel's DMX range is divided
into named functions — a shutter that is closed to 17, open to 72, then strobing to 254 reads as
those names on an encoder instead of a percentage — and one physical channel may carry functions
of different kinds, so a combined dimmer-and-strobe slot is one channel with two bands. Whether a
channel follows a virtual dimmer, and whether it inverts or never fades, are set per channel in
its editor.

## Editing the patch sheet

The dock has four screens: **CAD**, **Patch**, **Venue** and **Media**. **Patch** lists every
fixture with a DMX address, lamps and effect devices — lasers, foggers, particle effects and
patched scenery — alike; **Venue** lists the objects that are placed but not patched. The **Patch**
title has two tabs: **Sheet** is the patch sheet, and **DMX** is the address grid described in
"The Patch screen's DMX tab" below.

**Settings**, below the screens, holds the pages that are not a view of the rig: **Show** (the file
actions and the rig overview), **Visualizer**, **Fixtures**, **DMX** and **MCP**. **Visualizer** is
one page of boxes, two to a row: **Lamp** and **Laser** atmosphere, then **Rendering**,
**Features** and **Picture**.

Click a column header to order the sheet by that column, for example **Fixture ID** or **Patch**;
click it again to reverse the order. An arrow marks the column the sheet is ordered by. **Patch**
orders by universe, then address. Fixtures with nothing in the column, such as unpatched fixtures
or fixtures without a note, always come last, and fixtures with the same value keep Fixture ID
order. Shift and drag ranges follow the order the sheet shows. The sheet opens ordered by
Fixture ID.

Choose which columns the sheet shows from the window's **Settings** (⚙) under **Columns**; the last
visible column cannot be switched off. Patch and Venue each remember their own columns on this
computer. Above the switches, **View** offers three quick views, which only the Architect has — the
desk's Show Patch window does not:

- **Patch**: Fixture ID, Name, Manufacturer, Product / mode, Patch, Masters, Invert Pan,
  Invert Tilt, MIB, Layer and Note.
- **Visualization**: Fixture ID, Name, Location, Rotation, Bracket, Shaper, Footprint, Colour,
  Chain, Scale, Layer, 2D, 3D and Note.
- **Compact**: Fixture ID, Name, Patch, Layer and Note.

**Footprint width**, **Footprint height** and **Footprint depth** size a generated Venue object — a
truss's length, a curtain's width and drop, a stage element's rise — and show a dash for the
measurements the object is not made to, and for everything else in the rig. Click a size to type
it in metres; a size the object cannot be built at is refused with the range it can take. The size
is kept when the object is edited here or on the desk.

**Colour** sets a generated Venue object's colour — a curtain's serge, a black truss — with
**Default colour** returning it to its own material. **Chain** chooses **Plain chain**, **Motor on
top** or **Motor on bottom**; the end without the motor has a steelflex. The Visualizer draws each object with what was chosen for it, and a desk shows and edits the
same columns in **Show Patch**.

**Scale** draws a Venue object — an imported model above all — at a multiple of the size it was
built at, from `0.01` to `100`. `1`, or an empty entry, is its built size; anything outside the
range is refused with the range. The plan, the Visualizer and a desk's Stage all draw it at that
scale, and everything that is not a Venue object shows a dash.

**View** names the quick view whose columns the sheet shows exactly; changing a single column
leaves it reading **Custom**.

**Masters** says which masters reduce a fixture: `none`, `group` (the Group Masters only), `grand`
(the Grand Master only) or `both`. Click it to choose another value; anything short of `both`
warns which master the fixture stays live under. A fixture with no intensity shows `—`.

**MIB** is Move in Black: `Off`, or the delay before the fixture moves while dark, from `0s` to
`30s` with decimals such as `2.5s`. Click it to switch Move in Black off or on and set the delay.

Click a fixture's **Note** to open the note editor.

**No Layer Assigned**, under **All fixtures**, lists every fixture on the screen that has no layer
of its own: fixtures still on the default layer, and fixtures whose layer no longer exists. While
**Show all** is off, the sidebar hides every layer whose fixtures all belong to another screen —
for example a Trusses layer on the Patch screen — and hides **No Layer Assigned** when it would be
empty. An empty layer you created stays listed. Selecting fixtures in the drawing switches
**Show all** on only when that selection happens; returning to the screen later does not switch it
on again.

To delete a layer, press the bin to the right of its fixture count and confirm. Its fixtures stay
in the show and move to **No Layer Assigned**. A locked layer must be unlocked first, and the
default layer has no bin. The bin is the Architect's; on the desk, delete a layer with the keyboard
shortcut or the hardware button.

While a layer is open, selecting stays in it: clicking, Shift and drag ranges, and Ctrl-click
only ever select that layer's fixtures, and the sheet keeps showing the layer. A selection made in
the drawing or another window opens **All fixtures** only when it includes fixtures outside the
open layer.

Right-click a value to edit it. When the fixture you right-click belongs to the selection, the
editor changes every selected fixture; when it does not, the selection is replaced by that fixture
and only that fixture is edited.

A Fixture ID range in the value entry can be left open: `1100 [THRU] [ENTER]` numbers the selected
fixtures 1100, 1101, 1102 and on, and `1101 [THRU] [−] [ENTER]` numbers them 1101, 1100, 1099 and
on. A closed range such as `1012 [THRU] 1004` still counts from its first value towards its last.

After **Enter**, the sheet scrolls to the fixture whose value you opened when the new value moved
its row out of view, for example when a new Fixture ID sorts it further down.

With several fixtures selected, **Delete** asks whether to delete or unpatch all of them. Delete
removes every selected fixture from the show and clears the selection; Unpatch keeps their fixture
lines and clears all their DMX addresses. Either choice is applied as one patch change. Fixtures
on a locked layer are left out, and a fixture outside the selection is still deleted on its own.

## Lighting the rig without a desk

Select fixtures in the patch sheet and the editor's preview controls light them. No desk, no
network route, no console: this is what makes a demo rig worth opening, and what lets you check a
plan from every view before anyone rigs anything.

**Simple** exposes Intensity, Pan, Tilt, Colour and Gobo. It sets them on every selected fixture at
once, through the fixture's own profile — so one colour works on an RGB fixture and a CMY one
without you having to know which you are looking at, and a 16-bit channel gets its fine byte.

**Full DMX** is available when exactly one fixture is selected, and shows every slot of that
fixture's complete mode: every logical head, every channel, coarse and fine bytes, named as the
fixture library names them. It is a testing tool for one fixture, not a way to program a rig, so
selecting none or several disables it visibly and leaves the values it already set alone.

**Clear** returns the selected fixtures to their defaults.

These are preview values, not programming. They are session state of the window: they never enter
the show file, never become a preset, a cue or a stored look, and they are gone when the document
is closed. The editor has no programmer, no command line, no playbacks and no cue stack — anything
that starts to need cues, tracking or arbitration belongs on a desk, and the answer there is to
connect to one.

### When real DMX arrives

Preview values apply to a fixture while no source has delivered that fixture's universe.

* Start a real Art-Net or sACN source for one of the show's universes and the fixtures on it follow
  the network immediately. Fixtures on other universes keep the editor's values.
* Stop that source and the visualizer **holds the last values it received** rather than reverting to
  the preview look. A universe that has had real DMX keeps it until you clear it or a source
  resumes — a rig that jumped back to a preview the moment a console was unplugged would be worse
  than one that froze.
* Editor values and received DMX are never blended for the same parameter. A universe has one
  owner at a time.

The visualizer's status line says which is happening: **No DMX in — the Viz editor is driving these
fixtures** when the preview plane is lighting the rig, and the ordinary **Waiting for DMX** when
nothing is driving it at all.

## Load from Desk

When a ToskLight desk with a show open is on the same network, the editor's file bar gains a
**Load from Desk** button naming that desk and the show it is running. Pressing it takes a copy of
that show, keeps it beside the editor's own documents, and opens it. Two desks are two buttons,
each naming its own machine; hovering one shows the address it was found at.

What arrives is a copy. Patching it here does not reach the desk, and the desk does not know the
copy exists. To send work back, use **Load from Visualizer** in the desk's **Load Show** menu.

## The Patch screen's DMX tab

The **DMX** tab in the **Patch** title shows every channel of every patched universe as a grid of
numbered cells. A cell is lit when a fixture, one of its splits, or one of its multi-patches
occupies that address and dark when nothing does. The addresses of one fixture share a single
outline, a fixture's first address carries a mark on its left edge, so neighbouring fixtures stay
apart, and an address two patches share is drawn in orange. Select a cell to see which fixture owns
it, its patch range, split, fixture channel and attribute, and the address's DIP-switch setting.
**Sheet** returns to the patch sheet.

## The DMX settings page

**DMX** is a page of **Settings**. Its own three tabs sit left of the Settings pages in the title:

* **Network** configures where the show's DMX arrives from: the network interface this computer
  receives it on, and the live DMX inputs described below.
* **Values** is the desk's **DMX Output** window applied to the DMX this machine receives. Each
  universe the show patches or listens on is a row of dots, one for every channel, that brighten
  with the received level. A patched channel's dot has a grey outline and an unpatched one a dark
  red outline. A universe's header names the protocol, frame rate and sender, **Holding the last
  frame** once a source stops, or **Waiting for DMX** before anything arrived. Select a dot to
  read its value, fixture and DIP switches; with nothing selected, the side column lists every
  input being listened on with its health, its sender, and how many packets it accepted. The
  window's settings set the dot size. The editor outputs no DMX, so nothing on this tab overrides
  a value.
* **Sources** lists every Art-Net node and sACN source this computer finds on the network, one row
  per IP address: its name, IP address and protocols, its **Inputs** — each DMX input and the
  Art-Net universe it sends — and **Outputs** — each DMX output and the Art-Net universe it plays —
  and the universes it **Sends** and **Receives**. A universe is highlighted while its data is
  arriving. Select a node's name to read its long name, MAC address and status report.

Values listens exactly where the Visualizer does: the show's output routes, the live DMX inputs
over them, and the Art-Net and sACN defaults for every patched universe when neither names any,
each on the interface chosen for its protocol. It only listens while the tab is open, and shares
its ports so a Visualizer on the same computer keeps receiving beside it.

Sources sends an Art-Net poll to each network every three seconds and lists the nodes that answer;
a node that stops answering disappears after about ten seconds. sACN has no poll: a source is
listed from the universe-discovery announcements it sends every ten seconds. The ToskLight desk
does both, so it appears with every universe its routes send. A sender that neither answers polls
nor announces still appears, with the universes it is seen sending: any Art-Net broadcast, and
sACN on the show's own universes. sACN receivers announce
nothing, so they cannot be listed. Like Values, Sources uses the interface chosen for each protocol
and only looks while the tab is open. A node that answers polls only by unicast can be missed
while a Visualizer on the same computer is receiving Art-Net.

## Choose the input network

A computer with more than one network — a lighting network beside an office or venue network, or
two lighting networks — can receive DMX on only the one that carries it. The **Input Interfaces**
section at the top of the **DMX** settings page's **Network** tab has one choice per protocol:

* **Art-Net interface** and **sACN interface** each offer **All interfaces**, the default, and
  every network interface this computer has, by its system name and IPv4 address — for example
  `en0 · 10.0.0.5`. The loopback interface is marked **(this computer only)**.
* Choosing an interface takes effect at once: the **Values** tab and a running Visualizer listen
  on that interface only. Art-Net broadcast, sACN multicast, and unicast addressed to that
  interface all still arrive; DMX arriving on any other network is ignored.
* The two protocols are independent, so Art-Net can come from one network and sACN from another.

The choice belongs to this computer, not the show. It is saved with the Visualizer's settings,
so the Visualizer and the editor always listen on the same network, and a show opened on another
computer never carries this one's interface names.

An interface is remembered by its name, so an address that changes when the network is renewed is
still the same choice. When the chosen interface is not connected, it is listed as **not
connected** and a warning says that the protocol is not received until it returns: the editor
does not quietly fall back to every network. New interfaces appear in the list within a few
seconds of being connected.

## Configure live DMX inputs

The **DMX** settings page's **Network** tab maps a logical show universe to the Art-Net or
sACN universe the separate Visualizer output receives. Each mapping can be enabled or disabled
and carries its protocol, wire universe, delivery mode, and UDP port. Art-Net offers Broadcast or
Unicast; sACN offers Multicast or Unicast. Choose **Apply** to store the mappings in the portable
show, or **Cancel** to discard the draft. The network interface the mappings are received on is
chosen separately for each computer, as described above, and is never written into the show.

When a desk is detected, **Take from Desk** reads that desk's compatible output routes through a
read-only Visualizer session. With more than one desk, first select the source. The imported routes
are only a preview until **Apply** is chosen: taking routes does not replace the show and does not
change the desk. An explicit show input wins over a derived output route for the same logical
universe.

A Visualizer showing the editor's document always receives where these settings say. The
per-universe input pins in the Visualizer's own Quick Settings apply only while it shows a desk or
an opened show file; they are hidden while the editor is the source and never replace the show's
inputs.

## Load from Visualizer

The desk's **Load Show** menu offers the document this editor has open, in the same way and with
the same result: the desk imports it as an ordinary show and opens it. Only an editor that
actually has a document open is offered — an editor with nothing open is on the network, and says
so, but there is nothing to load from it.

## What the two sides publish

Each application announces itself on the local network, over the same standard service discovery
printers and audio interfaces use, saying which of the two it is and what it currently holds. It
publishes nothing else: no show content, no programming, no desk state. The name follows the
machine, so a rig with two editors is two entries an operator can tell apart.

The document served to the network is read-only, and it is the same read-only document the
Visualizer itself reads. There is no route into the editor that changes anything from outside it.

## When there is nothing to offer

Discovery is a convenience and never a requirement. A network with no discovery, a firewall that
blocks it, or a machine where the responder will not start costs the button and nothing else:
both applications start, run, and open files exactly as they did before. A show file opened
through **Open** or **Show from USB** is the same show file either button would have fetched.

## Fixture drawings in plans and documents

The Visualizer and rigging-document consumers use the same named top, left, right, front, and back
SVG drawings carried by an immutable fixture-package revision. The SVG's millimetre coordinate
space, origin, and orientation place it at physical scale. If that package has no usable drawing,
the renderer's fixture-type vector is used, followed by a plain box for an unknown type.

SVG remains the source for both on-screen and printable plans. HTML retains the vector artwork;
PDF or PNG output rasterizes that SVG at the requested output size rather than maintaining a
separate bitmap asset. Plan composition uses world depth and explicit opaque/empty regions, so its
occlusion does not depend on package file order or incidental 3D material names.

## Plan orientation

The top-down plan reads like a map: **+X** runs to the right and **+Y** up, so upstage is at the
top of the drawing and the audience at the bottom. The orientation mark in each viewport names the
axes it shows. Rotating a top-down viewport turns the whole plan with it; the mark follows, so a
quarter turn clockwise shows **+Y** to the right and **−X** up.

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
slice you composed rather than the whole rig. A viewport saved before cut planes existed has none,
which means the whole drawing.

## Drawings under the plan

A plot is drawn over the venue, so the venue's own drawing can be placed under it. **Elements** in
the CAD title opens a panel with two tabs, **Drawings** and **Objects**. On **Drawings**, **Add
Drawing** takes a DXF or an SVG, reads it, and
tells you what it found — what the file says its units are, how large the drawing is in metres, and
how many lines it holds — before anything is placed. Nothing is written into the show until you
press **Place Drawing**.

Every drawing belongs to one axis, the one it was drawn for: a ground plan to the top-down view, a
section or a front elevation to the matching elevation. It appears on the views that show that
axis and nowhere else, and a rotated top-down view turns its drawings with the rig rather than
leaving the plan behind.

Placement is in metres of the show, not of the file: give the drawing's origin an **X** and **Y**,
a **Scale** if the file was not drawn at full size, and a **Rotation** if it does not face the way
the rig does. Check the result against something you know — a stage width, a truss span — and the
rest of the drawing follows. **Show** takes a drawing off the views without removing it; **Remove**
deletes it from the show.

What the show keeps is the drawing itself, as lines, rather than the file it came from. A placed
drawing therefore travels with the show: save it, copy it to another machine, open it there, and
the plan is still under the rig with no file to go looking for. Re-importing the same drawing after
the CAD file changes places it again as a new drawing.

Lines, polylines, circles, arcs and ellipses are drawn, and so are the blocks a plan is assembled
from. Text, dimensions, hatches and splines are not: a drawing under a plot is there to say where
the walls are, and the lettering of somebody else's title block is not part of that. Cut planes do
not apply to drawings either, so a view cut to a slice of the rig still shows the venue it stands
in.

**Drawings** lists every placed drawing and every line, box, text and measurement drawn on a view,
as a tree you arrange freely. **New folder** adds a folder — inside the selected folder, or at the top
level — and **Rename** names it; **Delete folder** removes a folder and hands what it held to the folder
around it, so no drawing is lost. Drag a row onto a folder to put it inside, or onto another row to put
it before that row. Because the desk surface is touched rather than dragged, the same moves are
buttons too: **↑** and **↓** move the selected row among its neighbours, and **⇤** takes it out of its
folder. The arrangement is saved in the show and every Architect window follows it; a show saved
before folders existed opens with every drawing at the top level. Select a placed drawing to set its
**X**, **Y**, **Scale** and **Rotation** below the tree, or a drawn item to **Erase** it.

Each print page carries its own switches for the drawings on its axis, beside the page's cut
planes. A page prints every drawing of its axis unless you switch one off, which is also what a
page saved before drawings could be placed does.

## The CAD title's tools

The **CAD** screen's own title holds its tools, as icon buttons grouped and divided like every other
title's buttons. Rest the pointer on a button, or reach it with the keyboard, and its name appears in
a tooltip just below it. From left to right:

* **Undo** and **Redo** step back and forward through changes to the drawing.
* The add group places venue objects from the fixture library:
  * **Add truss** (a truss segment) opens the library on rigging, searched for trusses.
  * **Add stage element** (a deck on a scissor lift) opens it on Venue objects, searched for stage
    decks, stairs and railings.
  * **Add curtain** (a drape on its rail) opens it on Venue objects, searched for curtains.
  * **Add venue element** (a box) opens it on every object that is placed but not patched, such as
    crowds and imported models.
* The drawing group is what a press on a viewport does. **Select**, the arrow, selects and moves the
  rig as before; the others draw on the view you use them in.
* **Print** and **Elements** open their side panels, and **Settings** (⚙) the CAD settings.

Choosing an object from an add button places it exactly as **+ Add fixture** on the **Venue** screen
does, and the drawing shows it at once. Clear the search to choose anything else the filter offers.

What the drawing tools draw is saved in the show, shown in every Architect window, and printed on that
view's plan pages:

* **Draw line**: click each point. Double-click or press Enter to finish an open line; click the
  first point again to close it.
* **Draw box**: drag from one corner to the opposite corner.
* **Place text**: click where the text starts, type, and press Enter. Text is sized for the zoom it
  was placed at and grows and shrinks with the plan.
* **Measure**: drag from one point to another. The measurement is drawn in amber with a tick at
  each end and its distance — in millimetres below a metre, in metres above.
* **Erase**: click a line, box, measurement or text to remove it.

Escape drops a line, box or text still in progress; pressing it again returns to **Select**. The
middle mouse button or Alt still pans while any tool is in hand. On a rotated top-down view, what
you draw turns with the rig. Drawing is off while the print pages are open.

The project paperwork printed on every page is no longer a CAD panel: set it under **Show
information** on the **Show** screen.

## Objects in the Elements panel

**Objects**, the second tab of **Elements**, lists what the venue is built from apart from the lamps:
**Venue items** — trusses, stage elements, curtains and the other generated objects — and **3D
models**. Each row names the object, its ID, its kind and its size in metres. Its buttons add more:
the same **Add truss**, **Add stage element**, **Add curtain** and **Add venue element** as the title,
and **Import 3D model**, which places a GLB, glTF, 3MF or OBJ file as described below and selects it.
Select a row to select the object in the views and set where it stands — **X**, **Y** and **Z** in
metres — as if you had dragged it there. How large a placed model is drawn is its **Scale** in the
**Venue** sheet.

## Place your own venue models

The **Venue** screen's title bar has **+ Import 3D model** beside **+ Add fixture**, for a venue
the fixture library does not have: the hall you are playing, a stage build, a set piece. Choose a
GLB, glTF, 3MF or OBJ file on this computer and it is placed at once, at the stage origin, on the
layer that is open — or the default layer when **All fixtures** is. It gets the next free `0.x` ID
and is selected, so set its **Location** and **Rotation** in the sheet like any other Venue object.

The model is kept in the show, not in the fixture library. It is listed under the manufacturer
**Imported models**, travels with the show when it is saved, copied or opened on a desk, and is
drawn by the Visualizer and the desk's Stage exactly like a shipped Venue object. While one copy
of it is in the show, **+ Add fixture** on the Venue screen offers it again for another; import the
file again to replace a model with a changed version, which is placed as a new object.

Whatever format you choose, the show keeps the model as one self-contained GLB, so it needs no
other file once it is imported and moving the originals later changes nothing.

- **GLB** is kept as it is. It must be a GLB 2.0 file with its buffers and textures inside it.
- **glTF** (`.gltf`) may keep its `.bin` buffers and its textures beside it, as most tools write
  it; leave them in the folder they were exported to, next to the `.gltf`. A file that needs mesh
  compression such as Draco or meshopt is refused; export it again without.
- **3MF** is placed as its build says, with every part in its place, in the unit the file declares
  — millimetres unless it says otherwise. It is turned upright from Z up to Y up, as the same CAD
  tool's own glTF export would, and its base material colours are kept.
- **OBJ** says nothing about units, so it is read as metres with Y up; scale it in the modelling
  tool before exporting if it was built in millimetres. Its `.mtl` material library must be beside
  it, and each material's diffuse colour is kept.

glTF and GLB are read in metres with Y up, as glTF defines. A model is drawn at the size it was
built, so a 20 m hall is 20 m wide, and it may have at most 2,000,000 triangles and 64 MB once
packed together. Surface colours are drawn and textures are not. A file that breaks any of these
rules, or names a file that is not beside it, is refused with the reason, and nothing is added.
