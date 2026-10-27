# Generated Venue geometry reference

Set a curtain's height, a truss's length, and a deck's rise on the placed object. Each object keeps its own dimensions.

Trusses, curtains, decks, and stairs are built at the size you place them, so a truss repeats its chords over the chosen length. Each has one mode, and its name says the size it arrives at: **Curtain 2 m** is two
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

The **Patch** sheet of ToskLight Architect, with **Show all** on, has the same **Footprint width**,
**Footprint height**, and **Footprint depth** columns for generated Venue objects, and editing an
object there — its name, position, layer, or size — keeps the size it was given on the desk. A
selection takes one size or a `THRU` spread, exactly like **Location**.

Three more columns hold what each placed object is made of, in **Show Patch** and on the Architect's
**Patch** sheet alike:

- **Colour** — any generated Venue object: a curtain in red serge, a truss in black. Choose a colour
  in the picker, or **Default colour** to return the object to its own material — black serge for
  a curtain, mill-finish aluminium for truss, dark brown film-faced multiplex for staging. Every
  object placed before this keeps its default.
- **Chain** — how a chain is rigged: **Plain chain**, just the chain; **Motor on top**, a chain
  hoist hanging at its top; or **Motor on bottom**, the hoist at its bottom. The end without the
  hoist is fixed by a 22 mm steelflex round the chords of the truss it hangs from, on the side
  the chain comes from — the top chords for a chain from the roof. Where one chord is uppermost
  (a pipe, a two-point truss on edge, a three-point truss with its apex up) the sling is choked
  round that chord alone and goes straight up to the shackle; where two chords are level (a
  four-point truss, a three-point truss with its flat side up) it is basketed under both, up the
  outside of each, and its legs meet at the shackle at 45°. With nothing within half a metre,
  just the shackle. Every fixing ends in a bow shackle — a U with its walls drawn out and a bolt
  straight through their ends — and the chain's last link hangs on that bolt.

A chain nobody chose a mode for has its motor on top. A chain whose ends were chosen before the
modes existed keeps its hoist where it was and gets its fixing at the other end. Anything the
column does not apply to — a lamp, a truss, a multi-patch copy — shows a dash.

**Scale** draws a Venue object at a multiple of the size it was built at — an imported hall that
was modelled in centimetres, a set piece brought in at half size. Type any scale from `0.01` to
`100`; `1`, or an empty entry, returns the object to its built size, which is what every object
placed before this reads as. A scale outside that range is refused with the range. The Visualizer,
the ToskLight Architect plan and the desk's Stage all draw the object at its scale, and the Architect's
**Patch** sheet, with **Show all** on, shows and edits the same column. A lamp, a Crowd Area and a multi-patch copy
show a dash.

A chain is drawn as real hoist chain, in the Visualizer and in the Architect plan and elevations: links
of 7 mm wire, 35 mm long, each turned a quarter to the next and overlapping it by the wire's
thickness. From above, a chain is two crossed rounded rectangles, and a motor on top hides it. In
the Visualizer the steelflex is purple and snaps to the truss at that end of the chain, round its
top chord or under its two top chords as the section calls for; the Architect elevations draw it
schematically round one chord.

The trusses are drawn the way square truss is built, in the Visualizer and in the Architect plan and
elevations alike. The diagonals run at close to 45°, with a node about every chord spacing, so a
large truss is braced in longer bays than a standard one of the same length. Opposite faces run the
other way, so a side view shows an X in every bay, and a deco truss crosses its diagonals in every
bay of every face. Each piece has an end frame just inside each end, a coupler receiver on every
chord end, and the conical coupler centred on the joint, so a run of pieces reads as separate
sticks coupled end to end. The Architect plan and elevations leave out every line hidden behind
another part — a brace behind a chord, a chain link's end wire behind the link passing through it,
a scissor arm behind the one crossing it — the way a technical drawing does, on screen and in print. In the Architect plan a curtain is a wavy line along its track, four
waves per fold, and in a front or back elevation it is the rectangle it covers with a dotted line down
each fold, its dashes leaning 10–20° off vertical, alternating sides.

A stage element is the platform it is built on: its width and depth are fixed, and only its rise is
set, from 0.1 to 1.2 m. A stage element already placed at another base size keeps it. It stands on
what its profile is: a **Stage Element** on a scissor lift — a deck on crossed arms over a base
frame, with more stages of arms as it rises — and a **Stage Deck** on regular feet, one leg under
each corner carrying a 40 mm top, both in the Visualizer and in the Architect elevations; stage stairs
keep their own shape. All three are placed by their feet: the position is the middle
of the footprint on the floor they stand on, so Z 0 stands on the stage floor, and changing the rise
raises or lowers the deck while the feet stay put. The people in a
Crowd Area differ in size, the same way every time the show is drawn.

Existing shows retain their embedded fixture revision.

- **Venue** visual-only profiles — 1 × 1 m, 2 × 1 m, and 1 × 0.5 m stage elements with an adjustable rise; correctly rising stage stairs; **Curtain**, made to any width, drop and colour, beside the 1 m, 2 m, 3 m, 5 m, and 6 m curtains; **Disco Ball 50 cm**; and **Crowd Area**. The pipe, **Two-Point Truss**, **Three-Point Truss**, **Three-Point Deco Truss**, **Four-Point Truss** and **Large Four-Point Truss** profiles, and **Chain**, use the separate **Rigging** type. Crowd Area supplies all nine Sitting, Standing still, and Dancing × Sparse, Medium, and Dense modes and stores independent width and depth with the show. The conventional scenery archives include portable photographs and metre-authored GLB geometry; the desk displays its built-in Venue or Rigging type icon. Crowd Area is rendered procedurally from its portable crowd contract.
- **Venue** backline, PA, flight cases, and figures — visual-only objects for what stands on stage beside the lights: **PA Top**, **PA Top on a Pole Stand**, **Subwoofer**, **Line Array Hang**, **Stage Monitor Wedge**, **DJ Mixer**, **DJ Media Player**, **Drum Kit**, **Guitar Stack**, **Electric Guitar on a Stand**, **Saxophone on a Stand**, **Stage Piano**, **Microphone Stand**, flight-case racks at 2U, 4U, 6U, 8U, 14U, and 18U, and the **Deejay**, **Guitarist**, **Pianist**, and **Singer** figures. Each carries the shipped Visualizer model at the size that model was built at, has no DMX footprint, and is placed from **Add venue element** on Architect's CAD screen. Each stands for a class of equipment rather than one manufacturer's product. The **Flight Rack**, **PA Speaker** and **Line Array** are generated at the size they are placed instead: a rack by its rack units and depth, a PA speaker with or without a pole stand of any height, a line array by its elements. They replace the modelled racks, PA tops and line array in the PreViz add menus; shows that placed the modelled ones keep them.
