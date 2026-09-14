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

## Curtains

1. Add **Curtain**. Set **Footprint width** to 8 m and **Footprint height** to 5 m. Confirm the
   Visualizer draws a gathered drape of that size.
2. Open **Colour**, choose a red, and set it. Confirm the cell shows the colour and the Visualizer
   draws the curtain red.
3. Open **Colour** again and press **Default colour**. Confirm the cell reads **Default** and the
   curtain is black serge again.

## Chains

1. Add **Chain** and set **Footprint height** to 3 m. Confirm **Chain top** reads **Hoist** and
   **Chain bottom** reads **Direct**, and that the Visualizer draws a hoist body above the chain and
   a shackle below it.
2. Set **Chain top** to **Direct** and **Chain bottom** to **Steelflex loop**. Confirm the hoist is
   replaced by a shackle and a loop hangs below the chain.
3. Confirm **Chain top** and **Chain bottom** show a dash on a truss, a curtain and a lamp.

## Stage elements

1. Add **Stage Element 2 × 1 m**. Confirm **Footprint width** and **Footprint depth** show a dash and
   **Footprint height** can be set between 0.1 m and 1.2 m.

## The same objects in PreViz

1. Open the show in ToskLight PreViz and choose **Venue**. Confirm the **Footprint** columns,
   **Colour**, **Chain top** and **Chain bottom** show exactly what the desk set.
2. Change a truss's length, the curtain's colour and the chain's bottom end there, and rename one
   object. Save, and load the show on the desk. Confirm every change arrived and nothing set earlier
   was reset — in particular, renaming an object does not return its size to the default.
3. Select three trusses and enter `2 THRU 6` in **Footprint width**. Confirm they are 2, 4 and 6 m.

## Shows written before

1. Open a show saved before Venue objects had colours or chain ends. Confirm every Venue object
   opens at its stored size, draws in its default material, and that saving the show without
   touching them leaves them unchanged.
2. Confirm a stage element placed at another base size before bases were fixed keeps that size.
