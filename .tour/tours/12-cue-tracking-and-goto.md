---
slug: cue-tracking-and-goto
title: "Cue Tracking and Goto: Reconstructing the Stage"
components: [programmer, backend, engine]
order: 22
---

# Cue Tracking and Goto: Reconstructing the Stage

Operator contract: `docs/help/30-Programmer/03-programming-cues.md`,
`docs/help/40-Running-a-Show/01-cues-and-playbacks.md`, and
`docs/help/40-Running-a-Show/02-htp-ltp-and-ownership.md`. The executable contracts are
`tests/02-cues-tracking-and-arbitration.spec.ts`, `tests/02-cue-semantic-contracts.spec.ts`,
and the `CUE-*` entries described by `docs/testing/README.md`.

The important promise is that GOTO reconstructs a stage, not a history. Jumping directly to Cue 8
must produce the same tracked look as playing Cues 1 through 8.

## Stored data and compilation

`crates/light/domain/playback/src/model/cue.rs` owns stored Cue values, timing, trigger, and block/release
semantics. `crates/light/domain/playback/src/cue_tracking.rs` compiles sparse Cue changes into the tracked state
needed for arbitrary navigation. Recording enters through
`crates/light/src/programming/service/cue_recording.rs`; the active-show candidate and
lossless transaction live under `crates/light/src/programming/cue_active_show/`.

Portable Cue data is authority. The compiled tracking table is a replaceable runtime projection.

## GO and direct navigation

Surface adapters resolve a current-page or explicit Playback address, then submit
`crates/light/src/playback/command.rs` to `playback/service.rs`. The domain controls in
`crates/light/domain/playback/src/controls/navigation.rs` choose GO, GO minus, pause, resume, GOTO, or Load.
They do not replay intervening operator actions.

`crates/light/domain/playback/src/runtime/` installs the selected Cue and creates transitions.
`crates/light/domain/playback/src/contribution/` produces semantic fixture/head/attribute contributions for the
engine. Backward navigation and direct jumps use the same compiled tracking authority.

## Values that disappear

A missing value usually tracks. An explicit off, release, block, or asserted value changes that
rule. Intensity contributions participate in HTP; non-intensity lanes use LTP/ownership.
`crates/light/domain/playback/src/arbitration.rs` and `crates/light/domain/engine/src/` keep those rules separate from
Programmer LTP.

Move in Black is a dark-fixture transition policy, not stored stage authority. Its regression path
is `tests/07-move-in-black.spec.ts`. Automatic FOLLOW, TIME, and Chaser advances originate in
`crates/light/domain/playback/src/automatic.rs`, return through the render boundary, and publish the same typed
Playback event as manual navigation.

## Failure path

A Cue edit first prepares a complete candidate. Compile or revision failure leaves both the
portable file and installed runtime unchanged. Deleting the active Cue holds the resolved output
and re-anchors subsequent GO/GO minus; `tests/cueSemanticContracts/trackingScenarios.ts` proves the
inactive, active, sole-Cue, and direct-jump boundaries.

## Exercise

Without changing production state, read the CUE-002 and CUE-013 cases in
`tests/cueSemanticContracts/trackingScenarios.ts`. Draw the tracked fixture/attribute map after
each stored Cue and verify which map a direct jump installs.
