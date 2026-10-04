# Focus and Zoom operator controls

## Purpose and status

These scenarios are the acceptance contract for TL-551: the production Focus Special Dialog and the
Focus family encoders under the fixture-independent programming contract. Zoom is a full opening
angle in degrees, in the profile's Beam or Field convention. Focus is lens travel from 0% to 100%.

Executable coverage:

- Vitest: `apps/light-desktop/src/components/modals/specialDialogs/focus/` and
  `…/parameterControls/familyEncoders/useFamilyEncoderBinding.focus.test.tsx`
- the Storybook mockup spec `fixture-abstraction-mockup.spec.ts`

The root Playwright spec `tests/118-focus-zoom-operator-controls.spec.ts` covers FOCUS-ZOOM-001 to
009. Production reports programming contract 1, so it runs under `npm run test:e2e` as well as
`npm run test:e2e-semantic`.

- FOCUS-ZOOM-006 ends a dialog Focus drag with a window `blur` and a dialog Zoom drag with the
  document becoming hidden. Both are synthetic browser events. It counts every Programmer write
  the desk sends. A real application switch and a minimised desk window remain manual checks.
- FOCUS-ZOOM-008 arms Preload through the Preload lifecycle route and leaves it with Blind off,
  and reads the lane of every write from the request. Production publishes no Preload (Pending)
  readouts yet, so the Preload drags start from a requested Preload Zoom. The first Zoom step
  from scratch in Preload is kept as an expected failure. Step 3 is covered: no write of the
  switched drag reaches the Normal Programmer, its Finish stays on Preload, and the next drag is
  Normal. The rest of the drag after the switch is not sent yet; that case is also an expected
  failure.

- FOCUS-ZOOM-002 to 005 use two Cameo AURO SPOT Z300. Its profile declares the Beam convention from
  the user manual (TL-637), with 10–25° selection limits.
- FOCUS-ZOOM-001, 007 and 009 use one AURO SPOT Z300 and one user copy of it whose Zoom declares
  degrees but no Beam/Field convention, so the selection shares none.
  Every shipped Zoom light declares a convention, so the gap is authored on a copy.
- Every case starts from scratch: nothing is programmed beforehand. The first Zoom step adopts the
  displayed output's opening in degrees, and keys are pressed at operator pace, the next one while
  the previous step is still settling.

Record the build, desk mode and viewport with every manual run.

Run each visual check at 1496×761, 1024×768 and 760×900, in both the software-only and the
hardware-connected layout.

## FOCUS-ZOOM-001 — The Special Dialog opens directly as a modal

1. On a desk reporting the semantic programming contract, select two fixtures that have Zoom and
   Focus. Open the **Focus** tab.
2. Verify that a **Special Dialog** button is offered. Press it.
3. Verify that the standard modal opens at once, titled **Focus**, with a close button. It must show:
   - no inline or Expand state;
   - no fixture drawing, GLB model or image;
   - no extra status bar, tool panel or Encoders button;
   - no change to the upper workspace.
4. Verify that the beam diagram stays inside the modal and that the labels are at least 12 px.
5. Close the dialog with the close button, then open it again and close it with **Escape**. Each
   time, verify that the encoders are restored and that no Programmer request was sent.

## FOCUS-ZOOM-002 — Drag Zoom by the beam edges

1. Program the selection to a known Zoom, for example 20°, and open the dialog. From scratch,
   stepping the dialog's Zoom control does this: the first step starts from the opening the
   fixtures currently output.
2. Verify that the readout shows the convention label and the angle, for example `Beam 20.0°`.
3. Press the upper angle handle about 9 px off its centre and release without moving. Verify that
   nothing changed.
4. Press again off-centre and drag outwards. The beam must widen smoothly from where it was
   grabbed, without a jump.
5. Verify that the Programmer shows one Zoom change in degrees and no Focus change, and that one
   **UND** restores the previous Zoom.
6. Drag a beam edge stroke instead of a handle. The opening must follow the touched point.
7. Verify that the selection's limits from the published descriptor bound the range. The 8–48°
   demo range must not appear.
8. Verify that the edge strokes and handle circles are at least 44×44 px.

## FOCUS-ZOOM-003 — Drag Focus by the focus plane

1. Drag the amber focus plane from its grab point. Verify that Focus moves in 1% steps between 0%
   and 100% and that no distance is shown.
2. Verify that Zoom is unchanged, that the Undo step is separate from any Zoom step, and that the
   focus plane target is at least 44 px wide.

## FOCUS-ZOOM-004 — Angle handles at the far focus end

1. Set Focus to 100% and Zoom to the narrowest limit.
2. Verify that both angle handles can still be pressed and dragged, and that neither is covered by
   the Focus target.

## FOCUS-ZOOM-005 — Keyboard steps

1. With keyboard focus on the Zoom control, verify:
   - the arrows move 1°;
   - **Shift**+arrow and **Page Up**/**Page Down** move 10°;
   - **Home** and **End** reach the selection's Zoom limits.
2. Repeat on the Focus control: steps of 1%, 10%, and **Home**/**End** reach 0% and 100%.
3. Verify that each key press is one Undo step.
4. Press several keys faster than the desk answers. Every step is applied, in order, and the note
   never reads *Requested · unsupported*.

## FOCUS-ZOOM-006 — Blur and hidden end the drag once

1. Start a Focus or Zoom drag and keep the pointer pressed.
2. Switch to another application, or hide the desk window.
3. Verify that the drag stops. It must end once (one Finish) and send no further change, even if
   the pointer keeps moving before release.
4. Verify that a new drag after returning works normally.

## FOCUS-ZOOM-007 — Unknown convention and refused Zoom stay quiet

1. Select fixtures whose profiles declare different Zoom conventions, or no convention.
2. Open the dialog. The Zoom label must be neutral (`Zoom`) and show *Requested · unsupported*.
3. Drag Zoom. The requested angle stays visible, nothing is sent, and no error dialog or blocking
   message appears.
4. On a desk that refuses the first Zoom edit, for example a fixture whose current Zoom output
   cannot be read in degrees (no Zoom calibration in the profile):
   - the requested angle stays visible with *Requested · unsupported*;
   - Focus still works.
5. On a fixture whose calibration cannot reach the requested angle, verify that the programmed
   value stays the requested angle and is not rewritten.
6. Open the **Focus** encoders with the same selection. Encoder 2 reads *Zoom · Unsupported* and is
   not editable. Turn it on the software encoders, on an attached hardware desk and with OSC
   `encode/2`: nothing is sent and no error appears. Encoder 1 still moves Focus.

## FOCUS-ZOOM-008 — Normal and Preload lanes

1. With Preload capturing Programmer changes, drag Zoom and Focus.
   - Only the Preload Programmer changes.
   - Live output is untouched until **Preload GO**.
   - The changes use Programmer Fade.
2. Leave Preload and repeat. Only the normal Programmer changes.
3. Switch the capture mode in the middle of a drag. The rest of that drag stays on the lane it
   started on.

## FOCUS-ZOOM-009 — Encoder page order and parity

1. Open the Focus family on the encoders. Page 1 must be Focus, Zoom, Softness.
2. Turn Focus and Zoom on the software encoders, on an attached hardware desk, and with OSC
   `encode/1` and `encode/2`. All three must make identical changes:
   - Focus moves 1%, and 10% on a coarse turn;
   - Zoom moves 1°, and 10° on a coarse turn.
3. Verify that consecutive turns of one encoder form one Undo step, and that Focus and Zoom turns
   never share an Undo step.
4. Verify that Iris, Beam, Shutter, Strobe and Gobo pages are unchanged.
