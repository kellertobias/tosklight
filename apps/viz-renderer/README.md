# ToskLight Visualizer

The standalone, network-connected live visualizer. It runs as its own process with its own window,
its own connection to a scene source, and its own Art-Net and sACN receivers.

Building or opening ToskLight never builds this application, and the desk never needs it to run.
If the visualizer crashes, is uninstalled, or is never installed, the server, output engine, and
desk user interface are unaffected.

## Running it

```sh
npm run open:viz
```

That launches the latest visualizer build **with the latest Viz editor beside it**. Use `npm run
build:viz:open` to rebuild every helper first. With no source named it opens that editor as its
planning window and draws the rig you patch there. Name a desk, a show file or the built-in scene
and it does that instead — and for a desk it starts the latest built development server if nothing
is answering on `http://127.0.0.1:5000`, using a desk that is already running as-is and never
stopping it. Arguments pass straight through:

```sh
npm run open:viz -- --server 10.0.0.9 --port 5000
```

To build without launching anything:

```sh
npm run build:viz
```

On macOS that build also assembles `ToskLight Visualizer.app` around the binary, and `npm run
open:viz` launches the executable from inside it. A bare Mach-O process gets the generic executable
icon and is named after its binary in the Dock and the application menu, so the bundle is what makes
the visualizer look like the product it is. Its icon is the Viz icon — the ToskLight mark badged
"3D" — shared with the editor. Elsewhere the binary is the product and needs no wrapper: the window
and taskbar icon are set from the same artwork at runtime.

| Option | Meaning |
| --- | --- |
| `--server <host>` | Lighting-desk host or IP address. Default `127.0.0.1`. |
| `--port <1-65535>` | Lighting-desk API port. Default `5000`. |
| `--user <name>` | Desk user for the read-only visualizer session. Default `Operator`. |
| `--target <name>` | Which renderer the desk is addressing. Default `main`. |
| `--demo` | Render the built-in deterministic scene without connecting to anything. |
| `--view <name>` | Start in a named view: `top_down`, `left_to_right`, `right_to_left`, `front_to_back`, `back_to_front`, `lines_3d`, `simple_3d`, `full_3d`. |
| `--quality <tier>` | `draft`, `standard`, `high`, or `ultra`. |
| `--theme <name>` | `light_on_dark` or `dark_on_light`. |
| `--ambient <pct>` | How bright everything that is not a light source is, so trusses stay readable with the rig dark. |
| `--fog <pct>` | Haze amount to render with (default 50). |
| `--exposure <x>` | Operator exposure trim, `0.05`–`4.0`, on top of the automatic adaptation. |
| `--capture <path>` | Render, write one PNG, and exit. Used for golden images and benchmark evidence. |
| `--capture-frames <n>` | Frames to let the scene settle before capturing. Default `60`. |
| `--snapshot` | Take one snapshot once the scene has settled, print where it went, and exit. |
| `--blender <path>` | The Blender to export snapshots with. Found automatically otherwise. |
| `--verify` | Open the window, present a frame, and exit. Build-machine smoke check. |
| `--preferences <path>` | Keep this window's settings here instead of in the operator's application-data folder. |

The first launch needs no network configuration when ToskLight runs on the same computer.

Effect fixtures run their package-owned `effect.js` in isolated bounded QuickJS contexts. Particle
budgets are Draft 128, Standard 512, High 2,048 and Ultra 8,192; overload retains one particle per
active nozzle before distributing the remaining capacity and reports requested/drawn counts in
renderer frame statistics. See the operator manual's **Particle Effects** page for the script and
restart contract.

## The three ways it starts

| Started | What happens |
| --- | --- |
| **From a lighting desk**, or with `--server`/`--port` | Connect and visualize. Nothing else opens. |
| **With `--show <path>`** | Start a private server on that file and visualize it. No planning window. |
| **On its own**, with nothing named | Open the **Viz editor** beside the window. Choose or build a rig there and it appears here. |

A launch that names nothing has nothing to draw and no way to be told what to draw, so it opens
somewhere to decide that rather than an empty picture. `--demo` overrides all three.

The desk sets `TOSKLIGHT_VIZ_LAUNCHED_BY=desk` when it starts the visualizer itself, so a
desk-owned window never opens a planning surface the operator did not ask for.

The planning window is a separate application ([`apps/viz-editor`](../viz-editor/README.md)) and
stays one. **Source** in Quick Settings switches between the lighting desk and that planning
document at any time; choosing the planning source opens the editor if it is not already open. If
the editor cannot be started the visualizer says so on the status surface — in the words that name
what is missing — and keeps running. `TOSKLIGHT_VIZ_EDITOR` names the editor binary for a
development tree, where the two are not installed together.

Closing the planning window, or the private server behind an opened show file, is noticed within a
second and named on the status surface rather than left looking like a slow connection.

## Pointing a local server at a show file

The visualizer reads whatever show the desk has active. To bring up a server on a specific show
file and then look at it:

```sh
cargo run -p light-headless -- --data-dir .artifacts/runtime/light-data --show assets/demo.show
```

`--show` opens that file, registers it in the desk library under its own name, and makes it the
active show.

## Camera, keyboard and mouse

| Key | View |
| --- | --- |
| `1` | 3D |
| `2` | Top down |
| `3` | Front to back |
| `4` | Left to right |
| `5` | Right to left |
| `6` | 3D Simple |
| `7` | Back to front |
| `8` | 3D Lines |

| Input | Action |
| --- | --- |
| Right drag | Pan and tilt the camera on the spot (pans the plan views across the floor) |
| `Shift`+right drag | Pan: move the view across the stage floor |
| Middle drag (wheel pressed) | Move the camera on the camera plane |
| Wheel | Zoom in and out |
| `W` `A` `S` `D` | Walk the camera on the floor plane, facing where the camera points |
| `Enter` | Open and close **Quick Settings** |
| `Space` | Hide every overlay, or show them again |
| `T` | Switch light on dark and dark on light |
| `L` | Switch the plan labels on and off |
| `R` | Request a fresh scene snapshot |
| `Cmd`/`Ctrl`+`S` | Keep this moment as a **snapshot** |
| `Esc` | Close the window (or cancel Quick Settings while it is open) |

The left button is not a camera control. Dragging with it moves nothing; it belongs to the status
surface, where it opens Quick Settings and **inspects a fixture**. Clicking a fixture names it in
the status bar with its number, its patch address and its current level; clicking away from every
fixture clears that again. The selection is held by identity, so a rig repatched underneath it
either still has that fixture or no longer does — it never quietly becomes a different one.

A right drag turns the camera the way someone standing in the room turns their head: the camera
stays where it is, a drag to the right turns it to the right, and a drag downwards tilts it down.
Together with `W` `A` `S` `D` that is the whole of walking the rig — go somewhere, look around.

Panning moves the picture with the hand, parallel to the stage floor, and covers more ground the
further the view is zoomed out. `W` `A` `S` `D` walks the camera instead, facing where it points.
Neither changes height.

An orthographic view keeps its exact axis whatever the operator does: a plan view can be panned and
zoomed, and nothing can turn it into a slightly crooked one. A plan view has no heading to turn, so
a right drag pans it — which is what `Shift`+right drag does in the 3D views as well.

Inside Quick Settings the arrow keys move and adjust, `Enter` activates, and `Esc` cancels without
disturbing the current connection.

## The plan views

The six orthographic views draw a stage plot: outlines only, no shading and no bloom, with each
fixture as a symbol for its body kind — a circle and stalk for a moving head, a rectangle with a
diagonal for a lantern, a box for a bar or matrix. A symbol keeps its size on screen however far
the plan is zoomed out, and a lit fixture takes the colour it is emitting.

`L` shows the fixture number and its patch address (`universe.address`, or `unpatched`) beside each
symbol. A label that would collide with one already placed is dropped, so the page stays readable;
zooming in makes room and the rest appear.

`T` switches between light on dark and dark on light. Dark on light is ink on paper, for printing
and for working in a lit room.

## Following the desk

The desk decides which way this window is looking. **Running & Output** on the desk carries the
eight named views and the rendering quality; selecting one is an instruction, and this window
obeys it — the view changes and the camera goes with it, framed on the rig.

Between instructions the operator here is in charge: turn, walk, zoom, press `1`–`8`, and none of
it is taken away until the desk sends something new. A quality held locally in Quick Settings is
the exception — it reads `(local)` on the status bar and the desk does not replace it until it is
set back to **Follow source**.

A desk driving two windows keeps a view for each. This one follows `main` unless it was started
with `--target <name>`, so a second window started with `--target front-of-house` can hold the
plan while this one holds the beams. A desk with nothing to say — including one older than the
view — leaves whatever is selected here alone.

## Keeping up with a rig that changes

The desk publishes what changed, and the visualizer subscribes to the show and to the desk's own
configuration. Both halves of that matter: a desk delivers events to a subscriber rather than to
whoever opens the socket, and it names the change inside a typed envelope. The planning window
sends the same names on their own, and both are read.


A fixture patched, moved, renamed or repatched while the visualizer is running is applied where it
stands. The scene is re-read over the connection that is already open: the session stays up, the
Art-Net and sACN sockets stay bound unless the show actually moved a universe somewhere else, and
every head that still exists keeps the level and colour it is being sent. A rig edited during a
show does not blink.

Only a change of show goes the long way round. Its values belong to fixtures that no longer exist
and its universes may now mean something else, so the new show is staged whole and replaces the
old one when it is complete.

## The two input planes

Scene and configuration come from the desk API. **Every live value comes from real Art-Net or sACN
packets.** The visualizer never asks the desk for output values, so changing the programmer or a
playback with no output route configured correctly shows nothing: the fixtures stay at their
defaults and the status line says **Waiting for DMX**.

## Network setup

The visualizer listens on the destinations the show's own output routes describe.

- **Art-Net** listens on UDP `6454`; **sACN** listens on UDP `5568`.
- **Same computer.** The portable default is explicit loopback unicast: point the desk's output
  route at `127.0.0.1:6454` for Art-Net or `127.0.0.1:5568` for sACN. This still traverses the
  operating-system network stack, so it exercises the real encoders and receivers, and it does not
  depend on whether the platform reflects broadcast or multicast back to a local receiver.
- **Broadcast and multicast** are supported for normal network operation. sACN multicast joins
  group `239.255.<high>.<low>` for the destination universe on the selected interface.
- **Two computers.** Configure the desk's route to the visualizer machine, or use broadcast or
  multicast on a shared lighting network.
- **Firewall.** Allow inbound UDP `6454` and `5568` on the machine running the visualizer. On
  macOS the first launch prompts for incoming connections; accept it or the receivers bind but
  never see a packet.
- **Another local receiver.** Sockets are opened with address and port reuse, so the visualizer can
  share a port with another Art-Net or sACN consumer on the same machine.

If a show configures no output routes at all, the visualizer listens on the Art-Net and sACN
defaults for every universe the show actually uses and says so in its diagnostics.

## What the status surface shows

The application mark sits in the top left; clicking it opens Quick Settings.

**Bottom left** carries one badge per universe with that universe's own frame rate, and the render
latency beside them. A badge is

- **green** while the universe is arriving cleanly;
- **orange** after a frame-rate drop or any broken frames in the last thirty seconds; and
- **red** once the rate falls below 20 Hz, or more than 20% of one second's frames were broken
  inside the last thirty seconds.

The second row names the connection and its address, and says **Waiting for DMX** when the scene
has loaded but nothing is arriving. A snapshot taken or a Blender file written is confirmed here
and gone again in a few seconds. When the bar is too narrow for everything, the latency shortens
and the badges shrink before any of them is dropped.

**Bottom right** carries the fixture and head counts, the live-beam count, and what is on screen:
`2D <view>` or `3D <view> <quality>`. A quality this renderer is holding locally rather than
following the source reads `3D Full High (local)`. The second row carries the frame rate, the fog,
the exposure trim, and the ambient level.

**Bottom middle** carries the shortcuts worth having in front of you:

```
Cmd+S snapshot  •  Enter settings  •  Space overlays  •  1-8 views
```

`Ctrl+S` away from macOS; the bar names the key the platform uses. This is not a keyboard
reference — the table above is — it is the handful an operator reaches for without thinking, and
the snapshot leads because it is the one that keeps a look which is about to be gone. The
shortcuts have the last of the bar's room and give it back first, one at a time from the right,
so a narrow window or a long confirmation shortens the list instead of crowding anything. The
snapshot is the last one standing.

When the source has answered but the show has no fixtures in it — a planning document nobody has
patched yet — the bar says so and names where to fix it, because a window with nothing in it and
nothing to say for itself reads as a broken visualizer.

**The wheel adjusts a value under the pointer.** Hovering the fog, exposure, or ambient readout and
rotating the wheel changes it without opening anything. `Space` hides every overlay for a clean
picture, and shows them again.

Quick Settings names the connection, the show and its revision, and under them the GPU and backend
that are drawing, what the GPU itself spent on a recent frame where the adapter can time one, and
how fast values are arriving. It lists every configured input mapping with its protocol, universes,
delivery mode, bind address, health, and its accepted, duplicate, malformed, and out-of-order
packet counts. A
show that routes one universe over more than one route is working correctly, and the copies are
counted as duplicates rather than inflating the universe's frame rate.

## What a fixture's light looks like

Two lanterns pointed at the same spot, at the same angle, at the same level, do not look alike. A
profile lays down a flat disc with a rim you could cut paper on; a PAR is hot in the middle inside a
soft halo; a flood has no rim at all. Four numbers on each head carry that difference, and every
desk control — dimmer, zoom, iris, focus, frost, shapers — is applied on top of them.

| | What it decides |
| --- | --- |
| **Output** | How much light the engine makes. A 400 W head against a 100 W one, before anyone touches a dimmer. |
| **Beam angle** | The cone, from the fixture's narrow and wide figures and wherever its zoom sits between them. |
| **Sharpness** | How hard the rim is: `1` cuts, `0` blends away with no edge to speak of. |
| **Uniformity** | How the light inside the field is spread: `1` flat to the rim, `0` a bright centre that falls away fast. |

Sharpness and uniformity are separate on purpose. A good LED wash has no rim and is still even
across the middle; a PAR is hot in the centre and has faded long before its own rim.

Falloff is not a fifth number — it is what the other four produce. Intensity is flux spread over a
cone, so opening the field thins the same light across more of the stage, and the shaft fades along
its throw at the square of the distance from the lens. A beam light throws across a room; a flood
of the same engine lifts an area and gets nowhere near as far.

Until a profile carries its own figures, these come from the fixture type it declares:

| Class | Rim | Field | Cone |
| --- | --- | --- | --- |
| Beam | hardest | flat | 3–8° |
| Profile, spot, ellipsoidal | hard | flat | 10–32° |
| Scanner | hard | flat | 8–16° |
| Fresnel | soft | mild hot spot | 12–48° |
| PAR, ACL | softer | strong hot spot, oval lens | 10–26° |
| Wash, LED wash | none to speak of | even | 18–55° |
| Flood, cyc, ground row | none | even, very wide | 45–90° |

A focus or frost channel softens whatever the fixture starts from, so a profile out of focus reads
as a profile out of focus and never as a wash. Frost also evens out what it softens, because a
diffuser fills in a hot centre as it destroys the edge.

## Beam optics

A shaft leaves a lens, not a point, and it is brightest where it leaves.

The lit surface belongs to the fixture, not to one patched instance: every Source Four of the same
type has the same lens. It has a form — round, oval, or rectangular — and a width and a height, so
a point-source spot can have a small round one and a cyc flood a wide rectangular one. A lens is
kept inside the body that carries it, and inside the spacing between an emitter's cells, so the
cells of a bar keep their own beams instead of merging into one smear. The shaft is then the
frustum of a cone that converges some way behind that lens: just behind the face of a wide wash,
several metres back for a tight beam.

In-scattering falls with the square of the distance from that virtual apex. That is what makes a
wash fade quickly and a beam hold together down its throw, and it is what keeps the bright end of
a shaft at the lamp: a cone widens as it travels, so a view ray crosses more and more of it, and
any flatter falloff makes the far end of the beam the bright end, which is backwards.

Everything a head puts in front of its lamp is rendered in one place, so a pattern appears on the
floor and in the shaft of haze at the same time and from the same geometry:

- **Zoom** narrows the cone and brightens it, because the same light is going through less of it.
- **Iris** narrows the cone and leaves the brightness alone. That is the whole difference between
  an iris and a zoom, and the visualizer keeps it.
- **Focus** is sharp in the middle of its travel and soft at either end, the way a lens moved
  either side of the gate behaves.
- **Frost** widens the field, destroys the edge, and stops a gobo holding its shape.
- **Gobo** and **gobo rotation** project a pattern that turns with the wheel and with the head.
- **Prism** deviates the beam into one copy per facet, arranged around the axis and turning with
  the wheel. Each copy carries whatever is in the gate — a gobo appears in every one of them — and
  a facet passes nothing outside its own copy, which is what makes a prism read as several beams
  rather than one wider one. Frost fills the gaps back in, because a diffuser in front of a prism
  does exactly that.
- **Shapers** cut the beam with four framing blades, and the whole module turns.

A profile that carries a gobo wheel projects its own glass: the artwork travels inside the fixture
package, arrives with the scene, and is sampled in the gate, so the pattern on the floor is the
one that profile says it has. The shipped moving heads carry wheels; a profile that declares none
keeps the drawn patterns — the right character of pattern in the right slot, turning at the right
rate, rather than a particular manufacturer's glass.

## How the rig actually hangs

Two things about a fixture are set by hand at the rig and cannot be driven from any desk, so the
patch records them and the visualizer draws them:

- **Bracket angle** — how far the fixture is tilted in the clamp or yoke it hangs from, positive
  nose-down. It turns the fixture about its own transverse axis *after* its mounting rotation, so a
  lantern faced across the stage and then angled down in its bracket points where both of those
  say.
- **Shaper angle** — how far a fitted shaper or barn-door module is turned. A framing module the
  desk can also rotate over DMX starts from this angle rather than replacing it, and a barn door,
  which nothing turns but a hand, only ever has this one. A fixture with no module fitted has no
  angle at all.

Both are edited in the desk's Patch sheet, or in the Viz editor's, under **Bracket** and
**Shaper**.

## Fixture models

A fixture is drawn from its library model when its profile carries one. The desk already sends
each patched fixture's profile snapshot, and a package stores `assets/model.glb` inline, so the
model arrives with the scene — the visualizer does not fetch anything extra and does not read the
fixture library from disk.

The reader takes triangles, normals, node transforms and base colours, and nothing else. Node
names decide what moves: a node named for the head, the lamp or tilt follows pan and tilt; one
named for the yoke, an arm or pan follows pan; everything else is the base. A model authored at
another size is scaled to the profile's physical dimensions so a rig never mixes lamps drawn at
different scales.

A model that cannot be read never leaves a hole on the stage: the fixture keeps its procedural
proxy and the reason is listed in the scene warnings. The same is true of a fixture whose profile
carries no model at all, which today is every fixture in `assets/fixture-library` — none of the
shipped packages carries geometry yet, so the rig is drawn from proxies until they do.

## Snapshots, and rendering one in Blender

`Cmd`+`S` — `Ctrl`+`S` away from macOS, or **File → Take Snapshot** — keeps the moment that is on
screen. Nothing is asked and nothing opens: the look worth keeping is usually about to be gone, and
a dialog would take it. The status line says the capture was taken, and again when it is written.

A snapshot is the rig frozen: every fixture where it hangs, every head where it was pointing, every
colour and level the desk had it at, the trusses and the deck around it, and the camera the operator
was looking through. Only the copy happens on the frame — writing it out happens on a worker — so
taking one on a three-hundred-fixture rig costs no frame.

It is a folder of its own:

| | |
| --- | --- |
| `rig.glb` | Every body, lamp face, truss and piece of scenery, in metres, where this frame put them. Ordinary glTF 2.0: anything opens it. |
| `snapshot.json` | What triangles cannot say — one spot light per emitting head with its aim, cone, rim, colour, level and power, plus the camera, the haze, and the show it came from. |
| `rig.blend` | Written by the export below. Absent until then. |

Captures go to the platform's application-data folder — `~/Library/Application Support/ToskLight/
Visualizer/Snapshots` on macOS, `%APPDATA%\ToskLight\Visualizer\Snapshots` on Windows,
`~/.local/share/tosklight/visualizer/snapshots` elsewhere — and Quick Settings says which. Set
`TOSKLIGHT_VIZ_SNAPSHOT_DIR` to put them somewhere that is already backed up. The last twelve are
kept; taking a thirteenth drops the oldest. Anything else in that folder is left alone.

### Exporting one to Blender

The last snapshots are listed in **Quick Settings**, newest first, by the time they were taken.
`Enter` on one exports it: Blender is run in the background, builds the scene, and writes
`rig.blend` beside the capture. The row says what it is doing and what happened, and so does the
status line.

Only Blender can write a Blender file, so this step needs Blender installed — and that is exactly
why it is a separate step. Capturing needs nothing: an operator keeps the moment on whatever machine
is beside the stage, and makes the file later on whatever machine has Blender on it.

It is looked for in `/Applications`, in `Program Files`, and on the command search path. The
**Blender** row in Quick Settings says which one was found, or that none was, and a path typed there
is used instead — for a machine with several versions, or one that keeps it somewhere unusual. The
`TOSKLIGHT_BLENDER` environment variable and `--blender` do the same for a scripted run.

What the exported file contains:

- the rig, in metres, Z up, at the placement and the pan and tilt of that moment;
- one spot per emitting head, aimed where the desk aimed it, with its cone, its rim softness, its
  colour, and a power that follows the fixture's own output and its zoom — a beam light is brighter
  than a flood of the same engine, exactly as it is in the picture;
- a box of hazed air over the rig, because a beam is only visible in air that has something in it;
- the operator's camera, and Cycles set up to render it.

The geometry, the placement, the aim and the colour are the desk's and are exact. The haze density,
the exposure and the sample count are a *look*: they sit as named constants at the top of the import
script written beside the capture, and everything downstream of them is ordinary Blender data to be
edited by hand. What does not come across is named in `snapshot.json`: gobo artwork, prisms and
framing shutters are this renderer's own optics, so a beam arrives as its cone, its colour and its
level.

A lamp's light is placed just outside the body that carries it rather than at its middle. The
visualizer draws a shaft from the lens and never asks what is in the way; a renderer that traces
light does ask, and a spot left inside its own head lights nothing at all.

## Shadows

`3D Full` renders real shadow maps. The budget is per quality tier — 3 at Standard, 6 at High, 10
at Ultra — and goes to the brightest beams, because those are the shadows an operator notices.
Both the surface pass and the volumetric pass sample them, so a truss standing in a beam casts its
shadow through the haze as well as onto the floor. A light with no map is drawn unshadowed rather
than dark.

## Fog and ambient light

Haze is the renderer's own **Fog amount**, `50%` until you change it. It is never taken from the
show: a hazer's DMX output says how hard the machine is working, not how thick the air in the room
became, and following it swings the picture between an invisible rig and a milky one on a value
nobody is watching. Patched hazers still appear in the rig and their output is still decoded; they
just do not set the density.

Set the amount in **Quick Settings → Fog amount**, with the wheel over the fog readout, or with
`--fog <pct>` at startup. `0%` is clear air even with every hazer running flat out. The amount is
renderer-local: it never touches the show.

**Ambient** sets how bright everything that is not a light source is, so the trusses and the floor
stay readable with the whole rig dark. It is held at a constant screen brightness: a rig full of
beams pulls the automatic exposure down, and the ambient level is not allowed to go down with it.

## Settings that stay set

Fog, exposure, ambient, quality, appearance, plan labels, the connection fields and any input
overrides are kept between launches, beside the operator's own application data:
`~/Library/Application Support/ToskLight/Visualizer/preferences.conf` on macOS,
`%APPDATA%\ToskLight\Visualizer\preferences.conf` on Windows, and
`$XDG_CONFIG_HOME/tosklight/visualizer/preferences.conf` elsewhere. `--preferences <path>` and
`TOSKLIGHT_VIZ_PREFERENCES` put them somewhere else, which is how a test keeps out of an operator's
own configuration.

`Space` is not among them. Hiding the overlays is a gesture for looking at the picture now, and a
window that opens with no status surface, no connection state and nothing on screen is
indistinguishable from a broken one — so every launch starts with the overlays on.

They are settings of this window on this machine and never travel into a show or a planning
document. Anything named on the command line is what this launch was asked for and wins over what
was stored; a file that cannot be read costs the convenience, never the launch.

## Rendering quality

The tier decides the volumetric step count, the shadow budget, whether bloom runs, how much the
haze varies through the room, and the resolution the shaded passes are drawn at — `Draft` renders
at three-quarter scale and the composite samples it up to the display. The overlays and the plan
views are always drawn at the display's own resolution, so a cheaper tier never softens the type or
the lines on a stage plot.

## Anti-aliasing

The shaded passes are multisampled: four samples per pixel where the adapter offers it, two where
it offers that instead, and one — drawn plainly, and said so — where it offers neither. The count
is not a tier setting. It is decided once from what the GPU can do, because it is baked into every
pipeline and into the depth the beam pass reads, and because an aliased edge is not a quality
choice anyone would make. Quick Settings names it beside the GPU: `Apple M5 Max (Metal, 4× MSAA)`.

The beams resolve with the geometry rather than on top of it, so a shaft crossing the edge of a
truss is one edge and not two. The plan views gain the most: a stage plot is diagonal lines on a
page, and they were the thing that stair-stepped worst.

`TOSKLIGHT_VIZ_SAMPLES=1` turns it off for a benchmark that wants the two halves compared; any
other count is used where the adapter supports it.

## Frame pacing

The window presents one frame per display refresh, and it works out when that refresh is due from
how long each frame waited for the display to hand back a drawable.

This is not a frame-rate limiter. The drawable is only released on the refresh, so a frame started
any earlier than that spends the difference blocked inside the graphics driver — on the same thread
that delivers mouse and keyboard events. Presenting frames back to back leaves that thread with
almost nothing for input, and a drag then arrives seconds after the hand made it. Waiting for the
frame to fall due instead spends that time in the window system, where input is delivered, and
costs no frame rate: the display still gets a frame every refresh.

`--benchmark` reports both halves. `cpu p95` is the work, `wait p95` is what was spent waiting for
the display; a healthy `wait` is a millisecond or two, and a `wait` near the whole frame interval
means the loop is starving its own input.

## Layout

- `crates/viz/scene` — the semantic scene, live values, view configuration, and the provider trait.
- `crates/viz/dmx` — Art-Net and sACN receivers and packet decoding.
- `crates/viz/project` — DMX to fixture-parameter projection using the shared fixture library.
- `crates/viz/desk` — the lighting-desk provider.
- `crates/viz/render` — the `wgpu` render core.
- `crates/viz/snapshot` — freezing a rig, writing it as glTF, and exporting it with Blender.
- `apps/viz-renderer` — this application: window, input, Quick Settings, diagnostics.
