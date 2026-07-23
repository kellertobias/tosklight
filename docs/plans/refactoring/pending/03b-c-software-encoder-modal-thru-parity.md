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
