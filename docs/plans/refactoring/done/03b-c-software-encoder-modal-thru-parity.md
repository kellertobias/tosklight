# 03b-c — Software encoder value modal THRU parity

## Context

Split from 03b-b (see its Result). Plan-50 surface parity requires the main desk's
scalar encoder value modal to accept `THRU` spread expressions; today only the
hardware-connected layout can. The software-only layout has no THRU path at all —
two-point ranges included — so the software half of 03b-b's encoder coverage was
blocked, not skipped:

- The software layout renders encoders as `VerticalTouchFader` with `directInput`
  (`apps/control-ui/src/components/control/EncoderSurfaces.tsx`); its `SetValueDialog`
  (`apps/control-ui/src/components/control/VerticalTouchFader.tsx`) mounts
  `ModalNumberInput` **without** `allowThrough`, so the THRU keypad button is a no-op
  (`ModalInputControls.tsx`).
- Even with THRU text present, submit parses `Number(value)` → NaN → silent no-op.
- `applyParameterRange` (the server-side spread path from chunk 03) is wired only from
  `HardwareEncoderDisplay` (`allowThrough` + `onEditRange`).

## Work

1. Wire the software encoder value modal to the same `applyParameterRange` server path
   the hardware modal uses: enable `allowThrough`, route a THRU expression to the range
   submission instead of `Number(value)`, and surface the server's rejection (e.g.
   P > N) visibly — no silent no-op.
2. Extend PROG-002 with the software-only twins of the two hardware-connected
   multi-point encoder cases (intensity + Pan, 5-fixture ordered selection,
   `100 THRU 0 THRU 100` → normalized 1/0.5/0/0.5/1 → DMX 255,128,0,128,255,
   one atomic mutation), and a two-point range case for the previously uncovered
   software surface.
3. Update the PROG-002 testing text and, if operator help documents the encoder value
   modal, the relevant help Markdown (then `npm run manual`).

## Verification

```sh
bash tools/test.sh e2e --grep "PROG-002"
npm run test:e2e
```

## Decision (maintainer, 2026-07-23)

The two layouts using different value-entry dialogs with different capabilities is a
bug, not a feature gap: "That shouldn't at all be the case. If it is the case, this is
a bug and it needs to be fixed." Operator help (`30-Programmer/01-command-line.md`)
already documents THRU in the touch encoder's Set value modal, confirming the
documented contract; the implementation must match it. Fix now rather than defer.

## Result

- **Root cause confirmed as the maintainer ruled:** `EncoderSurfaces` branched into two
  different value dialogs — `HardwareEncoderDisplay`'s modal (THRU-capable, routed to
  `applyParameterRange`) versus `VerticalTouchFader`'s `SetValueDialog` (no
  `allowThrough`, `Number(value)` submit → silent no-op on any THRU text). Operator
  help already documented THRU in the touch Set value modal, so the implementation was
  the bug.
- **Fix — one submission path:** the THRU parsing/submit logic was extracted out of
  `HardwareEncoderDisplay` into a single shared `submitEncoderValue` helper
  (`ModalInputControls.tsx`); both modals now call it, so the layouts cannot diverge
  again. `VerticalTouchFader` gained an `onChangeRange` prop that enables `allowThrough`
  on its dialog and routes ranges to it; `EncoderSurfaces` wires it to the same
  `controller.applyParameterRange` as the hardware branch (same discrete/write guards).
  The touch fader also now shows the shared range readout (`0%...100%`) instead of a
  flattened single percentage — the software branch had been bypassing
  `encoderNormalizedDisplay`. Single-value entry, offsets, and every other
  `SetValueDialog` user (playbacks, stage controls, `TouchValueButton`) are unchanged:
  `allowThrough` stays off where no range handler exists.
- **Coverage:** new `tests/32-software-encoder-value-modal.spec.ts` with three PROG-002
  cases through the production touch Set value dialog — two-point `0 THRU 50`
  (DMX 0,32,64,96,128 + `0%...50%` readout), multi-point intensity, and multi-point Pan
  (both `100 THRU 0 THRU 100` → exactly one value per fixture, normalized
  1/0.5/0/0.5/1, DMX 255,128,0,128,255, `0%...100%` readout) — the software twins of
  the hardware cases in `tests/31`. PROG-002 test-bench text updated to name both
  layouts as one shared THRU path; operator help needed no change (it was already
  correct); `npm run manual` regenerated cleanly.
- **Suite numbers:** control-ui unit 1981 passed; PROG-002 focused 12/12 (one
  first-run failure was the known OSC-subscribe `hardware_connected` poll flake in
  tests/31, green on re-run); architecture, source-size, and command-boundary checks
  clean; full e2e **281 passed / 12 skipped / 0 failed** (278 baseline + the three new
  cases).
- **Surprise:** the touch layout's encoder display had a second parity gap beyond
  THRU — it never showed stored range/spread readouts. Fixed here since it is the same
  one-behavior contract.
