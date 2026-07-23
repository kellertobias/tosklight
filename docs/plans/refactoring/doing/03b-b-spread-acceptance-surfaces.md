# 03b-b — Cross-surface acceptance for the deterministic spread rule

## Context

Split from 03b (see its Result). The shared resolver (`light_core::resolve_spread`),
P>N entry rejection, normative unit vectors, PROG-002 multi-point command-line case,
and docs landed there. Remaining acceptance from the plan-50 spec (items 5–8):

## Work

1. Encoder-modal multi-point coverage, software-only AND hardware-connected layouts,
   intensity plus one non-intensity scalar (existing PROG-002 encoder cases cover only
   two-point ranges).
   - Hardware-connected done (PROG-002 multi-point Dimmer and Pan cases in
     `tests/31-hardware-connected-encoders.spec.ts`).
   - **Blocked for software-only:** the software encoder value modal (`SetValueDialog`
     in `apps/control-ui/src/components/control/VerticalTouchFader.tsx`) renders
     `ModalNumberInput` without `allowThrough` and submits via `Number(value)`, so no
     `THRU` expression — two-point or multi-point — can be entered through production
     controls. Only `HardwareEncoderDisplay` wires `allowThrough`/`onEditRange`.
     Plan-50 requires this surface; production support must land first, then extend
     PROG-002 with the software-only twins of the two hardware-connected cases.
2. OSC/attached-hardware keypad case: physical input continues the shared desk command
   and lands the same multi-point result exactly once.
3. Live Group / Preset / Cue recall before and after ordered-membership edits, plus a
   `DEGRP` case proving frozen per-fixture values do not follow later membership edits.
4. Compatibility case: load an existing show containing a multi-point
   `AttributeValue::Spread` without migration; assert the deterministic resolution.
5. Close the one remaining P>N entry gap: `set_group` via the v2 values wire cannot
   check control points against the group's current membership because
   `ProgrammingValuesEnvironment` carries only id sets — add ordered-membership sizes
   (one production construction site: `command_http/values_environment.rs`; four test
   rigs) and validate in `validate_group_value`.

## Verification

```sh
cargo test -p light-application -p light-server
bash tools/test.sh e2e --grep "PROG-002"
npm run test:e2e
```
