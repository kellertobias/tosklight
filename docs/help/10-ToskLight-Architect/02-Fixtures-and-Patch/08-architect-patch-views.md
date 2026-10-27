# Architect patch views

Open **Patch** to edit the rig in a sheet or inspect the address grid. Use [Fixtures and patch](index.md) for choosing fixtures, modes, splits, addressing, and multi-patching.

An unpatched fixture remains in the rig and can be positioned and visualized. Give it an address when it should receive DMX. Use the **Visualization** sheet view when placement matters more than addressing.

## Editing the patch sheet

The dock has four screens: **Show**, **CAD**, **Patch** and **Media**. **Patch** lists every fixture with a DMX address, including lamps, lasers, foggers, particle effects, and patched scenery.
Switch on **Show all** above its layers to include unpatched Venue objects such as trusses, stage elements, curtains, and imported models.
**+ Add fixture** then offers these objects too. Selecting a Venue object in a view switches **Show all** on. The **Patch**
title has two tabs at its right end, directly left of the window's **Settings** (⚙): **Sheet** is
the patch sheet, and **DMX** is the address grid described in "The Patch screen's DMX tab" below.

The Visualizer follows the selection of **CAD** and **Patch**. Every selected fixture, imported model
and Venue object, trusses, stage elements and curtains included, is outlined in blue there, in the
3D views and on the plan and elevations alike. Selecting more, deselecting and changing the selection
update the outline at once; clearing the selection removes it. The outline only marks the object: its
light output, position and material stay as they are, and clicking in the Visualizer picks as it
always does.

Click a column header to order the sheet by that column, for example **Fixture ID** or **Patch**;
click it again to reverse the order. An arrow marks the column the sheet is ordered by. **Patch**
orders by universe, then address. Fixtures with nothing in the column, such as unpatched fixtures
or fixtures without a note, always come last, and fixtures with the same value keep Fixture ID
order. Shift and drag ranges follow the order the sheet shows. The sheet opens ordered by
Fixture ID.

Choose which columns the sheet shows from the window's **Settings** (⚙) under **Columns**; the last
visible column cannot be switched off. Patch and Venue each remember their own columns on this
computer. Above the switches, **View** offers three quick views, which only the Architect has, the
desk's Show Patch window does not:

- **Patch**: Fixture ID, Name, Manufacturer, Product / mode, Patch, Masters, Invert Pan,
  Invert Tilt, MIB, Layer and Note.
- **Visualization**: Fixture ID, Name, Location, Rotation, Bracket, Shaper, Footprint, Colour,
  Chain, Scale, Layer, 2D, 3D and Note.
- **Compact**: Fixture ID, Name, Patch, Layer and Note.

**Footprint width**, **Footprint height** and **Footprint depth** size a generated Venue object, a
truss's length, a curtain's width and drop, a stage element's rise, and show a dash for the
measurements the object is not made to, and for everything else in the rig. Click a size to type
it in metres; a size the object cannot be built at is refused with the range it can take. The size
is kept when the object is edited here or on the desk.

**Colour** sets a generated Venue object's colour, a curtain's serge, a black truss, with
**Default colour** returning it to its own material. **Chain** chooses **Plain chain**, **Motor on
top** or **Motor on bottom**; the end without the motor has a steelflex. The Visualizer draws each object with what was chosen for it, and a desk shows and edits the
same columns in **Show Patch**.

**Scale** draws a Venue object, an imported model above all, at a multiple of the size it was
built at, from `0.01` to `100`. `1`, or an empty entry, is its built size; anything outside the
range is refused with the range. The plan, the Visualizer and a desk's Stage all draw it at that
scale, and everything that is not a Venue object shows a dash.

**View** names the quick view whose columns the sheet shows exactly; changing a single column
leaves it reading **Custom**.

**Masters** says which masters reduce a fixture: `none`, `group` (the Group Masters only), `grand`
(the Grand Master only) or `both`. Click it to choose another value; anything short of `both`
warns which master the fixture stays live under. A fixture with no intensity shows `—`.

**MIB** is Move in Black: `Off`, or the delay before the fixture moves while dark, from `0s` to
`30s` with decimals such as `2.5s`. Click it to switch Move in Black off or on and set the delay.

Click a fixture's **Note** to open the note editor.

**No Layer Assigned**, under **All fixtures**, lists every fixture on the screen that has no layer
of its own: fixtures still on the default layer, and fixtures whose layer no longer exists. While
**Show all** is off, the sidebar hides layers whose fixtures all belong to another screen, such as a Trusses layer on the Patch screen.
It also hides **No Layer Assigned** when it would be empty. An empty layer you created stays listed. Selecting fixtures in the drawing switches
**Show all** on only when that selection happens; returning to the screen later does not switch it
on again. The layer list appears once the screen's fixtures have arrived, already reduced to what
the screen shows, so no layer flashes up and vanishes as the screen opens.

To delete a layer, press the bin to the right of its fixture count and confirm. Its fixtures stay
in the show and move to **No Layer Assigned**. A locked layer must be unlocked first, and the
default layer has no bin. The bin is the Architect's; on the desk, delete a layer with the keyboard
shortcut or the hardware button.

While a layer is open, selecting stays in it: clicking, Shift and drag ranges, and Ctrl-click
only ever select that layer's fixtures, and the sheet keeps showing the layer. A selection made in
the drawing or another window opens **All fixtures** only when it includes fixtures outside the
open layer.

Right-click a value to edit it. When the fixture you right-click belongs to the selection, the
editor changes every selected fixture; when it does not, the selection is replaced by that fixture
and only that fixture is edited.

A Fixture ID range in the value entry can be left open: `1100 [THRU] [ENTER]` numbers the selected
fixtures 1100, 1101, 1102 and on, and `1101 [THRU] [−] [ENTER]` numbers them 1101, 1100, 1099 and
on. A closed range such as `1012 [THRU] 1004` still counts from its first value towards its last.

After **Enter**, the sheet scrolls to the fixture whose value you opened when the new value moved
its row out of view, for example when a new Fixture ID sorts it further down.

With several fixtures selected, **Delete** asks whether to delete or unpatch all of them. Delete
removes every selected fixture from the show and clears the selection; Unpatch keeps their fixture
lines and clears all their DMX addresses. Either choice is applied as one patch change. Fixtures
on a locked layer are left out, and a fixture outside the selection is still deleted on its own.

## The Patch screen's DMX tab

The **DMX** tab in the **Patch** title shows every channel of every patched universe as a grid of
numbered cells. A cell is lit when a fixture, one of its splits, or one of its multi-patches
occupies that address and dark when nothing does. The addresses of one fixture share a single
outline, a fixture's first address carries a mark on its left edge, so neighbouring fixtures stay
apart, and an address two patches share is drawn in orange. Select a cell to see which fixture owns
it, its patch range, split, fixture channel and attribute, and the address's DIP-switch setting.
**Sheet** returns to the patch sheet.

The tab's only window setting (⚙) is **Show sidebar**, which shows or hides the side column; this
computer remembers the choice.

To repatch a fixture, drag its block by any of its cells to the new addresses. The block keeps the
cell you took it by under the pointer, stops at the start and end of the universe, and can be
dropped into another patched universe. A split or a multi-patch copy moves on its own. Moved blocks
are outlined with a dashed line, and **Pending patch** at the top of the side column lists every
move with its old and new address. Nothing is written until **Apply Patch**, which writes every
moved fixture as one patch change; **Discard** puts the blocks back. A refused change stays pending
and shows the reason. Dragging back to the stored address takes a move off the list. Because
**Apply Patch** is in the side column, blocks can only be dragged while **Show sidebar** is on.
