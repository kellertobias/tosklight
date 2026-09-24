# Parametric Venue Objects

## Purpose

Prove that trusses, curtains, chains and stage elements are placed from the fixture library and
configured per placed object — size, colour and chain ends — on the desk and in ToskLight PreViz
alike, that the Visualizer draws exactly what was configured, and that shows written before these
choices existed open unchanged.

## Trusses

1. In **Show Patch** on the desk, add **Two-Point Truss**, **Three-Point Truss**, **Three-Point Deco
   Truss**, **Four-Point Truss**, **Large Four-Point Truss** and the pipe from the **Venue**
   manufacturer. Confirm each is placed with a `0.x` ID and no DMX address.
2. Set **Footprint width** on each to a different length between 0.25 m and 24 m (30 m for the large
   truss). Confirm **Footprint height** and **Footprint depth** show a dash.
3. Enter 40 m on a standard truss. Confirm the entry is refused with the range the truss can take.
4. Open the Visualizer. Confirm every truss is drawn at its length with its own number of chords,
   that the large truss is braced in visibly longer bays than the four-point truss, and that the
   deco truss crosses its diagonals in every bay.
5. Confirm each truss's diagonals run at close to 45°, that each piece has an end frame just inside
   both ends, and that every chord end carries a receiver with a conical coupler centred on the
   joint.
6. Open the Architect plan and a front elevation. Confirm each truss is drawn at its placed length
   with the same bracing: an X in every bay of a four-point truss, an end frame at each end, and a
   coupler at every chord end. Open a side elevation looking down a truss and confirm its chords,
   its end frame and, on a four-point truss, the end frame's diagonal.
7. Add **Four-Point Truss Corner 2-Way** and butt it against the end of the four-point truss.
   Confirm both are drawn at the same 290 mm section in the Architect plan and in the Visualizer:
   the corner's chords continue the straight run's rather than stepping in or out at the joint.
   Repeat with the two- and three-point trusses and their corner blocks.

## Curtains

1. Add **Curtain**. Set **Footprint width** to 8 m and **Footprint height** to 5 m. Confirm the
   Visualizer draws a gathered drape of that size.
2. Open **Colour**, choose a red, and set it. Confirm the cell shows the colour and the Visualizer
   draws the curtain red.
3. Open **Colour** again and press **Default colour**. Confirm the cell reads **Default** and the
   curtain is black serge again.

## Chains

1. Add **Chain** and set **Footprint height** to 3 m. Confirm **Chain** reads **Motor on top**, and
   that the Visualizer draws a hoist body above the chain and a purple steelflex below it: a wrap,
   two legs meeting at 45° and a shackle on the end link.
2. Hang the chain so its bottom end is on a four-point truss's top chord. Confirm a 22 mm steelflex
   wraps round that chord and ends in a bow shackle whose bolt the last link hangs on. Move the end
   onto a pipe and confirm a flange clamps the pipe instead, with no steelflex; move it clear of any
   truss and confirm only the shackle remains. Confirm the PreViz front and side elevations draw
   the same fixing.
3. Set **Chain** to **Motor on bottom**. Confirm the hoist moves to the bottom and the steelflex to
   the top. Set **Plain chain** and confirm only the chain is drawn.
4. In the PreViz plan, confirm the chain is two crossed rounded rectangles from above, and that a
   motor on top hides it. In a front and a side elevation, confirm links alternate between a link
   seen face-on — two nested rounded rectangles — and one seen edge-on, overlapping by the wire's
   thickness.
5. Confirm **Chain** shows a dash on a truss, a curtain and a lamp.

## Stage elements

1. Add **Stage Element 2 × 1 m**. Confirm **Footprint width** and **Footprint depth** show a dash and
   **Footprint height** can be set between 0.1 m and 1.2 m.
2. Set the rise to 0.2 m and then 1.2 m. Confirm the Visualizer and the PreViz front and side
   elevations draw a deck on a scissor lift over a base frame, with more stages of arms at 1.2 m.
3. Add **Stage Deck 2 × 1 m**. Confirm the same footprint fields, and that the Visualizer and the
   PreViz front and side elevations draw a 40 mm top on one leg under each corner rather than a
   scissor lift, with the top's surface at the height that was set and the legs standing on the
   floor.
4. Set the deck to 0.24 m and then 1.04 m — the lowest and highest the retired decks on fixed legs
   stood at — and confirm both are accepted.
5. Add **Stage Stairs**. Confirm it stands on neither a scissor lift nor legs, that its width,
   height and depth can all be set, and that the Visualizer and the PreViz front and side
   elevations draw a flight of steps climbing to the height that was set: a 200 mm rise per step,
   so a 0.6 m flight has three and a 1.0 m flight five. Set it to a deck's height beside that deck
   and confirm the top step meets the deck's surface.
6. Add **Stage Stairs with Handrails**. Confirm the same footprint fields, and that the Visualizer
   draws a rail up each side with a post on every nosing, following the climb. Confirm the plain
   flight has none.
7. Add **Stage Handrail**. Confirm **Footprint width** can be set between 0.4 m and 24 m and that
   **Footprint height** and **Footprint depth** show a dash — a stage edge guard is 1 m high and no
   deeper than its posts. Confirm the Visualizer and the PreViz elevations draw posts about 1.2 m
   apart under a top rail and a knee rail, and that the plan draws the thin line of its run.
8. With snapping on, drag the handrail towards the outside edge of a stage element in the plan.
   Confirm its foot line lands on that edge with a magenta diamond, and that its ends line up with
   the deck's corners so a run of rail closes the side. Drag it well clear and confirm it stays
   where it is put.
9. In **Add stage element**, confirm **Regular feet** and **Scissor feet** each list the three
   platform sizes and nothing per leg height, that **Stairs** lists **Without** and **With**
   handrails, and that **Handrail** is a part of its own.
10. Confirm a stairs profile is told from a deck by its kind, not its name: rename **Stage Stairs**
    to something without "stair" in it and confirm it still draws as a flight of steps.

## Snapping Venue geometry by dragging

With snapping on and no key held, drag each object by its move gizmo in the plan.

1. Place a 2 × 1 m deck and a 1 × 1 m deck. Drag the small deck towards the long back side of the
   large one, half a metre in from its end. Confirm it closes up against that side with no gap and
   that a magenta line runs along the joined side, wherever along it the deck is let go. Drag it
   towards the end of the large deck and confirm its end clicks into line with the large deck's
   end, a second magenta line marking that side.
2. Drag the small deck off the large one's corner until the corners are close. Confirm they meet
   corner on corner.
3. In a front view, drag the small deck down onto the large deck's top. Confirm its feet land on
   the top.
4. Place two four-point trusses and drag one end towards the other's end. Confirm the connectors
   meet with a magenta diamond. Drag a three-point truss towards a four-point one and confirm it
   does not couple to it.
5. Repeat any drag with **Shift** held and confirm nothing snaps and nothing is marked.

## Deleting and undoing in the CAD

1. Move a truss, then select it and press **Delete**. Confirm it goes at once with no question.
   Press ⌘Z and confirm it comes back with the same number, patch and place; press ⌘Z again and
   confirm the earlier move is undone too, with no message about missing elements.
2. Select a deck and press **Backspace**; confirm it goes at once. Redo with ⇧⌘Z after undoing it.
3. Select three elements and press **Delete**. Confirm a question lists all three; **Cancel** leaves
   all three in place. Delete again and confirm; one ⌘Z brings all three back.

## Crowds

1. Add **Crowd Area**. Confirm its people differ in size in the Visualizer and in every PreViz view,
   and that reopening the show draws the same sizes again.

## The same objects in PreViz

1. Open the show in ToskLight PreViz and choose **Venue**. Confirm the **Footprint** columns,
   **Colour** and **Chain** show exactly what the desk set.
2. Change a truss's length, the curtain's colour and the chain's mode there, and rename one
   object. Save, and load the show on the desk. Confirm every change arrived and nothing set earlier
   was reset — in particular, renaming an object does not return its size to the default.
3. Select three trusses and enter `2 THRU 6` in **Footprint width**. Confirm they are 2, 4 and 6 m.

## Shows written before

1. Open a show saved before Venue objects had colours or chain ends. Confirm every Venue object
   opens at its stored size, draws in its default material, and that saving the show without
   touching them leaves them unchanged.
2. Confirm a stage element placed at another base size before bases were fixed keeps that size.
3. Open a show that patched one of the fifteen retired decks on fixed legs — `Stage Deck 2 × 1 m,
   Legs 0.4 m` and its siblings. Confirm it still draws its own modelled deck at its own leg
   height, in the Visualizer and in the PreViz plan and elevations, and that it still stands on
   its feet and snaps like a stage element. The show carries its own copy of that profile; the
   packages are withdrawn, so confirm the fixture library no longer offers the fifteen to patch.
