# Position special dialog

Open **Position → Special Dialog** for the modal Position editor:

- the Pan circle sits above the Tilt fader on the left;
- the square Aim joystick sits on the right.

The modal has no Point or X/Y/Z controls; use encoder page 2 for those. **Return Home** sits below the joystick (see [Return Position fixtures home](#return-position-fixtures-home)). Opening, focusing or closing it changes nothing. **Escape** and the close button close it.

- **Pan circle.** Pan is unwrapped. Dragging around the circle keeps adding turns, and the turn readout below shows the count; the stored angle never wraps to ±180°. **−90°** and **+90°** move Pan by a quarter turn. **Reset** sets Pan to 0° and leaves Tilt unchanged; at 0° it changes nothing.
- **Tilt fader.** Drag, or use the arrow and Page keys, to set Tilt.
- **Aim joystick.** Hold the joystick away from its centre to keep Pan and Tilt moving, even without moving the pointer. Movement near the centre is gentle: there is a small dead zone, then speed rises with the square of the deflection, up to full speed at the edge. Arrow keys on the focused joystick move it the same way until released.
- **Stopping.** Movement stops immediately when you return to the centre, release, or when the pointer is cancelled or lost. It also stops when the desk window loses focus or is hidden, when you close the dialog, or when an angle reaches its limit.

**Resolved values.** While Target is active, Pan and Tilt in the dialog and on encoder page 1 show the commanded angles of the output you see. They are marked with where they come from:

- **From XYZ** when the selection aims at the Origin with X/Y/Z offsets;
- **From Point** when it aims at one 3D Point;
- **From Target** when the fixtures aim at different Points, or some at a Point and some at the Origin.

Without a Target, read-back angles are marked **Resolved**, for example before the first Position edit. Watching them changes nothing. The first real Pan, Tilt or joystick edit takes over that exact displayed pose once, then applies the edit. It never takes a newer pose that you had not seen.

When the selected fixtures point differently, the encoder shows the range of their angles, lowest first, for example `-31.2°...12°`. Nothing is averaged. The dialog then marks the values **Relative**, with the Target they come from, and moves every fixture by the same amount, keeping their differences.

Each press-and-release, key step or button press is one gesture and one **UND** step. The gesture follows the current mode: Normal programmer edits apply immediately, and Preload edits use Programmer Fade. A selected group is addressed as that group.

**Unsupported fixtures.** Programming Pan, Tilt or a Target needs Position physical data: which channel turns which axis, in degrees. A mover whose fixture type carries none is given a nominal one from its Pan and Tilt channels when it is patched, so it is programmed in degrees as well; its angles are estimated, not calibrated (see [Fixture types and GDTF](../../10-ToskLight-Architect/02-Fixtures-and-Patch/01-fixture-library.md)). A Position Dynamic applied to fixtures that have no Position programmed starts from each fixture's default pose, so you do not need to set a Position first. A Cue that fades a Position in over a fixture with no previous Position fades from that default pose too, and the first encoder turn after a show opens edits from the pose the fixture is about to output. Only fixtures that cannot be described this way, such as a Tilt-only fixture, are Unsupported. When none of the selected fixtures has Position data and nothing is programmed yet, encoder page 1 reads *Pan · Unsupported* and *Tilt · Unsupported*, the dialog shows **Unsupported** under the angles, and the encoders, joystick and buttons send nothing, on the screen, on a hardware desk and over OSC. No error interrupts you. Once a Position value is programmed for those fixtures, for example by recalling a preset, the encoders edit it again. The shipped **Cameo AURO SPOT Z300**, **JB-Lighting JBLED A7**, **ROBE Robin DLS Profile** and **Martin MAC 300** carry authored data.

The interaction defaults apply until fixture data supplies its own values. The joystick moves at most 120°/s Pan and 90°/s Tilt. The dialog offers −720° to +720° Pan and −135° to +135° Tilt, widened to include the current value. These are interaction limits only; each fixture still applies its own physical range.

## Return Position fixtures home

Open **Position → Special Dialog** and press **Return Home** below the Aim joystick to return the current ordered selection to its home pose: **Pan 0°** and **Tilt 0°**, the centre of each fixture's travel, where its Position physical data points the beam. Every selected head with Position goes home, including a selection whose angles differ, and a Target is replaced by those angles. Fixtures without Position are skipped. A selected group is addressed as that group. With no selection, or when the selected fixtures are **Unsupported** (see below), Return Home is disabled and never addresses every moving light in the show.

Return Home is one normal programmer gesture. It follows Programmer Fade and the current Blind, Preview, or Preload mode, and one **UND** restores the preceding programmer values. Record or Update the result when it should become show data. Return Home itself does not edit fixture profiles or save values into a Cue or Preset.

The [Position encoders](08-position.md) provide Point and X/Y/Z target controls.
