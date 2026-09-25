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
   that the Visualizer draws a hoist body above the chain and, with no truss in reach, a bow
   shackle alone on the end link.
2. Hang the chain so its bottom end is a hand's width above a four-point truss. Confirm a 22 mm
   purple steelflex is basketed under both top chords — down the outside of each, under them and
   across — with its two legs rising at 45° to a bow shackle whose bolt the last link hangs on;
   confirm no part of it goes round a bottom chord. Move the end over a three-point truss with
   its apex up, a two-point truss on edge, and a pipe, and confirm the sling is choked round the
   one top chord alone and goes straight up. Turn the three-point truss apex down and confirm the
   sling is basketed under its two top chords. Move the end clear of any truss and confirm only
   the shackle remains. Confirm the PreViz front and side elevations draw the steelflex
   schematically round one chord.
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
6. Add **Stairs** with **Both sides** handrails. Confirm the same footprint fields, and that the
   Visualizer draws a rail up each side with a post on every nosing, following the climb. Add it
   again with **Left**, then **Right**, and confirm the one rail is on that side as seen climbing,
   in the Visualizer and from above in the plan; confirm **No handrails** draws none. Change the
   choice under **Info → Parameters → Handrails**, save and reopen, and confirm it is kept. From
   above, confirm the flight shows a line at every nosing and an arrow up the climb, unlike a deck.
   Open a show that placed the retired **Stage Stairs with Handrails** and confirm it still draws
   its rails up both sides.
7. Add **Stage Handrail**. Confirm **Footprint width** can be set between 0.4 m and 24 m and that
   **Footprint height** and **Footprint depth** show a dash — a stage edge guard is 1 m high and no
   deeper than its posts. Confirm the Visualizer and the PreViz elevations draw posts about 1.2 m
   apart under a top rail and a knee rail, and that the plan draws the thin line of its run.
8. With snapping on, drag the handrail towards the outside edge of a stage element in the plan.
   Confirm its foot line lands on that edge with a magenta diamond, and that its ends line up with
   the deck's corners so a run of rail closes the side. Drag it well clear and confirm it stays
   where it is put.
9. In **Add stage element**, confirm **Regular feet** and **Scissor feet** each list the three
   platform sizes and nothing per leg height, that there is one **Stairs** whose choices are
   **No handrails**, **Left**, **Right** and **Both sides**, that **Handrail** is a part of its own,
   and that **Add venue element** does not offer a separate stairs with handrails.
10. Confirm a stairs profile is told from a deck by its kind, not its name: rename **Stage Stairs**
    to something without "stair" in it and confirm it still draws as a flight of steps.

## Scenic elements and generated equipment

1. Open **Add scenery**. Confirm it lists **Curtain**, **Chain**, **Disco ball**, **Stage railing**
   and **Flight rack**, and that there is one curtain rather than curtains at fixed widths. Place
   the curtain, set its width to 3 m in **Info** and confirm it draws 3 m wide in the plan and the
   Visualizer.
2. Choose **Flight rack → 12U**. Confirm the rack shows twelve panel lines on its front in the
   Visualizer and in a front elevation, stands on the floor, and that **Info** shows **Units** 12
   and **Depth** 0.6 m. Set **Units** to 4 and **Depth** to 0.8 m and confirm both views follow.
3. From **Add venue element**, place the **PA Speaker**. Confirm it stands on its cabinet with
   **Pole stand** off; turn it on and confirm a pole and three feet appear with the cabinet at the
   top, and that **Pole** changes the pole's height.
4. Choose **Add scenery → Disco ball**. Confirm **Info** shows **Diameter** 0.5 m and **Chain**
   0.25 m. Set **Diameter** to 0.8 m and **Chain** to 2 m and confirm the plan elevations and the
   Visualizer show a larger ball hanging 2 m under its hanging point, and that changing the
   diameter keeps the chain. Open a show that placed the retired **Disco Ball 50 cm** and confirm it
   still draws as before.
5. Place the **Line Array**. Confirm **Elements** is 8 and eight elements hang under the frame; set
   it to 3 and confirm the array shortens to three.
6. Confirm **Add venue element** still offers the musicians, DJ gear and other backline, but no
   longer the curtains at fixed widths, the 50 cm disco ball, the modelled racks, **PA Top**, **PA Top on a Pole Stand** or
   **Line Array Hang**. Open a show that placed any of those and confirm each still draws.

## Turning with the gizmo

1. Select a truss in the plan. Confirm an amber quarter arc sits between the gizmo's arrows. Drag
   along it and confirm the truss turns about its origin in 15° steps, with **Rotation Z** and the
   angle shown beside the gizmo; let go at 90° and confirm Info's **Rotation Z** changed by 90 and
   nothing else. Undo and confirm it turns back.
2. Repeat in a front view and a side view; confirm the arc turns about Y and X respectively.
3. Hold Shift while turning and confirm the angle goes free. Select two elements and turn them;
   confirm they turn about the gizmo together. Zoom in and out and confirm the arc stays the same
   size on screen and can still be taken.

## Duplicating by dragging

1. Select a deck and drag its gizmo with Option held (Ctrl on Windows or Linux). Confirm a copy
   moves with the pointer while the original stays, and that letting go leaves both, the copy
   selected with its own number and no DMX address. Press ⌘Z and confirm only the copy goes.
2. Start a drag without Option, press Option mid-drag, let it go, then let go of the mouse. Confirm
   a copy is still placed. Drag again without ever holding Option and confirm the deck just moves.
3. Option-drag an empty part of the plan and confirm the view pans. Duplicate from the right-click
   menu and confirm ⌘Z takes that copy away too.

## Picking and moving placed text

1. Place text over a deck with **Place text**, then pick **Select** and click the words. Confirm the
   text is outlined and Info shows **Text**, **Height** and **Position**, and that the deck is not
   selected.
2. Drag the words and confirm the text moves with the pointer; drag an arrow of its gizmo and
   confirm it moves along that axis only. Type a new X in Info and confirm it moves there. Undo each
   move and confirm it goes back; Redo and confirm it returns.
3. Change the words in Info, save and reopen the show, and confirm the moved, reworded text is still
   drawn where it was left. Repeat in a front view.

## Loading a model from Add primitive

1. Open the **Add primitive** menu and choose **Load model…**. Confirm the file picker opens for
   glTF, GLB, 3MF and OBJ files. Choose a GLB and confirm **Loading the 3D model…** shows, then the
   model stands at the stage origin, selected, in the plan and the Visualizer.
2. Move, turn and scale it in **Info**, save and reopen the show, and confirm it is still there as
   left. Choose **Load model…** again with a file that is not a model and confirm the reason is
   shown and the show is unchanged; close the picker once and confirm nothing is loaded.

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
6. On a computer whose Architect settings were saved before geometric snapping existed with the
   old **Snap to declared truss mounts** switch off (`snapToMounts: false` in the stored CAD
   settings), open Settings and confirm **Enable snapping** is on and a dragged deck snaps. Turn it
   off, reopen the Architect, and confirm it stays off.

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
2. In the PreViz plan, add a Crowd Area and, while it is still selected, set **Width** to 12 m and
   **Depth** to 6 m in Info. Confirm the plan's footprint grows to 12 × 6 m and fills with more
   rows and more people rather than larger ones, and that a width of 300 m is held at 250 m.
3. Confirm no standing person in a front or side view, or in the Visualizer, is taller than about
   1.85 m, that they stand on the floor, and that the tallest and shortest clearly differ.
4. Save, reopen and load the show on the desk. Confirm the crowd is still 12 × 6 m in the plan and
   in the Visualizer, with the same people.

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
