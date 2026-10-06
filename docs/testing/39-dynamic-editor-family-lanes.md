# Dynamic editor family lanes

## Purpose and status

These scenarios are the acceptance contract for building Position, Color and Zoom Dynamics by
touch in the Dynamics editor (TL-648). The operator contract is
[Programming Windows › Dynamic lanes](../help/10-Desk/30-Windows/06-programming-windows.md#dynamic-lanes).

A lane on Pan, Tilt, Zoom or a Color component is a fixture-independent family lane, stored the
way the desk stores any other Position, Color or Zoom programming: Pan and Tilt as Angles in
degrees, Zoom as an opening in degrees, Color as a semantic recipe, hue/saturation or orthogonal
component. The desk never receives a percentage of a Pan, Tilt, Zoom or colour channel from the
editor, so nothing it builds is refused. Encoders, keyframe chips and curves show those units.

Executable coverage:

- Vitest: `apps/light-desktop/src/features/dynamics/editableLane.test.ts` (lane chooser, typed
  lane shapes, round trip through the editor),
  `apps/light-desktop/src/windows/dynamics/ProgrammingLaneView.test.tsx` (lane encoders in
  degrees) and `apps/light-desktop/src/features/dynamics/DynamicMutationWriter.test.ts` (the editor
  shows the lanes the desk stored when it completes the Angle pair).
- Root Playwright: `tests/127-dynamic-editor-position-circle.spec.ts`.

Rig: two `ROBE › Robin 300 LEDWash` movers in `Mode 3`, stored as Group 1. Their profile declares
Pan 450° and Tilt 300° of travel, centred on home.

## BENCH-DYNAMIC-EDITOR-001 — A two-axis Circle by touch

1. Select Group 1 and open the Dynamics window. Tap empty tile 1; the lane chooser lists attribute
   groups first. Tap **position**, then **Pan**.
2. The editor opens on Dynamic 1 with two lanes: **Pan** on *Middle / amplitude* with a Sinus, and
   **Tilt** on *Keyframes* following Current. Both are stored as Angle lanes; Tilt is the Current
   partner every Angle Dynamic carries.
3. The lane encoders read **Middle · Current** and **Amplitude · 45°**. Enter `90` on the Amplitude
   value modal; it reads **90°** and the stored Pan amplitude is 90 degrees.
4. Tap **+ Add Lane**, **position**, **Tilt**. Tilt replaces the Current partner: the editor and
   the stored Dynamic both hold exactly two lanes, Pan then Tilt.
5. Select lane 2 (**Tilt**) and choose the **Cosinus** curve function. Amplitude reads **30°**. The
   stored Tilt lane swings 30 degrees around Current on a cosine. No error is shown.
6. In **Settings → Targets**, tap **Take Selection**, go back to the pool and tap tile 1.
7. Each quarter cycle, mover 1 sits at Pan 90° sin(phase) and Tilt 30° cos(phase) around home,
   and mover 2 half a cycle further on: the beams draw circles.

## BENCH-DYNAMIC-EDITOR-002 — Color and Zoom lanes

1. Tap empty tile 2, then **color**, **Red**. The Red lane runs between **Bottom · 0%** and
   **Top · 100%** of the colour recipe.
2. Add **color › White Blend** and **focus › Zoom**. Select the Zoom lane: it runs between
   **Bottom · 10°** and **Top · 40°** of beam opening.
3. The stored Dynamic holds a semantic Red recipe lane, a White Blend lane and a Zoom lane in
   degrees, and no error is shown.
