# Focus and Zoom

Focus and Zoom are programmed in units that mean the same thing on every fixture type:
- **Zoom** is the full opening angle of the beam, in degrees.
- **Focus** is the lens travel, from 0% to 100%.

A show that still holds Zoom as a percentage of the fixture's channel is not opened. See
[Shows programmed before fixture-independent programming](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming).

## Units

- **Zoom** is always a full opening angle in degrees. The angle uses the convention the fixture
  profile measured: **Beam** or **Field** angle (see
  [Physical mapping calibration](../10-Show-Setup/11-fixture-types-and-gdtf.md#physical-mapping-calibration)).
  The desk never converts between the two. It maps the requested angle to DMX through the profile's
  calibrated Zoom curve.
- **Focus** is a lens setting from 0% (near end) to 100% (far end). It is not a distance in metres,
  and the desk never presents it as one.
- Focus and Zoom are separate. Changing Zoom never changes Focus, and each has its own Undo step and
  its own Programmer ownership.

When a fixture cannot reach the requested angle, or its profile has no Zoom calibration, the desk
keeps your requested value as programmed. It does not rewrite the value to what that fixture can do.
Iris, Beam, Shutter, Strobe and Gobo controls are unchanged and stay in their own families.

## Focus encoders

The Focus family's first encoder page holds **Focus**, **Zoom** and **Softness**, in that order.

- Focus turns in 1% steps.
- Zoom turns in 1° steps.
- A coarse turn moves ten steps.
- Softness keeps its usual fader behaviour.

On a 6-encoder layout the pages fill from left to right.

When the selection's Zoom convention is unknown or mixed, the Zoom encoder reads
*Zoom · Unsupported* and turning it changes nothing, on the screen, on a hardware desk and over OSC.
No error interrupts you, and Focus keeps working. Among the shipped fixtures, the Cameo AURO SPOT
Z300 declares the Beam convention.

The on-screen encoders, an attached hardware desk, and OSC `encode/N` messages all make the same
edits. Consecutive turns of one encoder form one Undo step, which ends shortly after you stop
turning.

## Focus Special Dialog

Select fixtures, open the **Focus** tab and press **Special Dialog**. The dialog opens directly as a
standard modal over the desk. Close it with its close button or **Escape**. Opening or closing it
changes nothing.

The dialog shows a side view of the beam:

- **Zoom: drag either edge of the beam, or the round handle above or below its end.** Wider is a
  larger opening angle. The readout shows the angle in degrees with its convention, for example
  `Beam 24.0°`.
- **Focus: drag the focus plane, the amber line across the beam.** Left is 0% (near), right is 100%
  (far).
- The handle keeps the point where you grabbed it. Pressing slightly off-centre and releasing
  without moving changes nothing.
- The Zoom handles remain reachable even when Focus sits at its far end.
- With keyboard focus on a control, the arrow keys move one step. **Shift**+arrow, **Page Up** and
  **Page Down** move ten steps. **Home** and **End** go to the selection's limits. Keys pressed in
  quick succession are all applied, in order.
- Your first Zoom change starts from the angle the fixtures currently output, so you do not need to
  program a Zoom first.
- The controls are greyed out for a moment while the dialog loads the Programmer.

The Zoom range is the selected fixtures' common Zoom range. When the selected fixture types
disagree, the full 0–180° range is offered.

Each drag is one Undo step. The drag stops on release. It also stops cleanly when the desk window
loses focus or is hidden.

The dialog writes to the lane you are capturing to:

- With Preload capturing Programmer changes, it writes to Preload and uses Programmer Fade.
- Otherwise it writes to the normal Programmer.

## Quiet states

The readouts show what you requested. A small note beside a readout explains anything unusual:

| Note | Meaning |
| --- | --- |
| *Requested · achieved not reported* | Your requested Zoom. The angle the fixture actually reaches is not reported yet. |
| *Requested · unsupported* | The selection's Zoom convention is unknown or mixed, or the fixtures' current Zoom output cannot be read in degrees. Your requested angle stays visible; no error interrupts you. Focus keeps working. |
| *Mixed* | The selected fixtures have different requested values. Dragging sets them all to the new value. |
| *Not programmed* | Nothing is programmed yet. The control sits at its lower limit until you move it. |
| *Not available* | None of the selected fixtures has this function. |
