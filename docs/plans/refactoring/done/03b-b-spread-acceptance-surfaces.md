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

## Result

- **Item 1 (encoder modals) — hardware-connected done, software-only blocked.** Two new
  PROG-002 cases in `tests/31-hardware-connected-encoders.spec.ts` enter
  `100 THRU 0 THRU 100` through the production hardware encoder value modal over a
  5-fixture ordered selection: intensity (compact rig, normalized 1/0.5/0/0.5/1 → DMX
  255,128,0,128,255, exactly one value per fixture) and Pan (default-stage moving
  heads, same vector on the pan channels). The software-only layout **cannot express
  any THRU expression through production controls** — `SetValueDialog` in
  `VerticalTouchFader.tsx` mounts `ModalNumberInput` without `allowThrough` and submits
  via `Number(value)`; only `HardwareEncoderDisplay` wires `applyParameterRange`. Filed
  as `pending/03b-c-software-encoder-modal-thru-parity.md` (production wiring + the
  software twins of both cases). PROG-002 testing text updated in
  `docs/help/99-Development/02-test-bench-coverage.md`; `npm run manual` regenerated
  cleanly.
- **Item 2 (OSC keypad):**
  `osc_keypad_continues_the_shared_desk_command_line_and_lands_the_spread_once`
  (`command_input_tests.rs`) drives the real `/light/<desk>/programmer/<action>` OSC
  events through `handle_control_event`: both UI sessions of the shared desk read the
  identical `F1 THRU 5 AT 100 THRU 0 THRU 100` before Enter, the programmer lands the
  exact normative values in fixture-number order (storage order deliberately reversed),
  and exactly one TimedValue per fixture / one values event / one accepted
  `command_history` entry with `source == "osc"`.
- **Item 3 (recall matrix + DEGRP):** engine tests
  (`crates/engine/src/tests/spread_recall.rs`) pin live-group re-resolution across
  grow/shrink/reorder (5→6 members exercises the half-anchor expansion) and an active
  cue's `GroupCueChange` re-resolving mid-playback; server tests
  (`spread_recall_tests.rs`) prove the programmer retains `Spread` control points (not
  resolved values) through direct recall, Preset 1.1 recall, and `RECORD SET 25` → GO,
  each re-resolving after ordered-membership edits, and `DEGRP 1 AT 100 THRU 0 THRU
  100` lands frozen per-fixture values that ignore later membership edits. Every spec
  expectation held — no production mismatches.
- **Item 4 (compatibility):** `spread_compatibility_tests.rs` loads a hand-authored
  persisted show with a raw `{"kind":"spread","value":[1,0,1]}` group value: no staged
  migration (`snapshot.revision` == stored revision), control points byte-identical on
  disk after load, GO + render yields 255/128/0/128/255 in stored membership order (not
  DMX-address order), and a 4-point spread over 3 members degrades to the documented
  legacy linear sampling.
- **Item 5 (P>N wire gap) — closed.** `ProgrammingValuesEnvironment.group_ids` became
  `group_memberships: HashMap<String, usize>` (resolved via
  `light_programmer::resolve_group`, matching the command path; unresolvable derived
  groups fall back to stored membership size). `validate_group_value` rejects a
  ≥3-point spread with more control points than members on both the values and preload
  wires; pinned by
  `group_spread_with_more_control_points_than_members_is_rejected_without_mutation`.
  One production construction site (`command_http/values_environment.rs`) and the two
  rigs that name groups updated; the two lifecycle rigs use `Default` and were
  untouched.
- **Suite numbers:** application 391, server 420 (+7 new) + 14, engine 76+1, core 5,
  programmer 92 — all green; PROG-002 focused 9/9; architecture + source-size checks
  clean; full e2e **278 passed / 12 skipped / 0 failed** (baseline 276/11 + the two new
  encoder cases; one PRELOAD-006 in-suite flake on the first run passed 4/4 in
  isolation and the clean re-run had 0 failures).
- **Surprises:** (1) the software encoder modal turned out to have no THRU support at
  all — even two-point ranges are hardware-modal-only, so plan-50 parity needs
  production work (03b-c). (2) Worktree agents sharing the main repo's
  `CARGO_TARGET_DIR` left a newer-mtime stale `light-application` rlib that masked the
  renamed environment field until `cargo clean -p`; worktree Rust builds also require a
  stub `.artifacts/build/frontend/control-ui/index.html` for the RustEmbed assets.
