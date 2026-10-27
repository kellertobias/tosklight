# Map display regions and pixels

Open **Pixel Map** in the Pixel dock. The window shows the output's live picture beside the configuration, with the display regions and pixel zones drawn over it at the output's aspect ratio. When more than one output is enabled, choose the output above the picture.

The two title tabs switch between the two kinds of rectangle:

- **Display Regions** shows each screen's slice of the canvas in two tables. **Placement** holds **Name**, **Left**, **Top**, **Right**, and **Bottom**; **Presentation** holds **Rotation**, **Fit**, **Show**, and **Remove**. **Add display region** in the title bar adds a region covering the whole canvas.
- **Pixel Zones** shows each zone in two tables. **Placement** holds **Name**, the four edges, and the **Across** and **Down** pixel counts; **Patch** holds **Fixture type**, **Wiring** order, output **Universe** and **Address**, the resulting **Slots**, **Send**, and **Remove**. **Add pixel zone** in the title bar adds a zone at the next free address. The same tab holds the **Operating mode**, the **Output routes** table (name, Art-Net or sACN, universe, destination, send), and, in **Desk merge** mode, the desk handoff form for the selected zone.

**Operating mode > Direct** sends Pixel's sampled picture. **Desk merge** combines the desk fixture values with Pixel's pixels before sending the result. In Desk merge, configure the selected zone's desk handoff and its physical output route.

**Wiring** reads **Rows** (left to right), **Columns** (top to bottom), or the **folded** variants, which turn back at the end of each row or column.

Edges are fractions of the canvas from `0` to `1`. Selecting a row in either table, or editing one of its cells, marks its rectangle on the picture; pressing a rectangle on the picture selects its row in both tables. Only the open tab's rectangles respond to a press; the other tab's are drawn faintly for reference.

## Move and resize a region

Drag a rectangle on the picture, with the mouse or a finger, to move it; it stops at the canvas edge and keeps its size. The selected rectangle shows four corner handles; drag one to resize from that corner. A corner never crosses the opposite one, so the rectangle always keeps some size. A tap without movement only selects. With the rectangle focused, the arrow keys move it by a hundredth of the canvas (a tenth with **Shift**), and **Alt** with an arrow moves its bottom-right corner instead. Moved edges appear in the table at once, rounded to a thousandth. Moving a zone changes which part of the picture it samples, never its patch.

## Example: a centre screen with side strips

On an HDMI output, add a display region named **Centre**, set **Left** `0.333` and **Right** `0.667`, and choose **Turned clockwise**: that screen shows only the middle third of the canvas, turned. On **Pixel Zones**, add two output routes, set the second to **sACN**, then add **Left strip** (Left `0`, Right `0.05`) and **Right strip** (Left `0.95`, Right `1`), each **1** across and **30** down, on universes 1 and 2. Save. Choose the projector output and add its own regions; they may overlap the HDMI slice, because each output's regions only choose what that output shows. No map changes the canvas, the output resolution, or the monitor.

Edits stay a draft until **Save pixel map**, which stores the whole map for that output and nothing else. The button stays unavailable while nothing has changed or while the map has a problem, which is listed under the tables. Reopen Pixel Map to check the saved regions and zones.
