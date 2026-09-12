# CAD Drawings Under the Plan

## Purpose

Prove that the Architect places a DXF or SVG venue drawing on one CAD axis, draws it under the rig
at the placement the operator gives it, prints it on the pages that ask for it, and keeps it in the
show.

## Placing a drawing

1. Open the Architect on a show with a patched rig and press **Drawings** in the title bar, between
   **Print** and **Meta**.
2. Confirm the panel offers **Add Drawing** and says that placing a DXF or SVG draws the plan under
   the rig.
3. Press **Add Drawing** and choose a venue ground plan in DXF. Confirm the file chooser offers
   `dxf` and `svg`, and that the panel reports the drawing's name, the units the file states, its
   size in metres, and how many lines it holds, before anything is placed.
4. Confirm the drawing is not yet in the show: the views are unchanged and the list is still empty.
5. Leave **Axis** on **Top down** and press **Place Drawing**. Confirm the top-down view draws the
   plan under the rig in a lighter line than the fixtures, and that the elevations do not.

## Placing it where the rig is

1. Set **X** and **Y** in metres and confirm the drawing moves by that much on the plan while the
   rig stays where it is.
2. Set **Scale** and confirm the drawing grows about its own origin. Check a known dimension — a
   stage width or a truss span — against the viewport's scale bar.
3. Set **Rotation** and confirm the drawing turns about its origin.
4. Rotate the top-down viewport a quarter turn. Confirm the drawing turns with the rig and stays
   aligned to it.
5. Switch **Show** off. Confirm the drawing leaves every view and the list still holds it; switch
   it back on and confirm it returns.
6. Place a section or front elevation on **Front to back**. Confirm it appears only on the
   elevations that show that axis, and not on the plan.
7. Set a viewport's cut planes to a slice of the rig. Confirm the drawing is unaffected: a cut view
   still shows the venue it stands in.

## Pages that print it, and pages that do not

1. Press **Print**, add a page from the top-down viewport, and confirm the sheet shows the drawing
   under the rig.
2. Open the page's cogwheel. Confirm the drawing is listed by name beside **Fixture IDs** and
   **DMX patch**, and is switched on.
3. Switch it off. Confirm the sheet on screen stops showing it while the viewport underneath still
   does.
4. Add a second page and confirm it starts with the drawing switched on.
5. Press **Export to PDF** and confirm the first page prints the rig without the venue, the second
   prints both, and the drawing is behind the fixtures on the page that prints it.

## A file that cannot be placed

1. Press **Add Drawing** and choose a DXF holding only text and dimensions. Confirm the panel says
   the file holds no lines this version can draw, names the file, and adds nothing.
2. Repeat with a file that is neither DXF nor SVG, and with a drawing far larger than the import
   limit. Confirm each is refused by name with a reason, the panel stays open, and the show is
   unchanged.

## Persistence

1. Save, close, and reopen the show. Confirm every placed drawing is still there, on its own axis,
   at the same origin, scale and rotation, and that the print pages still print the ones they did.
2. Save the show under a new name, open that copy on a machine that has never seen the DXF, and
   confirm the drawing is still under the plan.
3. Open a show saved before drawings could be placed. Confirm it opens with no drawings and its
   print pages are unchanged.
