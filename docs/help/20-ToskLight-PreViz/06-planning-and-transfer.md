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
stage sides and the header screen flown above the back truss; the other feeds the three LED panels
around the Sunstrips.

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

## The Show screen

**Show**, at the top of the dock, is the open show as a document. It is available with no show open,
because it is where a show is created or opened. Its file actions run across the top: **New Show**,
**Load Show from Disk**, **Open Demo Show**, **Load from Desk** for each desk found on the network,
**Save As**, **Import MVR** and **Export MVR**.

Below them the show is seen from both sides at once. The left half draws the rig from above, with the
show's name over it. The right half is **Show information**, which titles every printed CAD page, in
two columns:

* **Show**, on the left: **Project**, **Venue**, **Show date** and **Version**.
* **Lighting designer**, on the right: **Name**, **Phone**, **Email** and the **Company logo**.

Below both columns are the show's name, when it was last saved, and how many fixtures and universes it
uses. Change a field and press **Save project info** to write it into the show; nothing is saved
until you do.

**Upload logo** takes a PNG, JPEG or WebP. The show keeps it as one small JPEG, flattened onto white
and fitted within 800 × 400 pixels, so it travels with the show and needs no file beside it. Every
printed page — plan pages, fixture lists and the page frames on the CAD screen — shows the logo in
the title block where the ToskLight mark is otherwise drawn. **Replace logo** chooses another one and
**Remove logo** takes it off; either change is kept once you press **Save project info**. A logo file
that is not one of these formats, or is larger than 10 MB, is refused with the reason.

**Make Default** keeps the lighting designer's name, phone, email and logo on this computer, not in
the show. Every show created afterwards with **New Show** starts with them; shows that are opened,
copied from a desk or opened from the demo keep their own. Press it again whenever the details change.

**MCP** in the Show screen's title opens the MCP integration in its place; press it again to return
to the file actions.

### MCP tools

The MCP server edits the show that is open in the Architect, the same way its own windows do. Each
edit is written against the revision it read, so a change you make in a window at the same time is
refused, not overwritten. Fixtures are named by the number an operator says out loud; a Venue object
is named by its `0.N` number (pass `"0.10"` as text, because the number 0.10 is 0.1). Anything
outside what the Architect allows — a size outside a profile's range, a DMX address on a Venue
object, a source for a server that is not there — is refused with the reason.

**Fixtures and Venue objects**

* `search_fixture_library` — profiles by manufacturer, name or mode, with their ids, modes, patch
  policy and Venue kind.
* `list_fixtures` — every fixture and Venue object, with placement, patch and layer. A Venue object
  also reports its kind, its size in metres, which measurements can be set, its colour, a chain's
  ends and its model scale.
* `add_fixture` — adds a fixture or a Venue object. Name the profile by `profile_name`, plus
  `manufacturer` when two makers use the name, for its newest revision and first mode; or pass the
  ids `search_fixture_library` returns. A Venue object gets the next free `0.N` number and no DMX
  address. `size_metres`, `colour`, `chain_mode` (or `chain_top` and `chain_bottom`) and
  `model_scale` set its size and look as it is placed.
* `remove_fixture`, `set_fixture_placement`, `set_fixture_shaper_and_gel`, `set_fixture_identity`,
  `set_fixture_patch`, `add_multipatch` and `assign_position_master` — edit one fixture. Nothing
  else about the fixture changes.
* `set_venue_size` — sets a Venue object's width, height or depth in metres. Only the measurements
  its profile makes adjustable can be set, within the profile's minimum and maximum, as in the patch
  sheet.
* `set_venue_options` — sets a Venue object's colour as `#RRGGBB`; `null` gives it its kind's own
  material back. On a chain it also sets the ends: `chain_mode` is `plain`, `motor_top` or
  `motor_bottom`, or set `chain_top` and `chain_bottom` one at a time.
* `set_model_scale` — draws an object at 0.01 to 100 times the size it was built at; `null` draws it
  at its built size.
* `list_layers`, `remove_layer` and `set_fixture_layer` — list the patch layers, empty one onto
  another, and move one fixture. `save_layer` works on a desk only; the Architect has no route that
  creates or renames a layer.

**Media layout**

These tools edit the Media workspace's layout. A desk has no media layout, so there they are
refused.

* `get_media_layout` — media servers, sources, LED module types, surfaces with their sections, and
  projectors, each with its id and revision.
* `put_media_server`, `put_media_source`, `put_led_module_type`, `put_media_surface` and
  `put_media_projector` — create an object when no `id` is given, or update the one named. An
  update changes only the fields given. A surface's `sections` replace its sections. Each section
  is a `projection_screen`, `tv` or `led` section, and an LED section without `occupied_cells`
  fills every cell of its grid.
* `delete_media_object` — deletes one object by `kind` and `id`. Deleting a server removes its
  sources; a surface that showed one keeps its fallback image.

## Rename the show

A show has a name of its own, separate from its file. The **Show** screen shows it over the rig
overview, with a pencil beside it. Press the pencil, type the new name, and press Enter or click
away to keep it; Escape leaves the name as it was, and an empty name changes nothing. The name is
what a desk's **Load Show** menu offers, so a renamed show is offered under its new name at once.
The file keeps its name; **Save As** writes the show to a new one.

## Configure the fixtures the rig is made of

**Library** is a page of **Settings**, at the foot of the dock, rather than one of the show's own
screens, because the fixture library belongs to the computer rather than to the document. The page
is available with no show open, and every show planned here afterwards patches from what is in it.
Its **Create Fixture** action, marked with a plus, and search sit left of the Settings pages in the
one title.

The library reads in three columns — **Manufacturer**, **Fixture**, and **Fixture info** — so each
column offers only what the column to its left has already chosen. Choose a manufacturer to see its
fixtures, and a fixture to read what it is: type, modes with their footprints, size, weight, power,
connectors, light source, lens, colour temperature, luminous output, beam angle, and its
photograph. The search bar narrows every column at once. **Create Fixture** opens a blank profile;
**Edit as new revision** opens the chosen one. This is the same editor ToskLight Control uses, so a
profile authored here is the one the desk reads, with the same Identity, Simulation and Modes tabs
and the same rules. Saving stores the next immutable revision: the library
assigns the number, and a save is refused rather than silently overwriting work if another window
has revised the same fixture in the meantime. A show already patched against an earlier revision
keeps its own embedded snapshot and is unaffected.

One difference from the desk is deliberate: the Architect reads the operator's own filesystem
when choosing a photograph, icon, or GLB, because it has no configured file roots to confine a
chooser to. The Geometry tab is the same as on the desk, including its live 3D preview: the lamp
body alone on a dark-blue background, framed to fill the view, updating as parts, transforms and
emitters are edited. The preview shows the geometry only, without a beam; how the lamp lights the
rig is checked in the Visualizer.

Channel work is the same on both products. Channels are ordered by dragging a row or by its move
buttons, and that order is the DMX slot order within the split. A channel's DMX range is divided
into named functions — a shutter that is closed to 17, open to 72, then strobing to 254 reads as
those names on an encoder instead of a percentage — and one physical channel may carry functions
of different kinds, so a combined dimmer-and-strobe slot is one channel with two bands. Whether a
channel follows a virtual dimmer, and whether it inverts or never fades, are set per channel in
its editor.

## Editing the patch sheet

The dock has four screens: **Show**, **CAD**, **Patch** and **Media**. **Patch** lists every
fixture with a DMX address, lamps and effect devices — lasers, foggers, particle effects and
patched scenery — alike. Switch on **Show all** above its layers and it also lists the Venue
objects that are placed but not patched — trusses, stage elements, curtains and imported models —
and **+ Add fixture** offers them. Selecting a Venue object in a view switches **Show all** on. The **Patch**
title has two tabs at its right end, directly left of the window's **Settings** (⚙): **Sheet** is
the patch sheet, and **DMX** is the address grid described in "The Patch screen's DMX tab" below.

The Visualizer follows the selection of **CAD** and **Patch**. Every selected fixture, imported model
and Venue object — trusses, stage elements and curtains included — is outlined in blue there, in the
3D views and on the plan and elevations alike. Selecting more, deselecting and changing the selection
update the outline at once; clearing the selection removes it. The outline only marks the object: its
light output, position and material stay as they are, and clicking in the Visualizer picks as it
always does.

**Settings**, below the screens, holds the pages that are not a view of the rig: **Visualizer**,
**Library** and **DMX**. It opens on **Visualizer**. **Visualizer** is one page of boxes that fill the window's width — two to a row, three or four once
the window is wide enough, and one to a row in a narrow window: **Lamp** and **Laser** atmosphere,
then **Rendering**, **Features** and **Picture**. **Features** holds **Crowd amount** beside the
labels, selection and floor grid. When the boxes are taller than the window, the page scrolls, so
every control can be reached however short the window is.

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
on again. The layer list appears once the screen's fixtures have arrived, already reduced to what
the screen shows, so no layer flashes up and vanishes as the screen opens.

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

The tab's only window setting (⚙) is **Show sidebar**, which shows or hides the side column; this
computer remembers the choice.

To repatch a fixture, drag its block by any of its cells to the new addresses. The block keeps the
cell you took it by under the pointer, stops at the start and end of the universe, and can be
dropped into another patched universe. A split or a multi-patch copy moves on its own. Moved blocks
are outlined with a dashed line, and **Pending patch** at the top of the side column lists every
move with its old and new address. Nothing is written until **Apply Patch**, which writes every
moved fixture as one patch change; **Discard** puts the blocks back. A refused change stays pending
and shows the reason. Dragging back to the stored address takes a move off the list. Because
**Apply Patch** is in the side column, blocks can only be dragged while **Show sidebar** is on.

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
  input being listened on with its health, its sender, and how many packets it accepted. Values
  has no window settings. The editor outputs no DMX, so nothing on this tab overrides a value.
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
and the 3D view show it; see [Shows, Revisions, and MVR](../10-Desk/10-Show-Setup/10-shows-revisions-and-mvr.md).

Each lamp's direction indicator starts where its light leaves it — the lens of the body it is
drawn with — and points where the lamp points: its bracket angle and its rotation, as the
Visualizer aims it, with moving heads at their home pan and tilt. A lamp with no known lens starts
its indicator at its position. The viewports and printed pages draw the same indicator; a viewport
draws it in a half-transparent yellow, so it stands apart from the lamp's outline.

A print page's cogwheel has a **Mounting hardware** switch. It is on for a new page and for a page
saved before the switch existed; switched off, that page prints every fixture without its clamp
wherever the model has a drawing without one. The viewports always show the hardware.

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
the CAD title opens a side panel whose title row carries its two tabs, **Drawings** and **Objects**,
and a **+** button that adds to the open tab. On **Drawings**, **+ → Import drawing (DXF, SVG)…**
takes a DXF or an SVG, reads it, and
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
as a tree you arrange freely. **+ → New folder** adds a folder — inside the selected folder, or at the
top level — ready to be named. Drag a row onto a folder to put it inside, or onto another row to put it
before that row. The selected row carries the same moves as buttons: **↑** and **↓** move it among its
neighbours, and **⇤** takes it out of its folder. A selected folder also shows **✎** to rename it and
**✕** to delete it, which hands what it held to the folder around it, so no drawing is lost. The arrangement is saved in the show and every Architect window follows it; a show saved
before folders existed opens with every drawing at the top level. Select a placed drawing to set its
**X**, **Y**, **Scale** and **Rotation** below the tree, or a drawn item to **Erase** it.

Each print page carries its own switches for the drawings on its axis, beside the page's cut
planes. A page prints every drawing of its axis unless you switch one off, which is also what a
page saved before drawings could be placed does.

## The CAD title's tools

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
  CAD screen. Every part in the menu has a small **++** button at the right of its row. Its tooltip
  reads **Add Several**, and it can be reached with Tab. It holds that exact part, with its options
  and size, without placing one first: every click on a CAD view then places another copy where you
  click, until you press Escape or **Done** on the banner.
  * **Add truss** (a truss segment) lists the sections — **Pipe**, **2-point**, **3-point deco**,
    **3-point regular**, **4-point** and **4-point large** — with the straight truss and, for
    **3-point regular** and **4-point**, the corner pieces made for that section. It places a
    **3-point regular** straight truss until you choose another.
  * **Add stage element** (a deck raised off the floor) lists the decks on **Regular feet** and the
    decks on **Scissor feet**, each by platform size, the one **Stairs** and the **Handrail**.
    **Stairs** asks for its handrails as you choose it: **No handrails**, **Left**, **Right** or
    **Both sides**, left and right as seen climbing the flight; the choice goes with the flight and
    can be changed later under **Parameters → Handrails** in **Info**. Either kind of deck is built
    to the height set in **Info**, so there is no part per leg height. Stairs climb to the height set
    there in 200 mm steps, and a handrail is a stage edge guard 1 m high, set to any length from
    0.4 m to 24 m. It places a 2 × 1 m deck on scissor feet until you choose another. From above, a
    flight is drawn as stairs rather than a deck: a line across it at every nosing, an arrow pointing
    up the climb and its rails along the chosen sides. A flight climbs along its longer side, as it
    does in the Visualizer. An elevation that looks across the climb shows the steps in profile,
    rising the way the arrow points. One that looks along it shows the flight end on, at its full
    height. Turning the flight turns this with it. Stairs placed from the retired **Stairs with
    Handrails** part keep their rails up both sides until you choose otherwise.
  * **Add scenery** (a drape on its rail) keeps the scenic elements together: the **Curtain** —
    one, generated at any width and drop you set in **Info**, so a 3 m drape is this curtain at 3 m —
    the **Chain**, the **Disco ball** — any diameter from 0.2 to 1.5 m on up to 3 m of chain, both
    set in **Info** as **Diameter** and **Chain** — the **Stage railing** and the **Flight rack**, chosen by the
    rack units it holds: 2U, 4U, 6U, 8U, 12U or 16U. A rack is 0.6 m deep until you set another
    depth in **Info**, where **Units** (1 to 24) and **Depth** (0.4 to 1 m) change it at any time. It
    places the curtain until you choose another.
  * **Add primitive** (a box, a ball and a cylinder) lists **Box**, **Cylinder** and **Ball**. Each
    fills the width, height and depth set in **Info** — a cylinder stands upright, so its height is its
    length — and takes the **Colour** set for it, neutral grey until you choose one. **Load model…**
    at the end of its menu opens the file picker for a 3D model — glTF, GLB, 3MF or OBJ — and places
    it at the stage origin, selected, while **Loading the 3D model…** shows until it is in. Place,
    turn and scale it from **Info** like any Venue object; the show carries the model itself, so it
    opens again with the show. A file that cannot be read is refused with the reason and changes
    nothing; closing the picker loads nothing.
  * **Place several…** ends the **Add truss** and **Add stage element** menus. A rig is rarely one
    of anything, so it opens a wizard that lays out a whole field of the part the button places now
    and writes it in one go. Nothing is placed until **Place**; **Cancel** places nothing at all,
    and everything the wizard does place is left selected so you can move, turn or size it as one.
    * For a **stage element**, choose whether the elements lie **Long side across** or **Long side
      deep**, then how many go **Across** and how many **Deep**. They butt against each other at
      their own footprint, so the field has no gaps to close afterwards, and the wizard names the
      size the field will cover. The first element lands where one placed on its own would.
    * For a **truss**, type the **Heights** the run is flown at and the lines it stands on —
      **Back** for runs that lie across the room, or **Across** once you turn them to **Runs
      deep**. One run is placed per height and line: two heights over three lines is six trusses.
      Both fields take a list such as `5 7`, or an evenly spaced run such as `4 THRU 8 BY 2`; a
      comma is a decimal point, as everywhere else on the desk.
  * **Add venue element** (a box) opens a dialog listing the Venue objects in this computer's fixture
    library that no button above places — crowds, PA and backline, figures and imported venue
    models — each shown by its picture on a dark ground. The trusses, decks, scenic elements and
    primitives are left out on purpose: place those from their own buttons and part menus. The
    **PA Speaker** is set in **Info**: **Pole stand** puts it up on a pole, and **Pole** sets the
    pole's height up to 2 m under its 0.6 m cabinet. The **Line Array** is set by its **Elements**,
    1 to 24 under its flying frame. The curtains made at one fixed width, and the disco ball, racks,
    PA tops and line array modelled at one size, are no longer offered; a show that placed one still
    draws it. Type in the dialog's search to narrow the list by name or type. Choosing an object
    only selects it; **Add** in the title bar (or a double-click on the object) places one. The
    small **++** button on each object — its tooltip reads **Add Several** — closes the dialog and
    holds that object instead: every click on a CAD view then places another copy where you click,
    on the floor in a plan and at the height you click in an elevation, until you press Escape or
    **Done** on the banner at the top of the view.

A placed part goes to the stage origin with the next free virtual ID, the drawing shows it at once,
and it is selected so **Info** opens to place it and set its size. A wizard's field takes the next
free virtual IDs one after another, in the order it lays the elements out. A part whose profile is not in this
computer's fixture library is listed but cannot be chosen, and pressing a button whose part is missing
says so instead of placing anything. When the show refuses a placement, the reason appears at the top
right of the drawing.
* The drawing group is what a press on a viewport does. **Select** (**V**), the arrow, selects and
  moves the rig as before; **Draw line** (**L**), **Draw box** (**P**), **Place text** (**T**),
  **Measure** (**M**) and **Erase** (**R**) draw on the view you use them in.
* **Plans** and **Elements** open their side panels, and **Settings** (⚙) the CAD settings.

## The side panel

**Plans**, **Elements** and **Info** share the side panel at the right of the drawing. Each panel's
title row holds its name, its tabs and a **+** button whose menu lists what can be added there: on
**Plans** a **Fixture list** page, on **Elements** the drawings or objects of the open tab. Drag the
panel's left edge to make it wider or narrower, or focus the edge and use ← and →; the width is kept
for the next time.

**Info** follows the selection. Select one element in a view and it opens at the foot of the side
panel; with no panel open, it is the whole side panel. Its title row has two tabs, **Generic** and
**Placement**; the tab you chose stays open as the selection changes.

For one element, **Generic** edits its **Name** and **Notes** and, for a lamp, its **Patch** as
`universe.address` — one field per split, and an empty field unpatches it. **Placement** edits its
**Position** (X, Y and Z in metres) and **Rotation** (X, Y and Z in degrees). A generated Venue object —
a truss, a curtain, a stage element — shows its **Size** instead of a scale: only the measurements its
profile lets you set, such as a truss's width (its length), a curtain's width and height or a stage
element's height, in metres within the range the profile allows. A size outside that range is put
back rather than written. A stage element's or stairs' **Position** is where its feet stand — the
middle of its footprint on the floor — so Z 0 stands on the stage floor, and changing its height
raises or lowers the deck without moving the feet. Every other generated object is positioned by
its centre. Its **Parameters** follow: a **Colour** as a hex value, empty for the kind's
default, and for a chain its **Chain top** and **Chain bottom**. A placed 3D model shows **Scale**. A
lamp shows its **Bracket angle** and **Barndoors** angle, empty when none are fitted. Every number
field names its unit.

An element keeps the exact version of its profile it was placed with, so a show always draws what it
drew when it was built. When this computer's library holds a newer version of that profile — because
a shipped part has been corrected since, such as a truss section or the size of a corner block —
**Placement** says so and offers **Update to the newest version**. It changes nothing else: the
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
value to set every element, or a range — `1 THRU 5`, `1 … 5` or `1 ... 5` — to spread it evenly from the
first selected element to the last. **Placement Assistant** lays the selection out in the order it was
selected: along a **Line** from a start to an end, in a **Grid** from a start with a number of columns
and a spacing across and deep, or around a **Circle** from its centre, radius, start angle and arc,
where 360° spaces the elements evenly around the whole circle. Nothing moves until **Apply**.

When every selected element was patched from the same model, **Shared model** follows the placement
fields and names it — `All 4 × Generic Stage Element 2 × 1 m`. It carries that model's own controls:
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
the copy follows the pointer and the original stays where it is. The copy is decided once the key
has been held at any point in the drag, or at the moment you let go, so letting the key go before
the mouse still places it; a drag with the key never held moves the selection as usual. A copy is a
new element like one from **Duplicate** — its own identity, the next free number, unpatched — it
becomes the selection, and one **Undo** takes it away again. Option-dragging anywhere but the
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

While you drag, the gizmo shows where its origin is on the two axes the view shows — X and Y in a
plan, the horizontal axis and Z in an elevation — in metres, the same figures as the Info panel's
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
  overall wherever it has arms, so adding arms makes it busier rather than bigger. Connectors join
  only within one truss system: three-point to three-point, four-point to four-point.
* **Stage elements** — decks on scissor lifts, decks on regular feet and stairs — butt their sides
  against a neighbour's and line them up flush with its sides, each direction on its own: slide a
  deck along the front of a wider one and it closes up against it wherever you let go, and its end
  clicks into line with the wider deck's end when it comes close. Two sides that meet on both axes
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
  its profile declares — the hook clamp on its yoke, the omega bracket under a moving head — so a
  short lamp catches a pipe from the distance its real hardware reaches and no further. A fixture
  whose profile says it hangs from nothing, such as a hazer or a floor can, is never picked up by a
  truss. See **Mounting** in the fixture profile editor to set the clip on a model you imported.

Only the axes the drag can move change, so a plan view never changes heights, and a truss, pipe or
rail more than half a metre off the view's plane — a truss 6 m above a lamp on the floor, seen from
above — is out of reach. **Measure** snaps its ends in the same way, onto connectors, stage corners,
curtain ends and the centre of every object. Hold **Shift** to place freely: while it is held, a drag
or a measurement snaps to nothing and a moved lamp is not mounted.

The trash button at the top of Info, **Delete** and **Backspace** delete the selection from the
show, with every multi-patch copy of each fixture. A single selected element goes at once; several
are always confirmed first, and **Cancel** leaves every one of them in place. A deletion is one step:
**Undo** in the CAD title, or ⌘Z, brings back everything it removed — same fixtures, numbers, patch
and place — and **Redo** deletes it again. While a field has the focus, Backspace edits the field,
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
  **Objects**, described under [Groups](#groups). **Group** shows when two or more selected Venue
  elements are not already exactly one group, **Ungroup** when the selection touches a group.
* **Duplicate** adds a copy of each selected element half a metre to the right of it, as the viewport
  you opened the menu in shows right. A copy is a new element with its own identity and the next free
  fixture (or virtual) number above its original's, keeps its profile, mode, size, rotation and name,
  and is not patched — give it a DMX address in Info when it needs one. The copies become the
  selection. **Undo** takes them away again.
* **Delete** is the trash button's delete: one element goes at once, several are confirmed first,
  and **Undo** brings the elements back.

A multi-patched fixture stands in the plan once for each copy. Click a copy and Info edits that copy
alone: its name, position and rotation change, and the fixture and its other copies stay where they
are. **Copy** at the top of Info switches between the original and each copy, named by its DMX
address. Notes and scale belong to the fixture, so their labels say **(all copies)**. Dragging in the
plan still moves the fixture with all its copies.

## The grid

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
  you click. Double-click, press Enter or right-click to finish an open line — a right-click adds no
  point where the pointer is; click the first point again to close it.
* **Draw box**: click one corner, then click the opposite corner.
* With snapping on, a line's and a box's points snap: onto a corner of a truss (its box or a
  connector) or of a stage element in reach; otherwise a line's next point that is nearly level with
  or plumb over the last one is made exactly horizontal or vertical; and every coordinate left free
  lands on whole 10 cm. Hold Shift to draw exactly where the pointer is.
* **Place text**: click where the text starts, type, and press Enter. Text is sized for the zoom it
  was placed at and grows and shrinks with the plan.
  With **Select** in hand, click placed text to pick it: it is outlined in cyan and picked on its
  own, so the element drawn under the words stays unselected. Drag the words to move them, or use
  the gizmo that stands on the text's start — the square moves it freely, an arrow along one axis.
  While text is picked, **Info** shows its **Text**, its **Height** and its **Position** — X and Y on
  a plan, **Across** and **Height** on an elevation — and typing a value there moves or rewords it.
  Every move or change is one step **Undo** puts back, and it is kept with the show. Click anywhere
  else to put the text down.
* **Measure**: drag from one point to another. The measurement is drawn in amber with a tick at
  each end and its distance — in millimetres below a metre, in metres above. With snapping on, each
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
| **⌘G** (Ctrl+G) | **Group** the selected Venue elements |
| **⇧⌘G** (Shift+Ctrl+G) | **Ungroup** the selected groups |

The project paperwork printed on every page is no longer a CAD panel: set it under **Show
information** on the **Show** screen.

## Objects in the Elements panel

**Objects**, the second tab of **Elements**, lists what the venue is built from apart from the lamps:
**Venue items** — every shipped Venue object, generated like trusses, curtains and stage elements or
drawn from a model like truss corners, decks on legs and the disco ball — and **3D models**, which
lists only the models imported into this show. Each row names the object, its ID, its kind and its
size in metres.

### Groups

Select two or more Venue elements — the trusses of one rig, the decks of a stage — and press
**Group** at the top of **Objects**, ⌘G, or **Group** in the right-click menu of a viewport. The
group is saved in the show as **Group 1**, **Group 2** and so on, and is listed under **Groups**
above the other objects; its elements move there from the lists below. **▸** opens a group to show
its members.

A click selects the whole group: its row, one of its members in the list, or any of its elements in a
view, and so does a selection rectangle that catches one of them. Hold **Shift** to take elements one
at a time instead — Shift-click in a view adds or removes just that element, and Shift-click on a
member in the list selects only that member. A selected group moves together with the gizmo, like any
selection. Every element of a selected group is drawn in violet, in the CAD views and in the 3D
PreViz alike, while an element selected on its own — including one Shift-picked out of its group —
keeps the ordinary blue selection colour, so a whole group always reads apart from a single piece. **Ungroup** — at the top of **Objects**, in the right-click menu, or ⇧⌘G — dissolves
every group the selection touches and leaves its elements where they are. An element is in one group
at most: grouping it again takes it out of its old group. A deleted element leaves its group, and a
group with no element left goes with it. **+** adds more: the same truss, stage element, curtain,
primitive and venue element as the title, and **Import 3D model…**, which places a GLB, glTF, 3MF or OBJ file as described below and selects it. Select a row to select the object in the
views; **Info** below then sets its position, rotation and scale.

## Place your own venue models

With **Show all** on, the **Patch** sheet's title bar has **+ Import 3D model** beside **+ Add fixture**, for a venue
the fixture library does not have: the hall you are playing, a stage build, a set piece. Choose a
GLB, glTF, 3MF or OBJ file on this computer and it is placed at once, at the stage origin, on the
layer that is open — or the default layer when **All fixtures** is. It gets the next free `0.x` ID
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
