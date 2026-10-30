# Track Sheet

The Track Sheet shows one Cuelist as a spreadsheet. Cues run across, and every fixture attribute the Cuelist stores runs down. Use it to see where a value is set, where it tracks on, and where it is blocked or released.

Open it from **Open Window → Playback & Automation → Track Sheet**. Choose the Cuelist in the **Cuelist** field.

## Reading the sheet

Each cell shows the value the Cuelist supplies to that fixture attribute in that Cue. A value recalled from a Preset shows the Preset name. A colour shows a swatch, and a position shows its Pan and Tilt angles or its Target. Other values show as programmed.

| Cell | Meaning |
| --- | --- |
| **New value** (bright) | This Cue stores a new value. |
| **Moves up** ▲ / **Moves down** ▼ | This Cue stores a higher or lower intensity than the one tracking in. |
| **Tracked** (dim) | An earlier Cue's value continues here. |
| **Blocked** ■ | This Cue stores the same value that would track in anyway. |
| **Released** × | This Cue releases the attribute; the Cuelist stops supplying it. |
| **Cue-only restore** ↺ | The desk restores the previous value after a Cue-only Cue. |
| Dashed outline | The value belongs to a Cue-only Cue and ends at the next Cue. |

A value stored on a Group shows in italics on each member fixture. A Cue header marked **Cue only** is a Cue-only Cue. The legend above the sheet repeats these states.

## Filtering

- **Fixtures** shows all fixtures, the currently selected fixtures, or the members of one Group.
- **Attributes** limits the sheet to Dimmer, Color, Position, Beam, Focus, or other attributes.
- **Changes only** hides every Cue that stores nothing for the visible rows.

Only the cells on screen are drawn, so large Cuelists scroll smoothly by touch or wheel.

## Editing a value

Tap a cell to show its details below the sheet: the fixture, attribute, Cue, state, value, and the Cue the value comes from. Intensity values can be edited directly in percent:

- **Update Cue** stores the value in this Cue, and it tracks on. This is the result of `UPDATE` for a value the Cue already stores, and of `UPDATE ALL` for a tracked value.
- **Cue Only** stores the value in this Cue, and the next Cue restores the value that tracked in before. If nothing tracked in, the next Cue releases it. A next Cue that stores its own value is left unchanged.
- **Update Source Cue** changes the earlier Cue the shown value comes from, like `UPDATE TRACKED`.
- **Remove Value** removes the value this Cue stores, and tracking recalculates through the following Cues.
- **Select Fixture** makes the row's fixture the Programmer selection.

Change other values, such as colours and positions, in the Programmer and use [Update](../20-Programmer-and-Cues/01-command-line.md#updating-cues). Every edit is a normal Cuelist change, so Cuelist undo applies.
