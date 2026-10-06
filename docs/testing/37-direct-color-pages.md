# Direct Color pages

## Purpose and status

These scenarios are the acceptance contract for TL-554: Direct (native) Color encoder pages 3 and
4, the Direct section of the full Color dialog, the first native edit, the first semantic edit of a
Direct value, mixed selections and the passive per-head Direct status. Pages 1 and 2 stay the
semantic Color controls of [36-semantic-color-controls.md](36-semantic-color-controls.md).
Production reports programming contract 1 (TL-552), so these Direct pages are the shipped Color
behaviour; they are absent only on an older contract-0 runtime.

Executable coverage:

- Rust (`light-headless-runtime`): `runtime/native_color_pages_tests/` (route, full-width
  descriptors, reference choice, inertness, first native edit through the HTTP values action, the
  colour report's Direct status, holds) and `runtime/color_intent_report.rs`
  (`direct_status_tests`).
- Rust (`light-application`): `programming/live_state_tests/color_adoption_tests.rs` (seeded once,
  holds, explicit start, approximate adoption; both lanes).
- Vitest: `…/familyEncoders/nativeColorSlots.test.ts`, `familyEncoderBinding.native.test.ts`,
  `useFamilyEncoderBinding.native.test.tsx`, `…/intention/color/ColorSpecialDialog.direct.test.tsx`
  and `apps/light-desktop/src/api/colorAdoptionWire.test.ts`.
- Root Playwright: `tests/112-color-intent.spec.ts` DIRECT-COLOR-001 and
  `tests/120-direct-color-pages.spec.ts` DIRECT-COLOR-002 to 006 (shared rig in
  `tests/bench/color/directColorRig.ts`). Both run under `npm run test:e2e` and
  `npm run test:e2e-semantic`; they skip only on an older contract-0 server. The desk uses the
  Advanced presentation, so the native controls start on page 3; on the six-encoder desk page 3
  shows the first six native controls of the reference head (the four-per-page split of
  DIRECT-COLOR-001 step 3 is the route contract, asserted through the API).
  - DIRECT-COLOR-002: all steps in the browser, on a desk-saved copy of the Martin ELP CL with
    eleven native controls (three overflow: White, the Color Scene macro and a colour wheel).
  - DIRECT-COLOR-003: the first native edit and the Native-only recipe are sent through the HTTP
    values action; the Direct status rows, DMX, report and the absence of toasts, alerts, focus
    moves and extra modals are checked in the browser. The unknown appearance comes from the
    ELP CL (E), whose emitters carry no measurement.
  - DIRECT-COLOR-004: White Blend is turned on the page-1 encoder and the explicit start is chosen
    in the dialog; black stays black is checked through the integrator path. The *UV starts off*
    wording is asserted with the explicit start (the shipped calibrated rig has no recipe with a
    known appearance and an unknown UV).
  - DIRECT-COLOR-005: step 2 compares the OSC hardware `encode/2 up` with one software detent of
    Enc 2 frame by frame; step 3 is the HTTP integrator path (no reference head, no displayed
    source).
  - DIRECT-COLOR-006: step 1 is a browser detent on idle heads (the recipe starts from the
    profile defaults; Rust: `native_color_pages_tests/adoption.rs`);
    step 2 is API-level: the displayed-source lease is retired by newer accepted frames, the
    edit holds with `displayed_source_unavailable`, the desk re-reads the readouts and the next
    detent applies. The browser's own re-read after the hold is covered by Vitest
    (`features/familyEncoders/useFamilyReadouts.test.tsx`,
    `features/programmerValues/familyGestureSession.displayedSource.test.ts`).

Physical colour, UV and wheel evidence is manual (TL-523). Record the build, the desk mode, the
viewport and the fixtures with every manual run.

## DIRECT-COLOR-001 — Pages 3/4 are inert; the first native edit seeds the shown output once

1. On a desk reporting the semantic programming contract, patch two fixtures of one verified
   type (A1, A2) and one of another type (B), program a Semantic colour on all three and output
   one frame.
2. Select A1, A2, B and open the **Color** tab. Page to 3, 4, 2, 1 and back to 3; open and close
   the Color dialog; switch Easy/Advanced. Verify the Programmer revision, the Undo depth and the
   stored Color values are unchanged and that no values action was sent.
3. Verify the native encoders name the reference head (for example `Red · 101`), show full-width
   native values, and that page 3 holds the first four and page 4 the next four controls in
   path order.
4. Turn one native encoder by one detent. Verify one values action with the reference head and
   that A1, A2 and B now all hold the same Direct recipe: every control at the value the encoder
   showed, the turned control moved by one detent.
5. Turn twice more and release. Verify the recipe moved by exactly three detents (never re-taken
   from the output) and that the gesture is one Undo step.

## DIRECT-COLOR-002 — Reference head and overflow in the full Color dialog

1. Select a fixture with more than eight native colour controls, press **Expand** and open the
   **Details** tab.
2. Verify **Direct color** shows `Reference: <number> · <name> · <head>` and every control beyond
   pages 3/4 as a touch encoder; wheel and macro functions are listed as choices, the current one
   marked.
3. Touch another head under **Reference head**. Verify pages 3/4 and the section follow it and that
   nothing was sent to the Programmer.
4. Turn an overflow control. Verify it sends the same Direct edit as an encoder of pages 3/4.
5. Touch another choice of the colour wheel. Verify one Direct edit selects that slot, every other
   control of the recipe is unchanged, and the choice is now marked current. Turn the wheel's
   encoder one detent (fine or coarse) and verify it moves exactly one choice, in profile order.

## DIRECT-COLOR-003 — Exact replay versus best-effort match, passively

1. After DIRECT-COLOR-001 step 4, open the full Color dialog on its **Details** tab.
2. Verify the Direct rows read **Native replay** for A1 and A2 and **Best-effort match** with
   *different fixture type* for B; UV is described separately; an exact native replay never reads
   as an exact colour (the approximation shows the measured match). Every row names its fixture by
   number and name, also a fixture without a verified native layout, never by an internal id.
3. Program a Direct recipe whose appearance is unknown on B. Verify **Native only · appearance
   unknown**: B keeps its visible colour, nothing white is invented, UV parks off when unknown.
4. Verify no toast, alert, focus move or modal appears at any point.
5. Repeat steps 1–2 with two Generic RGB LEDs of the same mode, a profile without an authored
   colour model. Verify the Direct rows read **Native replay**: the derived colour layout is the
   native source, so a Direct value captured on one replays exactly on the other.

## DIRECT-COLOR-004 — First semantic edit of a Direct value

1. With A1 holding a Direct recipe of known appearance, turn **White Blend** on page 1.
2. Verify A1 becomes a Color Intent starting from the recipe's appearance (black stays black, a
   half-level recipe stays half level) and the dialog says *Started from an approximation of the
   Direct colour*; unknown UV starts off and says so.
3. Give A1 a Direct recipe of unknown appearance and turn **White Blend** again. Verify nothing
   changes, the Color encoders read *Choose start* and the dialog offers **Start from black** and
   **Start from white**.
4. Choose **Start from black** and turn again. Verify the edit applies from black and the dialog
   says *Started from your explicit colour*.

## DIRECT-COLOR-005 — Hardware and OSC parity

1. In the hardware-connected layout, page the Color encoders to 3.
2. Send `encode/2 up` from the attached hardware and turn the same software encoder one detent.
   Verify both send identical Direct edits naming the same reference head.
3. Verify an OSC integrator's Direct edit without a reference head uses the first verified head of
   the selection, and without a displayed source the latest accepted frame.

## DIRECT-COLOR-006 — Idle heads and quiet holds

1. Select fixtures that have no colour programmed. Verify pages 3/4 show the reference head's
   profile defaults. Turn a native encoder by one detent and verify all selected heads hold the
   reference recipe: every control at its default, the turned control moved by one detent.
2. Turn a native encoder after the displayed output has moved on. Verify the first detent is held,
   the desk re-reads, and the next detent applies.
