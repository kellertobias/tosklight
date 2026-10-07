# Position operator controls

## Purpose and status

These scenarios are the acceptance contract for TL-549, refined by TL-652 (readouts, ranges, target
provenance and Angle Dynamics about a Target) and TL-651 (creating and managing Points). They cover the production Position
encoders and the modal Position Special Dialog under the fixture-independent programming contract.
Pan and Tilt are angles in degrees, Pan is unwrapped, and X, Y and Z are Target offsets in metres.

Executable coverage:

- Vitest: `apps/light-desktop/src/components/modals/specialDialogs/semanticPosition/` and
  `…/parameterControls/familyEncoders/useFamilyEncoderBinding.position.test.tsx`. The second file
  drives the hardware/OSC path from the server `desk_action` operator event.
- The Storybook mockup spec `fixture-abstraction-mockup.spec.ts` covers geometry and held-joystick
  behaviour on the shared `PositionDialog`.
- The root Playwright spec `tests/117-position-operator-controls.spec.ts` covers POSITION-CONTROLS-001,
  002, 003, 004, 006, 007, 008, 009, 010 and 011. Production reports programming contract 1, so it runs under
  `npm run test:e2e` as well as `npm run test:e2e-semantic`, where a missing semantic publication
  fails instead of skipping.
- POSITION-CONTROLS-002 drags the Pan circle to +450° and back to −450° and checks ±90° and
  **UND** against the Programmer, including an open dialog following **UND** of its own ±90° step
  and the next step starting from the undone value.
- POSITION-CONTROLS-006 arms Preload through the Preload lifecycle route and leaves it with Blind
  off, so the pending Preload values stay. It reads the lane of every dialog write from the request
  the desk sends. Step 1 runs the held joystick from requested Preload Angles. The adoption of the
  displayed pose in Preload (POSITION-CONTROLS-005 step 4) opens the dialog before the Pending
  lane has published and ticks frames until the dialog reads it. Step 2 is covered: the gesture's
  Preload part ends with one Preload Finish, the rest of the held motion continues on the Normal
  Programmer with its own Finish, Preload no longer changes, and the next gesture is Normal too.
- POSITION-CONTROLS-012 and 013 (TL-651, Points) are in `tests/128-position-points.spec.ts`. Vitest
  covers the picker's Create Point and Manage Points on the software and hardware-connected Point
  slot (`familyEncoders/FamilyEncoderSlotSurface.point.test.tsx`), the Points view and its
  one-shot Create Point request (`windows/PatchWindow.test.tsx`) and the Point helpers
  (`components/setup/points/pointManagement.test.ts`).
- POSITION-CONTROLS-005 has no Playwright case yet. Its provenance and range wording is covered by
  POSITION-CONTROLS-010 and by Vitest in `familyEncoders/positionReadouts.test.ts`.
- POSITION-CONTROLS-010 and 011 use two AURO SPOTs 4 m apart on a truss 6 m upstage, the second one
  unpatched, and a 3D Point downstage centre. Both movers are stored as Group 1 and the group is
  selected, so the Programmer holds Position as one group value. Every Target edit is a software
  encoder gesture: a Point step or a keyboard step on X or Z. POSITION-CONTROLS-011 starts the Angle
  Dynamic through the Dynamics start route, the same request the Dynamic pool sends. Its Pan/Tilt
  Middle/Amplitude lanes about Current are the ones the Dynamics editor authors (BENCH-DYNAMIC-EDITOR-001,
  `tests/127-dynamic-editor-position-circle.spec.ts`). The Rust test
  `actual_live_angle_current_follows_a_changed_base_target_while_the_dynamic_runs`
  (`…/physical_adapter/position/tests/numeric.rs`) checks the same frame-by-frame Current at the
  runtime boundary.
- Every case starts from scratch: a fresh show patched from the shipped library and an empty
  Programmer. The rig is two Cameo AURO SPOT Z300, whose profile carries a nominal Position physical
  graph (TL-637), so the displayed output seeds the first Position edit. POSITION-CONTROLS-008 uses
  two copies of a user profile made from the GLP JDC1 whose Tilt has no continuous function, so
  no Position physical data can be derived. Every shipped mover, including Tilt-only ones such as
  the JDC1, now gets an authored or derived nominal model and is programmed normally.

Physical aiming must be verified by hand on real fixtures. Record the build, the desk mode, the
viewport and the fixtures with every manual run.

Run each visual check at 1496×761, 1024×768 and 760×900, in both the software-only and the
hardware-connected layout.

## POSITION-CONTROLS-001 — Pages, modal geometry and inert navigation

1. On a desk reporting the semantic programming contract, select two moving heads and open the
   **Position** tab.
2. Verify that encoder page 1 shows **Pan** and **Tilt** in degrees, with the other encoders
   unassigned.
3. Press **Position** again. Verify that page 2 shows **Point**, **X**, **Y** and **Z**. A 6-encoder
   layout fills pages from left to right instead: page 1 shows Pan, Tilt, Point, X, Y and Z.
4. Verify that visiting either page leaves the programmer revision unchanged.
5. Open **Special Dialog**. Verify that it opens directly as a standard modal titled **Position**,
   with a close button, and contains:
   - the Pan circle, with **−90°**, **Reset** and **+90°** and a turn readout, above the Tilt
     fader on the left;
   - a square Aim joystick on the right, with width equal to height within 1 px.
6. Verify that the modal has none of the following: an Aim reference, X/Y/Z inputs, a Point
   chooser, a fixture drawing, GLB or image, a tools panel, a status footer or extra upper
   controls. **Return Home** sits below the joystick (POSITION-HOME-001).
7. Close with the close button and with **Escape**. Each time, verify that the programmer revision
   is unchanged.
8. Repeat on a contract-0 desk. The legacy Position dialog with its normalized Return Home is
   shown, and the encoders keep their normalized behaviour.

## POSITION-CONTROLS-002 — Unwrapped Pan and the ±90° actions

1. Program the selection to Pan 0°, Tilt 20° and open the dialog.
2. Drag the Pan handle clockwise across 0° several times, to about +450°. Verify that the value
   and the turn readout keep accumulating (+1.25 turns) and never wrap.
3. Drag back to about −450°. Verify that the value stays signed and unwrapped.
4. Press **+90°** and **−90°**. Verify that each moves Pan by exactly 90° and makes one Undo step.
5. Press **Reset**. Verify that Pan becomes 0° and that Tilt is unchanged. Press **Reset** again
   and verify that no programmer revision is made.

## POSITION-CONTROLS-003 — Hardware, keyboard and OSC encoder parity

1. Select two fixtures and connect the hardware surface, or an OSC client subscribed to the desk.
2. Send `/light/desk/encode/1 up`. Verify that Pan changes by exactly 1° as an Angle value, never
   as a normalized level.
3. Turn the software encoder 1 by one step. Verify that it makes the identical edit.
4. On page 2, send `/light/desk/encode/2 up` while the selection holds Angles. Verify that one
   request activates Target at the Origin and applies X +0.1 m. No Angle value remains.
5. Send further X/Y/Z detents. Verify that only the offsets change and the Target reference is
   kept.

## POSITION-CONTROLS-004 — The held joystick and its stop paths

1. Open the dialog and hold the joystick at its right edge without moving the pointer. Verify that
   Pan keeps increasing at about 120°/s.
2. Move slightly off centre, inside about 8% of the radius. Verify that nothing moves. Just
   outside, verify that the motion is gentle (quadratic response).
3. Verify that each of the following stops all motion at once, with no programmer revision
   arriving 500 ms later:
   - returning to the centre (the gesture stays open until release);
   - release;
   - pointer cancel, for example a touch interrupted by the system;
   - switching to another application (window blur);
   - minimising or hiding the desk;
   - **Escape** or the close button while holding.
4. Hold until an angle reaches the dialog limit. Verify that motion stops there with no further
   requests.
5. Verify that each held gesture is one Undo step.

## POSITION-CONTROLS-005 — Target readouts and one adoption

1. Put the selection into Target at a Point or the Origin, so the heads aim at it.
2. Open the dialog. Verify that Pan and Tilt show the resolved commanded angles, marked
   **Resolved**. Encoder page 1 shows the same values, labelled **Resolved**.
3. Move the target or the mount. Verify that the readouts follow without any programmer revision,
   Undo step or modified-show flag.
4. Drag Tilt by a few degrees. Verify that the first edit switches the selection to Angles,
   starting exactly from the displayed pose, and that Pan keeps its displayed value. Only one
   Angle activation is sent for the whole gesture.
5. With a selection whose resolved angles differ, verify that the encoders show their real range,
   lowest first (for example `-31.2°...12°`), never **Mixed** or an average. The dialog says
   **Relative** with the Target provenance. A dialog edit moves every fixture by the same amount and
   keeps their differences.
6. While Target is active, verify that Pan and Tilt are marked **From XYZ** (Origin), **From Point**
   (one 3D Point) or **From Target** (different references) instead of **Resolved**, on the
   encoders, the hardware encoder display and in the dialog.

## POSITION-CONTROLS-006 — Normal and Preload lanes

1. Repeat POSITION-CONTROLS-004 step 1 and POSITION-CONTROLS-005 step 4 in Preload. Verify that the
   edits author into Preload with the Programmer Fade, and that live output is unchanged.
2. Leave Preload in the middle of a held gesture. Preload changes are atomic: verify that the part of
   the gesture before the switch stays in Preload (one Undo step there) and that the rest of the
   still-held motion continues on the Normal Programmer as its own Undo step, without the Programmer
   Fade. Preload no longer changes. The next gesture writes to the Normal Programmer.

## POSITION-CONTROLS-007 — The first edit from scratch adopts the displayed output

1. Patch two moving heads whose profiles carry Position physical data, for example the Cameo AURO
   SPOT Z300, and select them with an empty Programmer.
2. Open the **Position** tab. Verify that encoder page 1 reads **Pan · Resolved** and
   **Tilt · Resolved** with the displayed output's angles.
3. Send `/light/desk/encode/1 up`. Verify that both fixtures now hold Angles: Pan is the displayed
   Pan plus exactly 1°, and Tilt is the displayed Tilt. The edit is never a silent no change.
4. Type Pan 10 on encoder 1 instead, starting again from scratch. Verify that Pan becomes exactly
   10° and Tilt keeps the displayed value.
5. Open the dialog and press **+90°**. Verify that Pan becomes exactly 100°.

## POSITION-CONTROLS-008 — Fixtures without Position physical data are quietly unsupported

1. Patch two fixtures that have no Position physical data, for example a user copy of the GLP
   JDC1 whose Tilt has no continuous function (every shipped mover gets an authored or derived
   nominal model), and select them with an empty Programmer.
2. Open the **Position** tab. Verify that encoder page 1 reads **Pan · Unsupported** and
   **Tilt · Unsupported** and that both encoders are disabled.
3. Turn the encoders on screen, on an attached hardware desk and with OSC `encode/1` and
   `encode/2`. Verify that no Position edit is sent and the programmer revision is unchanged.
4. Open the dialog. Verify that both angles read **Unsupported**, that the joystick and the
   −90°/Reset/+90° buttons are disabled, and that no error or alert appears.
5. Program a Position value for the same fixtures by other means, for example a preset. Verify that
   the encoders become editable again.

## POSITION-CONTROLS-009 — Position Preset tiles count the fixtures showing them

1. Patch two moving heads, for example the Cameo AURO SPOT Z300, and store Position 1 with Tilt
   −67.5° and Position 2 with Tilt 45° for both fixtures. Select both with an empty Programmer and
   show the Position Presets in a Preset pool.
2. Verify that both tiles read `0 / 2`.
3. Touch Position 1 and let the fade finish. Verify that Position 1 reads `2 / 2` and Position 2
   still reads `0 / 2`. The count compares the requested Position intent, so the achieved output
   pose and mechanical limits never deactivate it, and it keeps counting while the pane stays open.
4. Touch Position 2. Verify that Position 1 returns to `0 / 2` and Position 2 reads `2 / 2`.

## POSITION-CONTROLS-010 — Point and X/Y/Z on a group: Target readouts and From Point ranges

1. Patch two moving heads at different positions and a 3D Point, then unpatch one head. Store both
   heads as a group, select the group and open the **Position** tab.
2. Verify that Point, X, Y and Z read **—** before any Target is active.
3. Step the Point encoder once. Verify that the group now holds Target at the Origin and Point
   reads **Origin**. Step again. Verify that the group aims at the 3D Point and Point shows its
   name.
4. Step X up three times and Z down twice. Verify that the group's Target keeps the Point reference
   and holds X 0.3 m, Y 0 m, Z −0.2 m. The encoders read `0.3 m`, `0 m` and `-0.2 m`, not **—**,
   including for the unpatched head.
5. Verify that Pan and Tilt are marked **From Point**. Pan shows the real range of the two heads'
   commanded Pan, `min...max`, matching the published output readouts, and never **Mixed**.
6. Open the dialog. Verify that its caption names **From Point** too.

## POSITION-CONTROLS-011 — A degree-based Dynamic circles about the XYZ-resolved aim

1. With the group from POSITION-CONTROLS-010 aimed at the 3D Point, read the commanded aim of the
   patched head.
2. Start an Angle Dynamic whose Pan and Tilt lanes swing about **Current** (Sinus and Cosinus, as the
   Dynamics editor builds a circle). Verify that the head moves. The centre of its motion, the mean
   of two frames half a cycle apart, equals the Target's aim. The Programmer still holds the Target
   at the Point; it was not replaced by Angles.
3. Step X up ten times (1 m) while the Dynamic runs. Verify that the Target keeps its Point and the
   head keeps circling. The centre of the circle moves with the Target.
4. Release the Dynamic. Verify that the head rests exactly on the centre it circled last, the
   Target's current aim.

## POSITION-CONTROLS-012 — Create Point from the Point encoder, patched and unpatched Points

Points are the show's 3D Points (**ToskLight → 3D Point** fixtures). One stable identity, the
fixture UUID, serves an unpatched aim Point, a patched Point and a tracked Point alike.

1. Patch two moving heads, store them as a group and select it, with no 3D Point in the show. Open
   the **Position** tab. Verify that Point reads **No Points**, not **—**.
2. Tap the middle of the Point encoder. Verify that the picker offers **Origin**, says that the show
   has no Points yet, and offers **Create Point** and **Manage Points** in its title.
3. Choose **Create Point**. Verify that **Show > Show Patch** opens on **Points** with one new row,
   highlighted, named **Point 1**, with the next fixture ID after the highest one, **Unpatched**
   and at 0, 0, 0 m.
4. Rename it **Singer** and set X −1 m, Y 2 m and Z 1.5 m. Verify that the Patch stores the name and
   the location in millimetres and that the Point stays unpatched.
5. Add a second 3D Point patched at 2.1. Verify that the Points view lists it with its address.
6. On the desk, step Point. Verify **Origin**, then **3 · Singer**, then the patched Point, then
   **Origin** again; that the group's Target names each Point by its UUID; that the movers have a
   commanded pose aimed at the unpatched Point; that Pan reads **From Point**; and that the pose
   differs between the two Points.
7. Open the picker. Verify that the current choice is marked, and pick **3 · Singer** by name.
8. Record a cue, clear the Programmer, save the show and open the saved file. Verify that Singer
   keeps its UUID, name, location and empty address, and that the cue aims the movers exactly where
   they aimed before.
9. Repeat steps 2 and 3 on a hardware-connected desk: touching the Point display opens the same
   picker. Hardware and OSC `encode/N` detents step the same choices (POSITION-CONTROLS-003).

## POSITION-CONTROLS-013 — A deleted Point is reported, never replaced

1. With a patched 3D Point in the show, open **Show Patch > Points** and press **+ Create Point**.
   Verify that a second, unpatched Point is added.
2. Aim the group at the unpatched Point with the Point encoder.
3. Delete that Point in the Points view; **Delete** asks **Confirm delete** first.
4. Verify that the group still holds its Target at the deleted Point's UUID and that the Point
   encoder reads **Missing point**.
5. Step Point. Verify that the steps go on to **Origin** and then the remaining Point, and that the
   movers aim again.

## POSITION-HOME-001 — Return Home

Executable: `tests/65-semantic-special-dialogs-and-hardware-selection.spec.ts` (Default Stage) and
`tests/testBench/06d-profile-discrete-and-special-programmer-controls.spec.ts`
(BENCH-DISCRETE-SPECIAL-002); Vitest in `semanticPosition/PositionSpecialDialog.test.tsx`.

1. Select two moving heads in reverse patch order with a fixture without Position between them,
   and program them to different Angles (for example Pan 120°/Tilt 30° and Pan −60°/Tilt −20°).
2. Open **Position → Special Dialog** and press **Return Home**. Verify that both heads now hold
   Angles Pan 0° and Tilt 0°, the fixture without Position is untouched, and the request used the
   Programmer Fade.
3. Press **UND** once. Verify that both heads are back at their earlier Angles.
4. Connect the hardware surface and press **Return Home** again. Verify the same home Angles.
5. While Target is active, verify that Return Home switches the selection to the home Angles in one
   request. In Preload, verify that it authors into Preload; with a group selected, that it
   addresses the group.
6. Clear the selection, or select only fixtures that read **Unsupported**. Verify that Return Home
   is disabled and sends nothing.

## PROG-002 — Typed THRU ranges on the Position encoders

Executable: PROG-002 Pan in `tests/64-semantic-operator-controls.spec.ts` (software value pad and
hardware encoder modal); Vitest in `familyEncoders/FamilyEncoderSlotSurface.spread.test.tsx` and
`familyEncoders/familyEncoderBinding.test.ts`.

1. Select five moving heads in order. On encoder 1 (Pan) type `270 THRU −270 THRU 270` in the
   value modal, from the screen, the attached hardware modal and the keypad.
2. Verify that one request spreads Pan over the ordered selection: 270°, 0°, −270°, 0°, 270°, each
   head keeping its own Tilt, and that the encoder reads the range `-270°...270°`. One **UND**
   undoes it.
3. Repeat with a group selected. Verify that the group is addressed and its members follow their
   stored order.

## Physical evidence (manual)

On real moving heads, verify:

- that the joystick motion and the stops above are visible on the fixtures;
- that −90°, +90° and multi-turn Pan reach the expected mechanical positions within the fixtures'
  ranges;
- that Target offsets aim at the expected stage point.

This cannot be automated.
