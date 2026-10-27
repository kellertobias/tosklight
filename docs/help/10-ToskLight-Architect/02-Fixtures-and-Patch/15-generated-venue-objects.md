# Size and finish Venue objects

Set dimensions on the placed object in Show Patch. In Architect's Patch sheet, turn **Show all** on to see the same fields. Each object keeps its own size and finish.

## Set dimensions

**Footprint width**, **Footprint height**, and **Footprint depth** expose the dimensions available for that object. A curtain has width and drop, a truss has length, and a stage deck has its supported base size and rise. Fixed dimensions show a dash.

Enter one value or a `THRU` spread for a selection, as with **Location**. A value outside the supported range is refused with the accepted range. Editing the name, layer, or position retains the dimensions.

Stage elements have fixed base sizes of 1 × 0.5 m, 1 × 1 m, or 2 × 1 m and an adjustable rise from 0.1 to 1.2 m. Existing objects retain their embedded profile and supported size. Stage elements, decks, and stairs are positioned by the middle of their footprint at floor level. At Z 0 the feet stand on the stage floor; raising the deck keeps the feet there.

**Stage Element** uses a scissor lift; **Stage Deck** uses regular feet. **Stage Stairs** keeps its stair geometry. The Visualizer and CAD draw those differences.

## Choose materials and chain placement

Open **Colour** to choose a finish, such as red serge or black truss. **Default colour** restores the object's material: black serge for curtains, aluminium for truss, and dark brown multiplex for staging. Existing objects retain that default finish.

For a chain, **Chain** chooses **Plain chain**, **Motor on top**, or **Motor on bottom**. A new chain defaults to a motor on top. The other end shows its fixing when a nearby truss supports it. Older chains retain their hoist position. Inapplicable cells show a dash.

## Scale an imported model

Use **Scale** when a model's authored size differs from the intended size, such as a hall modelled in centimetres. Values range from `0.01` to `100`; `1` or an empty entry restores the authored size.

The Visualizer, Architect plans, and Control's Stage use the same scale. Lamps, Crowd Areas, and multi-patch copies have no scale field. Crowd Areas use their independent width and depth instead.

## Choose a scenic object

The Venue library includes staging, curtains, mirror balls, backline, PA, flight cases, and people. Pipes, trusses, and chains use the Rigging type. Their objects have no DMX footprint.

Generated **Flight Rack**, **PA Speaker**, and **Line Array** objects expose dimensions suited to the object, such as rack units, pole height, or array elements. Existing shows keep their earlier modelled objects.

Use the [model catalogue](../../99-Appendix/01-model-catalogue.md) to identify a body, [Build the venue](../04-build-the-venue.md) for CAD placement, and [Crowd areas](../90-Advanced/02-crowds.md) for audience modes.
