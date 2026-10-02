# Direct Color pages

## Purpose and status

These scenarios are the acceptance contract for TL-554: Direct (native) Color encoder pages 3 and
4, the Direct section of the full Color dialog, the first native edit, the first semantic edit of a
Direct value, mixed selections and the passive per-head Direct status. Pages 1 and 2 stay the
semantic Color controls of [36-semantic-color-controls.md](36-semantic-color-controls.md). Nothing
here applies while the runtime reports programming contract 0: the legacy Color pages and dialog
stay in force.

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
- Root Playwright: `tests/112-color-intent.spec.ts` DIRECT-COLOR-001. It skips under
  `npm run test:e2e` (production contract 0) and runs under `npm run test:e2e-semantic`.

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

1. Select a fixture with more than eight native colour controls and press **Expand**.
2. Verify **Direct color** shows `Reference: <number> · <name> · <head>` and every control beyond
   pages 3/4 as a touch encoder; wheel and macro functions are listed as choices.
3. Touch another head under **Reference head**. Verify pages 3/4 and the section follow it and that
   nothing was sent to the Programmer.
4. Turn an overflow control. Verify it sends the same Direct edit as an encoder of pages 3/4.

## DIRECT-COLOR-003 — Exact replay versus best-effort match, passively

1. After DIRECT-COLOR-001 step 4, open the full Color dialog.
2. Verify the Direct rows read **Native replay** for A1 and A2 and **Best-effort match** with
   *different fixture type* for B; UV is described separately; an exact native replay never reads
   as an exact colour (the approximation shows the measured match).
3. Program a Direct recipe whose appearance is unknown on B. Verify **Native only · appearance
   unknown**: B keeps its visible colour, nothing white is invented, UV parks off when unknown.
4. Verify no toast, alert, focus move or modal appears at any point.

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

## DIRECT-COLOR-006 — Quiet holds

1. Select fixtures whose reference head outputs no colour yet and turn a native encoder. Verify
   nothing changes and nothing is reported as an error.
2. Turn a native encoder after the displayed output has moved on. Verify the first detent is held,
   the desk re-reads, and the next detent applies.
