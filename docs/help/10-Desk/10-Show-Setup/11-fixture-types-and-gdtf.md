# Fixture Types and GDTF

The fixture library is desk-wide and persists independently of show files. Open **Desk Setup > Shows & recovery > Open Fixture Library** to launch its modal and search, import, create, revise, and inspect complete fixture profiles. Library search follows the shared [search-bar layout](../30-Windows/01-desk-interface-and-windows.md#search-bars) and filters automatically with every typed character. Its optional Options dialog selects the fixture type. A profile is one revisioned fixture containing Generic information and an ordered set of modes; a patched show embeds the selected profile revision and mode so later library edits or deletion cannot change that show.

![Fixture-library manufacturers, modes, footprint, heads, and revision](../../assets/screenshots/workflows/fixture-library.png)

The shipped library includes separate conventional **Dimmer PAR Can**, **Dimmer Profile**, and **Dimmer Fresnel** fixture profiles, each with 8-bit and 16-bit dimmer modes. Shipped control fixtures leave body geometry to the renderer-owned default models, so the desk, demo show, Visualizer, and CAD all choose the same PAR, elongated profile, Fresnel with barn doors, moving-head, strip, laser, or effect body from fixture semantics. Portable visual-only Venue and Rigging objects are either generated at the size they are placed — a truss, a stage deck, a curtain — or keep their exact GLB geometry, because a railing or a mirror ball cannot be represented by a generic lamp body. Choose the fixture profile for the physical lantern rather than treating these appearances as modes of one Dimmer profile.

## Transferable fixture packages

ToskLight has no fixture definitions compiled into the application. Every fixture supplied with the desk is an ordinary `.toskfixture` package, loaded through the same package reader used by **Import fixture**. You can export it, move it to another desk, keep it with a test, unpack and edit it, or replace it with a corrected package without rebuilding ToskLight.

Select a fixture and choose **Export fixture** to download its complete immutable revision. On another desk, choose **Import fixture** and select that file. A package keeps the stable fixture, mode, head, channel, function, split, and geometry IDs. Importing identical content is a no-op; importing changed content with the same fixture ID and manufacturer/name creates the next local revision. Reusing an existing ID for a different fixture family is rejected.

The shipped package directory currently provides an operator-focused Generic family and these manufacturer profiles with complete ordered mode lists:

- **Generic ACL** — a compact 200 mm-long, 80 mm-diameter conventional ACL lamp with aligned lens and beam geometry. **Blinder 2**, **Blinder 4**, and **Blinder 8** are three fixtures, because a two-lamp bar and an eight are different lanterns rather than two personalities of one. Each has the groupings its lamp count allows — one and two channel, and on the eight also four — and each dimmer channel owns a non-master logical head that drives its share of the lamps. Shows patched before the split keep the fixture they were patched with. **Fogger** provides Fog, Fan/Fog, and Fog/Fan modes; **Hazer** provides both two-channel orderings.
- **Generic RGBW, RGBWA, and RGBWAUV LED** — one canonical RGB-first emitter order with an 8-bit dimmer first, an 8-bit dimmer last, or a virtual dimmer. **Generic RGBCCT LED** provides the six useful placements of the RGB block, cold white, and warm white (`RGBCW`, `RGBWC`, `CRGBW`, `CWRGB`, `WRGBC`, and `WCRGB`), each with those same three dimmer choices. The library deliberately avoids factorial permutations of individual RGB emitters that do not represent normal fixture personalities.
- **Generic Dimmer RGB Control PAR (LED PAR 56 Suedbahnhof)** — the operator-supplied five-channel personality in Fixed 0, Red, Green, Blue, Fixed 0 order. The first and fifth slots are static outputs that always transmit zero; the RGB emitters use virtual intensity because the fixture has no physical dimmer channel.
- **Generic rare-capability references** — **Endless Pan Tilt** retains endless 16-bit axis representation on the canonical Pan and Tilt controls; **Beam Size and Edge** keeps Zoom independent from Softness; **Media Positioning** provides independent media-layer X and Y axes; **Flame Jet** supplies a ToskLight-authored single-nozzle demonstration; and **Kabuki Curtain** maps one raw slot to Reset, Hold, and a latched Release through its portable physics script. These are explicit transferable reference personalities: match their documented channel order to the device rather than treating them as manufacturer profiles.
### Generated Venue objects

A curtain's height, a truss's length and a deck's rise are measurements of the venue rather than
personalities of a fixture. Shipping one profile per size — and a mode per size inside it — described
the same object over and over and still only covered the sizes somebody had thought of.

The trusses, curtains, decks and stairs now declare what shape they are and are built at the size
they are placed, so a truss repeats its chords over whatever length it is given rather than being
stretched to it. Each has one mode, and its name says the size it arrives at: **Curtain 2 m** is two
metres wide until you say otherwise. What can be changed is what the object really is made to
measure — a curtain's width and drop, a truss's length, a deck's rise — and a size outside what the
object can be built at is held to what it can.

Set the size in **Show Patch**. The **Footprint width**, **Footprint height**, and **Footprint
depth** columns offer exactly the dimensions the object is made to measure — a curtain's width and
drop, a truss's length, a deck's width, depth, and rise — and show a dash for the rest, because a
truss's cross-section is what the truss is rather than a number to type. Each placed object keeps
its own size, so every segment of a truss run can differ. A size outside what the object can be
built at is refused with the range it can take. The same columns set a Crowd
Area's width and depth.

The **Patch** sheet of ToskLight PreViz, with **Show all** on, has the same **Footprint width**,
**Footprint height**, and **Footprint depth** columns for generated Venue objects, and editing an
object there — its name, position, layer, or size — keeps the size it was given on the desk. A
selection takes one size or a `THRU` spread, exactly like **Location**.

Three more columns hold what each placed object is made of, in **Show Patch** and on the PreViz
**Patch** sheet alike:

- **Colour** — any generated Venue object: a curtain in red serge, a truss in black. Choose a colour
  in the picker, or **Default colour** to return the object to its own material — black serge for
  a curtain, raw aluminium for truss, grey for staging. Every object placed before this keeps its
  default.
- **Chain** — how a chain is rigged: **Plain chain**, just the chain; **Motor on top**, a chain
  hoist hanging at its top; or **Motor on bottom**, the hoist at its bottom. The end without the
  hoist is fixed by what it hangs from: on a three- or four-point truss, a 22 mm steelflex wrapped
  round the nearest chord, its two legs meeting at 45°; on a pipe or a two-point truss, a flange
  clamped round the tube; with nothing within half a metre, just the shackle. Every fixing ends in
  a bow shackle — a U with its walls drawn out and a bolt straight through their ends — and the
  chain's last link hangs on that bolt.

A chain nobody chose a mode for has its motor on top. A chain whose ends were chosen before the
modes existed keeps its hoist where it was and gets its fixing at the other end. Anything the
column does not apply to — a lamp, a truss, a multi-patch copy — shows a dash.

**Scale** draws a Venue object at a multiple of the size it was built at — an imported hall that
was modelled in centimetres, a set piece brought in at half size. Type any scale from `0.01` to
`100`; `1`, or an empty entry, returns the object to its built size, which is what every object
placed before this reads as. A scale outside that range is refused with the range. The Visualizer,
the ToskLight PreViz plan and the desk's Stage all draw the object at its scale, and the PreViz
**Patch** sheet, with **Show all** on, shows and edits the same column. A lamp, a Crowd Area and a multi-patch copy
show a dash.

A chain is drawn as real hoist chain, in the Visualizer and in the PreViz plan and elevations: links
of 7 mm wire, 35 mm long, each turned a quarter to the next and overlapping it by the wire's
thickness. From above, a chain is two crossed rounded rectangles, and a motor on top hides it. In
the Visualizer the steelflex is purple and snaps round the nearest chord of the truss at that end of
the chain, and the flange clamps the nearest pipe.

The trusses are drawn the way square truss is built, in the Visualizer and in the PreViz plan and
elevations alike. The diagonals run at close to 45°, with a node about every chord spacing, so a
large truss is braced in longer bays than a standard one of the same length. Opposite faces run the
other way, so a side view shows an X in every bay, and a deco truss crosses its diagonals in every
bay of every face. Each piece has an end frame just inside each end, a coupler receiver on every
chord end, and the conical coupler centred on the joint, so a run of pieces reads as separate
sticks coupled end to end. The PreViz plan and elevations leave out every line hidden behind
another part — a brace behind a chord, a chain link's end wire behind the link passing through it,
a scissor arm behind the one crossing it — the way a technical drawing does, on screen and in print. In the PreViz plan a curtain is a wavy line along its track, four
waves per fold, and in a front or back elevation it is the rectangle it covers with a dotted line down
each fold, its dashes leaning 10–20° off vertical, alternating sides.

A stage element is the platform it is built on: its width and depth are fixed, and only its rise is
set, from 0.1 to 1.2 m. A stage element already placed at another base size keeps it. It stands on
what its profile is: a **Stage Element** on a scissor lift — a deck on crossed arms over a base
frame, with more stages of arms as it rises — and a **Stage Deck** on regular feet, one leg under
each corner carrying a 40 mm top, both in the Visualizer and in the PreViz elevations; stage stairs
keep their own shape. All three are placed by their feet: the position is the middle
of the footprint on the floor they stand on, so Z 0 stands on the stage floor, and changing the rise
raises or lowers the deck while the feet stay put. The people in a
Crowd Area differ in size, the same way every time the show is drawn.

A show patched before this keeps the fixture revision embedded in it and is unaffected.

- **Venue** visual-only profiles — 1 × 1 m, 2 × 1 m, and 1 × 0.5 m stage elements with an adjustable rise; correctly rising stage stairs; **Curtain**, made to any width, drop and colour, beside the 1 m, 2 m, 3 m, 5 m, and 6 m curtains; **Disco Ball 50 cm**; and **Crowd Area**. The pipe, **Two-Point Truss**, **Three-Point Truss**, **Three-Point Deco Truss**, **Four-Point Truss** and **Large Four-Point Truss** profiles, and **Chain**, use the separate **Rigging** type. Crowd Area supplies all nine Sitting, Standing still, and Dancing × Sparse, Medium, and Dense modes and stores independent width and depth with the show. The conventional scenery archives include portable photographs and metre-authored GLB geometry; the desk displays its built-in Venue or Rigging type icon. Crowd Area is rendered procedurally from its portable crowd contract.
- **Venue** backline, PA, flight cases, and figures — visual-only objects for what stands on stage beside the lights: **PA Top**, **PA Top on a Pole Stand**, **Subwoofer**, **Line Array Hang**, **Stage Monitor Wedge**, **DJ Mixer**, **DJ Media Player**, **Drum Kit**, **Guitar Stack**, **Electric Guitar on a Stand**, **Saxophone on a Stand**, **Stage Piano**, **Microphone Stand**, flight-case racks at 2U, 4U, 6U, 8U, 14U, and 18U, and the **Deejay**, **Guitarist**, **Pianist**, and **Singer** figures. Each carries the shipped Visualizer model at the size that model was built at, has no DMX footprint, and is placed from **Add venue element** on the PreViz CAD screen. Each stands for a class of equipment rather than one manufacturer's product.
- **ToskLight** product and Visualizer profiles — **Audio Player** is an Internal fixture: one independently programmable Audio service voice with a regular fixture ID and no DMX address. It is addressed through the canonical Media attributes — Media Folder, Media File, Play mode, and Volume — so the Media encoder group and the Media pane control it exactly like any other media source. Play mode carries transport and repeat together: a looping mode repeats the file, a once mode plays it through, and Stop and Pause hold the voice silent. Stop is the patched default. Play mode names every mode it can be in — Loop, Reverse, Bounce, the Once and Reverse once end states, their tempo-synced counterparts, Stop, and Pause — so the encoder and the Media pane show the mode by name instead of a percentage, and each mode can be chosen directly or generated as a preset. Media Server play mode is named the same way. Shows patched before this change keep their stored Audio Folder/File, Transport, Repeat, and Volume attributes and continue to play. **Media Server** provides two complete personalities: 158 slots for two layers and 512 slots for eight layers. Each 59-slot layer is an independently programmable logical head; the trailing 40-slot output block belongs to the shared master head and exposes Output, Geometry, Mask position, Shapers, Colour, and the fixed Layer Opacity Cycle effect. Shows patched before the release keep their embedded 75-slot legacy, 89/323-slot mask-position, or 119/353-slot effect-bank snapshots and still load, but the Media Server decodes only the two current personalities, so repatch those fixtures. Both personalities use the complete master with two selectable effect banks per layer, each carrying Effect Select, Effect Strength, and four effect parameters, plus blend mode and strobe, in and out points, four visualizer parameters, and 3D model mapping with pan and tilt. The output block mirrors through a negative scale instead of a Flip/mirror channel. **Visualizer Camera** keeps the stable 17-slot X/Y/Z, Yaw/Pitch/Roll, and Zoom wire contract, while **Visualizer Laser** provides the packaged demo laser and its scan program. This manufacturer is reserved for implemented ToskLight-owned product fixtures; planned further Visualizer fixtures do not appear until their capabilities exist.

- **JB-Lighting JBLED A7** — Standard and Compressed RGB personalities in 8-bit and 16-bit color,
  with the complete shutter-effect table and a documented open shutter as the safe home and
  Highlight look.
- **Martin MAC 250 Entour** — 16 Bit and 16 Bit Extended; **MAC 300** — Mode 4;
  **ELP CL Profile** — 10-Channel; and **ELP WW Profile** — 4-Channel.
- **Cameo AURO SPOT Z300** — 17-Channel and 20-Channel, with named color and gobo wheel
  positions, gobo shake and wheel rotation bands, prism, frost, auto program, auto movement,
  and device-setting functions from Cameo's DMX chart; **ROOT PAR 6** — D7CH with its physical
  delay channel and virtual intensity; and **Q-SPOT 40 TW** — all five 1-, 2-, 3-, and
  8-channel personalities.
- **Prolights ECL Fresnel CT+M** — STANDARD; **Clay Paky Stage Zoom 1200** — the
  venue's 20-slot 16-bit, gobo-fine, lamp-control personality in addition to its existing modes.
- **High End Systems Trackspot** — the classic seven-channel mirror scanner in low- and high-resolution DMX personalities.
- **Showtec Sunstrip Active DMX** — ten independently controlled tungsten lamps.
- **Showtec Sunstrip LED RGB 42206** — ten independently controlled RGB pixels, each with its
  own additive color system, so every pixel shows its own color in the Visualizer.
- **ROBE Robin DLS Profile**, **Robin 600X LEDWash**, **Robin LEDBeam 150**, **Robin 300 LEDWash**, and **Robin DLF Wash** — every documented manufacturer personality. The 600X and 300 zone modes expose their three concentric RGBW zones as logical heads.
- **Claypaky Sharpy**, **ETC Source Four LED Series 2 Lustr**, **CHAUVET Professional COLORado 1 Solo**, and **GLP JDC1**. JDC1 SPix modes expose all twelve RGB plate pixels and twelve white beam segments as logical heads.

The Source Four LED Series 2 configuration can independently enable Strobe, Fan Control, and Plus Seven. Its package contains the canonical console personalities, including the common fully enabled and Plus Seven variants, instead of multiplying every fixture-menu option permutation into a separate mode.

Channel order, footprints, fine-byte slots, safe defaults, and physical ranges come from the corresponding manufacturer DMX charts. So do the positions on colour and gobo wheels, prisms, macros, effect banks, and the power, special-function and reset channels: the shipped manufacturer packages name each band the chart prints, so an encoder reads "Gobo 3" or "Total reset" rather than a percentage. Where a manufacturer chart is genuinely proportional - a frost, a fan, a macro speed - the channel stays a single range, and where one DMX range carries two mutually exclusive meanings chosen from the fixture menu, such as ROBE's combined Pan/Tilt speed and time, the package keeps the range unnamed rather than asserting one of them. Shipped packages are not privileged or reserved: after loading, they are normal desk-library profiles. When a newer shipped package is installed, ToskLight updates it only if its last package-installed revision is still current. An operator-created later revision is preserved and reported instead of being overwritten.

Package format, asset restrictions, JSON structure, generated projections, and archive validation
are fixture-author documentation. For an operator, the important rule is simple: export a working
fixture as the safest template, keep its identity when correcting that fixture, and import the
result as a new immutable revision. If an import contains an unknown attribute, map it to a
compatible desk attribute or create a custom one before importing; existing revisions remain
readable and exportable.

## Import GDTF

Choose **Import GDTF** and select a `.gdtf` archive. ToskLight normalizes the supported modes, channels, physical information, emitters, capabilities, geometry, and model into a fixture profile and retains the original GDTF bytes beside every resulting immutable revision. MVR export can therefore use the retained source instead of reconstructing an archive from lossy normalized data.

GDTF interchange is preserved in both directions through the show workflow: import the original `.gdtf` into the Fixture Library, then export an MVR to carry the retained GDTF archives for the fixtures in that rig. A profile without a retained GDTF is exported in the MVR as a GDTF generated from the profile's modes and channels; it describes the patch and channel layout but not wheels, emitters or the 3D model. The Fixture Library does not export a standalone GDTF file.

The same canonical-attribute preflight applies before a newly normalized GDTF profile is stored. An
import or migration error leaves the original data untouched and appears in the open import dialog
or as an actionable warning in the Fixture Library. Do not delete the source row until the warning
has been investigated or the fixture has been recovered.

![Import every mode from a local GDTF archive](../../assets/screenshots/workflows/fixture-library-import.png)

## Create or edit a fixture profile

**Create fixture** opens a blank profile with one mode named **Default** and one editable main head. **Edit as new revision** opens the same editor with the chosen revision. The title bar contains **Identity**, **Simulation**, **Modes**, **Save fixture**, and Close; the Modes tab also adds **Add mode** at the top right. There is no footer Cancel action.

Closing an unchanged editor is immediate. Closing a changed editor through Close, Escape, or the backdrop asks whether to **Stay** or **Discard changes**. Saving an existing profile first asks to **Save and create revision**. A failed or stale save keeps the editor open and explains the problem.

### Identity

Identity covers who the fixture is: manufacturer, full and short names, fixture type, notes, stage icon, photograph, and optional visualizer GLB model. Notes, photograph, and visualizer are shown side by side; drag the visualizer preview to inspect the GLB from another angle and scroll to zoom. Manufacturer remains free text. Use its lookup button to search the unique desk-library manufacturers with the shared full-text keyboard and fill the field without saving the editor.

### Simulation

Simulation is what the Stage needs in order to draw the fixture. **Body** chooses the shape it is
drawn as, **Physical** is how
the lantern is built: dimensions, weight, power consumption, connectors, light source, colour
rendering index, and lens. **Optics** is what comes out of it: colour temperature, luminous output,
beam angle, relative output, edge, field uniformity, and light-source dimensions.

Colour temperature, luminous output, and beam angle describe the light rather than the lantern, so
they are part of Optics. A profile written before that keeps working untouched: the values are read
from where they used to be stored and belong to Optics from then on, including in the profile
revision a patched show has already embedded.

Leave any optics field empty to use the normal appearance for the fixture type.

Every physical figure is entered in a fixed unit and precision:

| Field | Unit | Precision |
| --- | --- | --- |
| Width, Height, Depth | millimetres (mm) | whole numbers |
| Weight | kilograms (kg) | up to two decimal places |
| Power consumption | watts (W) | whole numbers |
| Sharpness, Uniformity | percent (%), 0 to 100 | one decimal place, always shown as e.g. `85.0` |

A figure beyond its precision — `420.5` mm, `24.555` kg — is kept on screen and named under the
field, and **Save fixture** lists it instead of saving. Nothing is rounded for you. Every shipped
fixture is stored at this precision. A profile imported from before these rules, with a dimension
such as `498.2` mm, still loads and patches unchanged; its off-precision figures are named when you
open it in the editor and must be corrected before that revision can be saved.

**Generic body** names one of the bodies ToskLight ships — the full list is in the
[Model Catalogue](../../99-Appendix/01-model-catalogue.md) — so a PAR 64 long nose is drawn as one
rather than as whatever its declared type suggests. Press the field to open the body picker, which
shows a picture of every body grouped by kind and can be searched by name. Leave it on *Guess from
the fixture type* and
the fixture keeps the behaviour it has always had: the body is inferred from the declared type and
the channels the mode has, which is right for most fixtures and cannot tell a PAR 64 from a PAR 16
or a two-cell blinder from an eight. A fixture that ships its own visualizer model is drawn with
that model, whatever the body says.

**Mounting** is what holds the lantern up, and it is what ToskLight Architect rigs it by: drag a
lamp near a truss and the pipe line of its clip lands on the chord the clip reaches. **Hangs by**
is a hook clamp over a pipe, a yoke or bracket bolted down, or nothing at all for a fixture that
stands — a hazer, a floor can — which is then never picked up by a truss. The rest is six figures
in whole millimetres from the centre of the body: **Pipe across**, **deep** and **up** put the bar
where it ends up, and **Clip width**, **depth** and **height** say how big the hardware around it
is, which is how near a pipe has to come to catch it. The clip is taken to hang directly under the
pipe, as a hook clamp does.

Every shipped lantern already carries the clip its real hardware has. Fill this in for a model you
imported yourself, where only you know where the clamp is. The figures are read against the
fixture's declared Physical size, so a lamp placed larger or smaller in the plan keeps its clamp in
proportion; a fixture with no declared size takes the clip as it stands.

### Modes and heads

Modes have stable identities, names, notes, and complete channel configuration. Each row in the full-width Modes list edits that mode's name and notes directly and summarizes its heads, logical channels, and splits. Add modes from the title bar; reorder them with drag-and-drop. Removing a mode asks for confirmation first, because its heads, channels, and functions go with it; the final mode cannot be removed. **Edit channels** opens the nested tabs in this order: **Heads**, **Channels**, **Control actions**, **Color**, and **Emitters & Motion**.

The path under each editor window's title — for example *Acme Orbit › Modes › Default › Channels*
— shows where you are among the nested windows. Press an earlier step to close every window below
it.

Every head has a stable identity and an optional master/shared designation. Heads describe logical emitters, not patch blocks: one head may own channels in several independently patched splits. At most one head is master/shared. A head that still owns channels cannot be removed until those channels are reassigned or removed.

A split is an independently patchable address block configured in Channels. Its footprint follows the slots its channels take up, and every physical channel belongs to one split; each split gets its own optional universe and address in Show Patch. An unpatched split remains selectable, programmable, and visible but emits no DMX.

![Nested fixture mode editor with Heads, Channels, Color, and Geometry tabs](../../assets/screenshots/workflows/fixture-library-mode-editor.png)

### Channels

Channels are arranged by split, one row per DMX slot, in the order the manufacturer's chart lists
them. Every cell shows its value; press it to open the list or number keypad that changes it. Each
row has these columns:

- **Attribute** opens a picker with three columns: the encoder group (Intensity, Color, Position,
  Beam…), the activation group inside it, and the attribute itself — plus **Static** for a slot that
  is never controlled.
- **Level** is **Coarse**, **Fine**, **Ultra**, or **Extreme**. A channel is Coarse on its first
  slot. To make a 16-bit Pan, give two slots the Pan attribute and set the second to **Fine**: it
  becomes Pan's second byte, and the table shows it as *↳ Pan*. Setting it back to **Coarse** makes
  it a channel of its own again. A Fine slot needs a coarse slot with the same attribute on the same
  head to refine; existing default, Highlight, and function values keep their meaning when a byte is
  added or removed.
- **Default** and **Highlight** open a number keypad for the raw value.
- **Mapping** opens the channel's physical range — minimum and maximum, in the unit of the chosen
  attribute — with the channel's functions as a table underneath. A function's behavior-specific
  values, such as a fixed value's label or angular motion, open under its row with **Details**.
- **Invert** and **Snap** switch the channel's inversion and whether it jumps instead of fading.
- **Masters** opens **React to Virtual Intensity** — **Ignore**, **Follow**, or **Inverse** — and
  the switches for **React to Sequence Master**, **React to Group Master**, and **React to Grand
  Master**. **Inverse** makes the channel full while the virtual intensity is at zero and gone at
  full, for a slot that must do the opposite of the dimmer. The table shows it as *−VI*.

The table is the tab's only scrolling area: it scrolls up, down, and sideways under a fixed column
header, while the mode editor's title bar and split headers stay in place. In a channel's
**Mapping** window the function table likewise scrolls on its own below the physical range.

Only the Coarse row carries these settings; a further byte shows a dash. Drag a row by the handle
beside its slot number, or use its move buttons. The red bin removes a slot after asking, and the
slots after it move up.

**Add split** and **Add channel** sit in the mode editor's title bar while **Channels** is open; a
new channel goes into the split that is open. With more than one split, each has a one-line header
with its name and how many channels it holds — press it to open that split — and its own move and
remove buttons. A split's footprint follows its slots, so it is never typed. Typed control actions
have their own **Control actions** tab, with **Add control action** in the title bar. The editor blocks invalid
footprints and overlapping slots. Choose a safe home and Highlight look, then verify them on the
real fixture. Detailed raw-value, color-system, and control-action authoring is fixture-developer
documentation.

A **Static output** channel has no Programmer control. It transmits its authored Default raw value
during normal output and its separately authored Highlight raw value while Highlight is active, so
a normally fixed control slot can still switch a fixture display or identification mode for
Highlight. A per-fixture Highlight override may replace that Highlight raw value.

### Color

Configure the fixture's additive, subtractive, hue/saturation, or wheel color system. Every head
has its own section and its own color system, so each cell or pixel of a multi-head fixture is
configured independently. **Copy to other heads** gives every other head that has the same kind of
channels its own copy, bound to that head's channels; each copy can then be edited on its own.
Heads without matching channels, such as a shared master head, keep what they have.

Each color system also declares its **Color Intent calibration**: **Measured** for data taken with a
colorimeter or spectrometer, **Nominal** for datasheet or typical values, **Uncalibrated** when the
data cannot promise a colour. Raise the **Calibration revision** whenever the colour data changes and
name its **Calibration source**. A **Subtractive CMY** system can hold the **Measured filter
output**: the open beam and the beam with each flag fully in. [Color Intent](../20-Programmer-and-Cues/05-color-intent.md)
uses this data to decide how closely the fixture can show a colour and says so.

For a **Discrete color wheel**, **Fill slots from wheel functions** creates one slot for each
named position of the wheel channel. Colors you already defined are kept. Each slot has a
**display color** that the Visualizer shows while the wheel is in that slot. Choose one in the
picker to store it with the profile; it is marked **Defined color**. A slot without a defined color
shows the color its name describes, marked **From the slot name**: Open or White shows white, Deep
Red a dark red, CTO a warm tint. A slot whose name describes no color leaves the fixture's own
color in the Visualizer. **Steady colour for Color Intent** says whether Color Intent may park the
wheel in that slot: left on **Judge by the slot name**, a split, scroll, rotation or effect position
is never used. Each wheel slot gives every field room for its value; in a narrow window
**DMX from** and **DMX to** move onto their own line rather than shrinking. Then use **Generate
portable presets** in **Control → Special Dialog** when fixed or indexed choices should be added to
the show.

### Emitters & Motion

A mode says which of its heads owns which of the fixture's emitters, and which attribute moves each
of its moving parts; that is the whole of what a personality says about geometry. An emitter no
head owns is not lit in that mode — which is how one personality gives every ring of a wash a head
of its own and another drives them all together, without either describing the lantern twice.

Under **Moving parts**, press a part to choose which of this mode's channels drives it. Only
attributes the mode's channels carry are offered, each once with the slots that carry it, plus
**Not driven** to leave the part resting where it is drawn. A template's pan arm and tilt head start out
driven by Pan and Tilt in every mode.

## Geometry

Geometry belongs to the fixture, not to one of its personalities: a moving head has the same yoke
whichever mode it is patched in. It is edited once, in the profile's own **Geometry** tab, and every
mode binds its heads to the emitters described there.

A profile written before this carried a whole graph on each mode. Reading one moves the graph to
the fixture and leaves the head each emitter named behind as that mode's binding, so nothing an
operator authored is lost and no show needs repatching. Modes are compared by what they describe
rather than by the identifiers they were written with, and a mode that adds parts the others leave
out — a wash whose zone personalities drive four heads where its plain ones drive one — is read as
that personality driving more of the same lantern.

Where a profile's modes genuinely describe different geometry the profile is left exactly as it
was. No shipped fixture is in that position any more: the Venue objects that were are generated
instead, described below.

Choose a suitable fixture geometry and use the preview to confirm the lamp's shape. Detailed
model hierarchy, emitter, pivot, and projection authoring is fixture-developer documentation.

The Geometry tab fills the editor window: the part and emitter list, the selected part's
properties, and the live 3D preview sit side by side, and each scrolls on its own. The preview
shows the lamp itself — its parts, their transforms and the emitter faces — framed on a dark-blue
background; it draws no beam, so check how the light falls on Stage. A part's
properties are split into tabs — **Generic** (name, parent part, GLB node binding, and **Remove
part**), **Translation**, **Rotation**, **Scale**, **Pivot**, and **Animate**.

**Animate** makes a part move and describes how: **Motion kind**, **Motion axis**, the physical
minimum and maximum, and its speeds. It does not name the attribute that drives the part; each mode
chooses that under **Emitters & Motion**.

An axis that moves — a yoke's pan, a head's tilt — can also declare how fast it actually travels:
**Top speed**, **Acceleration**, and **Deceleration**, in degrees per second for a rotation and in
millimetres for a translation. A real lantern does not arrive instantly, and two
fixtures given the same position at the same moment do not arrive together. Leave the figures
empty and the axis moves as fast as it is told to, which is how every fixture behaved before.

## Revisions and compatibility

The server assigns revision numbers atomically and rejects concurrent edits. Open **Revision history** to inspect immutable revisions, edit an older revision as a new one, or delete an unused revision. Deletion warns when a patched show embeds that revision; the show's snapshot remains intact even if deletion is confirmed.

Legacy library entries migrate through an explicit schema-v1 reader. Compatible modes are combined only when their fixture-family metadata agrees; conflicts remain separate and produce a visible warning. Installations that predate transferable packages remove only the historical code-owned catalog rows, then load the equivalent `.toskfixture` files as ordinary profiles. User-authored profiles are never claimed by manufacturer or model name.

During legacy or GDTF migration, intensity, RGB/RGBW/additive, CMY/subtractive, and identifiable Open/White wheel channels receive the same deterministic physical Highlight defaults; unmatched wheel, Position, and unrelated channels retain their source defaults. Existing authored schema-v2 Highlight raw values are preserved exactly. A patched fixture without a per-instance Highlight override map inherits those values from its embedded profile revision. Later desk-library edits therefore do not silently change the Highlight Look already stored with a show.

![Create a complete revisioned fixture profile](../../assets/screenshots/workflows/fixture-library-create.png)
