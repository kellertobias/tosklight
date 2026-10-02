# Programming Windows

Use the window that gives the clearest view of the current task; all of them operate on the same user programmer.

| Window | Primary use |
| --- | --- |
| Stage | Spatial selection, 2D/3D visualization, and Preload following. |
| Fixtures | Fixture/head rows, attributes, source ownership, ordering, and active/Cuelist filters. |
| Channels | Channel-oriented value and source inspection. |
| Groups | Reusable ordered selections, references, projection, and phase ranking. |
| Presets | Mixed, Intensity, Color, Position, and Beam pools. |
| Dynamics | Numbered Dynamic pool, Lanes, Phase, Speed, targets, and running state. |
| Cuelists / Cues | Cue content, order, timing, triggers, tracking, and execution. |
| DMX | Final universe output and diagnostic overrides. |

Selections made in Stage, Fixtures, Groups, Presets, or the command line are the same actual programmer selection. Activating [Highlight and Step Through](../20-Programmer-and-Cues/02-selecting-and-setting-values.md#highlight-and-step-through) freezes that ordered selection as its original set. PREV and NEXT replace the actual selection with one original member; ALL restores the whole frozen set. Preset, encoder, dialog, and other value changes use that actual focus and immediately reveal the real value for each touched attribute.

In the Dynamics Speed view, the large tempo button taps the chosen Speed Group. While the Dynamic follows a Speed Group, the circle in the button's upper-right corner lights at the start of every beat of that group as the desk runs it, so it follows taps, tempo changes, and changes made on another surface or over OSC. It stays amber while the group is paused, and turns to a dashed outline when the desk is not answering. The Programmer's Dynamics Speed page shows no lane chooser, because Speed applies to every lane of the Dynamic alike.

In the Dynamics Speed view, choose **Loop** to repeat until Off or **One-shot** to run one complete effective cycle and stop automatically. Run Mode is separate from Start now, Join sync now, and Next boundary, which determine when and where the cycle begins. A completed one-shot does not restart merely because its Cue, Programmer, or Playback value remains active; trigger it again with a new deliberate activation.

The Dynamics editor separates target ordering from phase distribution. **Projection** places
fixtures in a 2D plane and nothing else; **Phase** turns that plane into the one-dimensional order
each lamp takes its phase from.

**Projection** starts from the saved live Group's mapping. **Inherit** follows the Group;
**Override** enables the local controls and a **Projection** toggle choosing Planar, Cylindrical,
or Spherical. All three are placed by one position and one direction, and each offers only what it
reads: Planar takes a direction and a Rotation and reads no position, while Cylindrical and
Spherical take a position and are aimed by Azimuth, Elevation, and Rotation — Azimuth swings the
direction, Elevation lifts it, and Rotation is the roll about the result that decides where the
spread starts. Angles are set in whole degrees. A simplified Stage view beside the values shows the
shape, its axis, and where the spread starts, with the selected lamps as dots — or every lamp when
nothing is selected — so the shape can be placed against the real rig; drag it to orbit. Changes are saved
as they are made, so there is no Apply button; if the Dynamic changed elsewhere, authority reloads
and your values are kept. Frozen or targetless Dynamics instead show **Selection order (no Group
mapping)**. On the encoders, projection takes two pages: the kind and its position, then its
direction and rotation.

**Phase** owns the ordering — **Inherit**, Linear, Grid, Radial, Radar, or Random — alongside
offset, span, anchors, blocks, repeats, wings, and uniform/per-lane phase behavior. **Inherit**
takes the Group's ordering and appears only for a Dynamic bound to a live Group, because anything
else has nothing to inherit from. **Random** ignores fixture positions and projection.

With an empty programmer selection, the first ordinary tap on a populated Preset selects every
fixture or logical head for which that Preset stores a value; it does not recall values. Tap the
Preset again to recall it onto that selection. The selection is immediately shared with Stage,
Fixtures, the command line, OSC, and attached controls. Unpatched fixtures remain selectable.
Targets that no longer exist are skipped and reported without substituting another fixture. An
empty Preset slot remains inactive unless a recording workflow is armed. Record/Store, Update, and
Set keep priority over this selection shortcut.

## Return Position fixtures home

Open **Position → Special Dialog** and press **Return Home** below the Aim joystick to return the current ordered selection to its home pose: **Pan 0°** and **Tilt 0°**, the centre of each fixture's travel, where its Position physical data points the beam. Every selected head with Position goes home, including a selection that reads **Mixed**, and a Target is replaced by those angles. Fixtures without Position are skipped. A selected group is addressed as that group. With no selection, or when the selected fixtures are **Unsupported** (see below), Return Home is disabled and never addresses every moving light in the show.

Return Home is one normal programmer gesture. It follows Programmer Fade and the current Blind, Preview, or Preload mode, and one **UND** restores the preceding programmer values. Record or Update the result when it should become show data. Return Home itself does not edit fixture profiles or save values into a Cue or Preset.

## Position encoders and Special Dialog

Position programs fixture-independent angles and targets instead of raw Pan and Tilt channel levels. A show that still holds Pan and Tilt as channel percentages is not opened; see [Shows programmed before fixture-independent programming](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming).

**Encoders.** Position has two encoder pages. Page 1 holds **Pan** and **Tilt** in degrees; page 2 holds **Point**, **X**, **Y** and **Z**, with the offsets in metres. Press the **Position** family button again to switch pages. Switching pages only changes what the encoders show; it never activates Angle or Target and sends nothing. Software encoders, the keyboard, attached hardware and OSC `encode/N` controls all make the same edit: up and down move one step (1° or 0.1 m), and the coarse direction moves ten. Typing a range such as `270 [THRU] −270 [THRU] 270` in an encoder's value modal spreads that angle or offset over the ordered selection, exactly as an intensity range does; the encoder then reads **Mixed**.

Angle and Target are exclusive. Turning Point, X, Y or Z while the selection holds Angles, or has no semantic Position value yet, activates Target at the Origin, or at the chosen Point, and applies that offset in the same edit. Turning Pan or Tilt while Target is active switches back to Angles.

**Special Dialog.** Open **Position → Special Dialog** for the modal Position editor:

- the Pan circle sits above the Tilt fader on the left;
- the square Aim joystick sits on the right.

The modal has no Point or X/Y/Z controls; use encoder page 2 for those. **Return Home** sits below the joystick (see [Return Position fixtures home](#return-position-fixtures-home)). Opening, focusing or closing it changes nothing. **Escape** and the close button close it.

- **Pan circle.** Pan is unwrapped. Dragging around the circle keeps adding turns, and the turn readout below shows the count; the stored angle never wraps to ±180°. **−90°** and **+90°** move Pan by a quarter turn. **Reset** sets Pan to 0° and leaves Tilt unchanged; at 0° it changes nothing.
- **Tilt fader.** Drag, or use the arrow and Page keys, to set Tilt.
- **Aim joystick.** Hold the joystick away from its centre to keep Pan and Tilt moving, even without moving the pointer. Movement near the centre is gentle: there is a small dead zone, then speed rises with the square of the deflection, up to full speed at the edge. Arrow keys on the focused joystick move it the same way until released.
- **Stopping.** Movement stops immediately when you return to the centre, release, or when the pointer is cancelled or lost. It also stops when the desk window loses focus or is hidden, when you close the dialog, or when an angle reaches its limit.

**Resolved values.** While Target is active, Pan and Tilt in the dialog and on encoder page 1 show the resolved commanded angles of the output you see, marked **Resolved**. Watching them changes nothing. The first real Pan, Tilt or joystick edit takes over that exact displayed pose once, then applies the edit. It never takes a newer pose that you had not seen. When the selected fixtures disagree, the angle reads **Mixed**. The dialog then marks the values **Relative** and moves every fixture by the same amount, keeping their differences.

Each press-and-release, key step or button press is one gesture and one **UND** step. The gesture follows the current mode: Normal programmer edits apply immediately, and Preload edits use Programmer Fade. A selected group is addressed as that group.

**Unsupported fixtures.** Programming Pan, Tilt or a Target needs Position physical data: which channel turns which axis, in degrees. A mover whose fixture type carries none is given a nominal one from its Pan and Tilt channels when it is patched, so it is programmed in degrees as well; its angles are estimated, not calibrated (see [Fixture types and GDTF](../10-Show-Setup/11-fixture-types-and-gdtf.md)). A Position Dynamic applied to fixtures that have no Position programmed starts from each fixture's default pose, so you do not need to set a Position first. A Cue that fades a Position in over a fixture with no previous Position fades from that default pose too, and the first encoder turn after a show opens edits from the pose the fixture is about to output. Only fixtures that cannot be described this way, such as a Tilt-only fixture, are Unsupported. When none of the selected fixtures has Position data and nothing is programmed yet, encoder page 1 reads *Pan · Unsupported* and *Tilt · Unsupported*, the dialog shows **Unsupported** under the angles, and the encoders, joystick and buttons send nothing, on the screen, on a hardware desk and over OSC. No error interrupts you. Once a Position value is programmed for those fixtures, for example by recalling a preset, the encoders edit it again. The shipped **Cameo AURO SPOT Z300**, **JB-Lighting JBLED A7**, **ROBE Robin DLS Profile** and **Martin MAC 300** carry authored data.

The approved interaction defaults apply until fixture data publishes its own values. The joystick moves at most 120°/s Pan and 90°/s Tilt. The dialog offers −720° to +720° Pan and −135° to +135° Tilt, widened to include the current value. These are interaction limits only; each fixture still applies its own physical range.

## Run fixture control actions

Open **Control → Special Dialog** to run the selected fixtures' authored control actions. The familiar Lamp, Reset, and Fan buttons apply only where a fixture profile provides the matching action; the Fixture controls row exposes every authored action by its profile name, including Custom actions. Momentary actions remain active only while held, timed actions release on the fixture profile's timer, and latched actions toggle on and off.

Control actions are live fixture overrides, not recordable encoder values. Use **Generate portable presets** in the same dialog when fixed or indexed fixture functions should become portable Preset choices for the selected fixtures.

## Spread a Color range

Open **Color → Special Dialog** to set one colour for the current selection. To spread a range, hold
Shift on the keyboard, the on-screen **SHIFT** or the attached hardware, touch the first value, then
the last; holding Shift for one drag from the first to the last value does the same. The first touch
only marks endpoint **1**; nothing is written until the last endpoint completes the range. The
markers **1** and **2** and the value readout then show the range.

The first selected fixture receives the first value, the last receives the last, and the fixtures
between receive equal steps in the current selection order. Endpoints keep the order they were
touched, so Saturation 80% to 20% runs downwards. Hue takes the shorter way round the colour wheel:
350° to 10° passes through red, never through cyan, and a range of exactly half the wheel turns
clockwise, towards increasing hue. Equal endpoints give every fixture the same value. Touching a
control again without Shift returns only that control to one value and keeps every other range.
Reversing the selection reverses which fixtures receive the steps.

Each touch or drag is one normal Programmer gesture and one Undo step. Blind, Preview, Preload,
Record, and Update use the same programmer behavior as other Color edits. With nothing selected the
dialog changes nothing and shows no message.

The Fixture Sheet is also the on-desk Highlight-state view: original-set rows remain subtly selected while the active step is prominent, including on multi-head rows and master rows shown while subheads are hidden. The command bar replaces its DMX-rate text with `Highlight` while HIGH is active but adds no separate status panel; neither does the hardware simulator.

Pane settings are local to that pane. A Stage pane can follow Preload while another shows live output; a Preset pane can remain on Position while another shows Color.

See [Channel Faders](07-channel-faders.md) for the current Channels workflow. Use Dynamics for
animated Programmer values and Dynamic Playback assignments; use a Live or Preload-following Stage
window to inspect their authoritative output.
