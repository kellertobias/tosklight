# Position Reference: Following a 3D Point

## Purpose

Prove that any fixture or Venue object can take a 3D Point as its Position Reference, that moving
or rotating the point carries everything referencing it in the Visualizer relative to the point's
own origin, that the 2D CAD keeps its drawing and notes the reference, and that a 3D Point offers
DMX modes whose position channels are offset binary about their middle value.

## The column appears with the first point

1. Open **Show Patch** on a show without a 3D Point. Confirm the table has no **Position
   Reference** column and the column list in **⚙ Settings → Columns** offers it as a switch that
   changes nothing yet.
2. Add **ToskLight → 3D Point** as fixture 901 with **Empty** for its address. Confirm the
   **Position Reference** column appears between **Scale** and **Layer**, that every other row reads
   **None** in it, and that the point's own row shows a dash.
3. Remove the point. Confirm the column disappears again.

## Referencing a point

1. Add the point back as 901, a **Venue → Four-Point Truss** as `0.1`, and two moving lights as 1
   and 2 hung 1 m either side of the truss centre. Place the point at the truss centre.
2. Press `[SET]` and touch the truss's **Position Reference** cell. Confirm the dialog offers
   **None** and **901 · 3D Point** and not the truss itself. Choose the point and **Set**. Repeat
   for fixtures 1 and 2 through the right-click path. Confirm all three cells read **901 · 3D
   Point** and `GET /api/v2/patch` reports the point's fixture id as each fixture's
   `position_master`.
3. Rename fixture 1. Confirm its **Position Reference** is unchanged after the write.
4. Try to give the point a Position Reference. Confirm its cell is a dash and cannot be edited.
   Through the API, send fixture 2 with `position_master` set to fixture 1. Confirm the patch is
   refused with a message that fixture 1 is not a 3D Point, and that nothing changed.

## Moving in the Visualizer

1. Open the Stage renderer. Select fixture 901 and lower **Point Z** by 1.5 m. Confirm the truss and
   both lamps drop 1.5 m together and keep their spacing along the truss. Confirm the beams still
   leave the lenses and that clicking a moved lamp selects it where it is drawn.
2. Set **Point Rot Z** to 90°. Confirm the truss and lamps swing a quarter turn about the point,
   not about the stage origin: each lamp stays 1 m from the point.
3. Return the point to its centre values. Confirm everything returns to where it was rigged and
   that the fixtures' **Location** and **Rotation** cells never changed.
4. `Fixture 1 AT Fixture 2` after moving the point. Confirm fixture 1 aims at fixture 2 where it is
   drawn, not where it was patched.

## The 2D CAD keeps its drawing

1. Load the show into the Architect. In the CAD plan, confirm the truss and lamps are drawn where
   they were rigged and that **Follows 3D Point 901 · 3D Point** is shown beside each of them, with
   fixture IDs and DMX addresses hidden as well as shown.
2. Move the point on the desk. Confirm nothing moves in the CAD.
3. Select the truss. Confirm the **Placement** tab in **Info** notes the reference. Change its
   rotation there and reload the desk's patch. Confirm the reference survived the write.

## DMX modes

1. Open **+ Add fixture → ToskLight → 3D Point**. Confirm the modes **Position 16 bit** (6 ch),
   **Position 24 bit** (9 ch), **Position 32 bit** (12 ch), **Full 16 bit** (12 ch), **Full 24 bit**
   (15 ch) and **Full 32 bit** (18 ch).
2. Patch a point in **Full 16 bit** at 1.1. Confirm the output reads 128,0 for each position axis
   and 128,0 for each rotation axis at rest. Set **Point X** to +50 m and confirm the first two
   slots read 192,0; set it to −50 m and confirm 64,0. Set **Point Rot Z** to +90° and confirm the
   last two slots read 192,0.
3. Repatch the same point in **Position 24 bit**. Confirm the footprint is 9, that rotation is no
   longer sent, and that **Point X** at rest reads 128,0,0.
4. Open a show saved with a 3D Point patched before these modes existed. Confirm it opens, the point
   still moves its slaves, and a profile update offer maps it onto **Full 24 bit**.
