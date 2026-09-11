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

A show has a name of its own, separate from its file. The **Show** page shows it over the rig
overview, with a pencil beside it. Press the pencil, type the new name, and press Enter or click
away to keep it; Escape leaves the name as it was, and an empty name changes nothing. The name is
what a desk's **Load Show** menu offers, so a renamed show is offered under its new name at once.
The file keeps its name; **Save As** writes the show to a new one.

## Editing the patch sheet

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

## Configure live DMX inputs

The **Show** screen's **Live DMX Inputs** section maps a logical show universe to the Art-Net or
sACN universe the separate Visualizer output receives. Each mapping can be enabled or disabled
and carries its protocol, wire universe, delivery mode, and UDP port. Art-Net offers Broadcast or
Unicast; sACN offers Multicast or Unicast. Choose **Apply** to store the mappings in the portable
show, or **Cancel** to discard the draft. The receiving machine's network-interface choice stays
local to that machine and is never written into the show.

When a desk is detected, **Take from Desk** reads that desk's compatible output routes through a
read-only Visualizer session. With more than one desk, first select the source. The imported routes
are only a preview until **Apply** is chosen: taking routes does not replace the show and does not
change the desk. An explicit show input wins over a derived output route for the same logical
universe; a renderer-local override remains the final authority.

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
