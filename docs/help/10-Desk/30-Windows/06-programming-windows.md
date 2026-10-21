# Programming Windows

## Quick guide

- **Record:** press `[REC]`, then a target. Groups and Presets normally merge; **REC MRG** identifies that choice. A multi-Cue list shows **REC CUE** to add the next Cue. A one-Cue list asks how to record.
- **Update:** press `[^REC]`, then an existing Group, Preset, or Cuelist. Review and confirm **Update**. Use **Update All** when new fixtures or attributes should also be included.
- **Selection:** Stage, Fixtures, Groups, and the command line share the same ordered selection. PREV/NEXT steps through it and ALL restores it.

An explicitly chosen Record/Update mode takes priority over defaults. See [Command Line Reference](../20-Programmer-and-Cues/01-command-line.md).

## Working with the pools

Select fixtures, set values, then store reusable selections in Groups, values in Presets, or looks in Cues. Pool panes are different views onto the same desk Programmer.

With no selection, tapping a populated Preset first selects its stored fixtures; a second tap recalls it. Missing fixtures are skipped, and unpatched fixtures remain usable. Record, Update, and Set take priority over this shortcut.

For Dynamics, **Take Selection** stores the target. A single Group remains live; other selections are frozen. **Projection** places fixtures in a plane, cylinder, or sphere for spatial ordering; **Phase** distributes the animation across that order. Inherit uses a live Group's mapping; Override edits it locally. Linear follows selection order and Random ignores spatial positions.

In **Speed**, tap the large tempo button to tap its Speed Group. Its beat indicator flashes on beats, stays amber while paused, and becomes dashed if the desk cannot be read. **Loop** repeats until Off; **One-shot** runs one effective cycle. Start-now/sync/boundary options separately decide when it begins.

## Reading pool tiles

Names are top-left and numbers top-right. The lower-left information depends on the pool:

| Pool | Lower-left information |
| --- | --- |
| Cuelists | Cue count and a play marker when running. |
| Groups | Members, or selected/total for a partial selection. |
| Presets | Fixtures currently on that Preset. |
| Timecode | Unarmed, Armed, or Playing. |
| Dynamics | Fixtures currently running the Dynamic. |

Record/Update overlays identify the armed operation without moving the underlying tile layout. **REC**, **REC MRG**, and **REC CUE** mean Record, Merge, and next Cue. They do not bypass confirmations.

See the close-up examples in [Groups and Presets](../20-Programmer-and-Cues/03-groups-and-presets.md) and [Virtual Playbacks](../20-Programmer-and-Cues/14-virtual-playbacks.md).

## Dynamic lanes

The Dynamics tile's X divides its symbol into **I** (Intensity), **P** (Position), **C** (Color), and **O** (Other). Filled sectors mean configured lane families, even while stopped; the fixture count indicates running use.

Tap an empty tile or **+ Add Lane** to choose an attribute. Pan/Tilt and Zoom use degrees; Hue uses degrees, Temperature uses kelvin, and level-like controls use percentages. Enter values in the displayed units.

A new Pan/Tilt lane animates around **Current** and adds a partner axis that holds Current. For a circle, add Pan and Tilt, then choose **Cosinus** for Tilt. Current follows the underlying aim, including a moving Point; releasing the Dynamic restores that aim. A missing target holds its angles rather than jumping.

RGB/Amber and Hue/Saturation are alternative base-color recipes within one Dynamic. White Blend, Temperature, and UV can accompany either.

## Return Position fixtures home

Open **Position > Special Dialog > Return Home** to set selected supported heads to **Pan 0° / Tilt 0°**. This is the fixture's home pose, not necessarily straight down. It replaces a Target with angles, follows the current Programmer mode/fade, and can be undone with **UND**.

Nothing selected means nothing moves. Record or Update explicitly if home should be stored in a Cue or Preset.

## Position encoders and Special Dialog

Position uses angles or Targets. Pan/Tilt edits select angle mode; Point/X/Y/Z edits select Target mode. Switching encoder pages alone sends nothing.

| Page | Controls |
| --- | --- |
| 1 | **Pan / Tilt** in degrees. |
| 2 | **Point / X / Y / Z**, with offsets in metres. |

Press the Position family again to switch pages. Enter a THRU range to spread values in selection order. Mixed values read as a minimum–maximum range.

### Choose a Point

Turn **Point** through Origin and the show's Points, or touch it to choose by name. **Create Point** and **Manage Points** open [Show Patch > Points](../10-Show-Setup/12-patch-fixtures-and-scenery.md#points). At Origin, X/Y/Z are stage coordinates; at a Point they are offsets from it.

**No Points** means the show has none. **Missing point** preserves an aim reference whose Point was deleted; choose another rather than expecting a replacement.

### Aim with the Special Dialog

- **Pan circle:** drag for continuous turns; ±90° makes a quarter-turn and Reset sets only Pan to zero.
- **Tilt fader:** drag or use arrow/Page keys.
- **Aim joystick:** hold away from centre to keep moving; release or return to centre to stop. Movement also stops when focus is lost or the dialog closes.
- **Return Home:** returns both axes to zero.

Point/X/Y/Z remain on encoder page 2 rather than in this modal.

**From XYZ**, **From Point**, and **From Target** identify target-derived angles. **Resolved** shows the current commanded pose before an angle edit. The first edit takes over that displayed pose. For differing fixture poses, **Relative** edits preserve their differences rather than averaging them.

Each gesture is one Undo step and follows Normal, Blind, Preview, or Preload. Fixture ranges still depend on the profile; estimated Position data is not calibrated aiming. Unsupported fixtures show that state and disable unavailable controls. See [Fixture Types and GDTF](../10-Show-Setup/11-fixture-types-and-gdtf.md) for calibration, and [Recovery](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming) for incompatible older percentage programming.

## Run fixture control actions

Open **Control > Special Dialog** for the selected fixtures' Lamp, Reset, Fan, and other authored actions. Momentary actions require holding, timed actions release on their timer, and latched actions toggle.

These are live overrides, not recordable encoder values. **Generate portable presets** creates Presets for supported fixed/indexed choices.

## Spread a Color range

In **Color > Special Dialog**, hold Shift and touch the first and last values, or Shift-drag between them. The first touch marks endpoint 1; the second applies the range. First and last selected fixtures receive the endpoints, with equal steps between them in selection order.

Hue takes the shorter path around the wheel, so 350° > 10° passes through red. Reversing selection reverses distribution. Touching without Shift resets only that control to one value. The edit follows normal Programmer modes and is one Undo step.

Use a Live or Follow Preload Stage to inspect the result. See [Channel Faders](07-channel-faders.md) for direct fixture levels.
