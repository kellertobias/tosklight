---
slug: value-spreading
title: "Value Spreading: Selection Order and Multi-Point Curves"
components: [programmer, backend, engine, control-ui]
order: 26
---

# Value Spreading: Selection Order and Multi-Point Curves

Operator contract: `docs/help/30-Programmer/02-selecting-and-setting-values.md` and
`docs/help/20-Show-Setup/05-groups-and-presets.md`. PROG-002 is protected across
`tests/01-foundational-dimmers-and-groups.spec.ts`,
`tests/31-hardware-connected-encoders.spec.ts`, and
`tests/32-software-encoder-value-modal.spec.ts`.

`0 THRU 100` and `100 THRU 0 THRU 100` are one semantic value applied over one ordered selection.
The client does not expand them into N writes.

## Input and canonical control points

Both encoder dialogs use the shared THRU submission path in
`apps/control-ui/src/components/control/ModalInputControls.tsx`. Software and hardware layouts send
the same `SetSelection` mutation with ordered fixture IDs and control points.

`crates/application/src/programming/` validates the request and resolves Group membership.
`light_core::attributes::spread_position` is the single interpolation primitive. Multiple control
points are sampled deterministically over the normalized selection position; invalid point/member
combinations fail before mutation.

## Live Groups and frozen values

A spread stored against a live Group retains its control points. Recall re-resolves them over the
Group's current ordered membership, so grow, shrink, and reorder operations change the sampled
fixtures predictably.

DEGRP freezes the current expansion into per-fixture values. Presets and Cues can retain a live
reference or embedded values according to their record mode; migration preserves the stored
`{"kind":"spread"}` body byte-for-byte when no owned field changes.

## Runtime path

Programmer, Preload, Preset recall, and Cue playback all produce ordinary semantic attribute
values. The engine does not know whether a vector came from a two-point fader range or a
multi-point curve; it resolves fixture/head/attribute contributions normally and fixture
projection encodes them to DMX.

## Failure and parity

Server validation rejects a curve with more than two points when it has more control points than
resolved members. The whole action remains atomic. OSC keypad entry uses the same shared command
line, and acceptance asserts exactly one stored value per fixture rather than a duplicated
client/server fan-out.

Executable landmarks:

- `crates/engine/src/tests/spread_recall.rs`
- `crates/server/src/runtime/tests/spread_recall_tests.rs`
- `crates/server/src/runtime/tests/spread_compatibility_tests.rs`
- `tests/31-hardware-connected-encoders.spec.ts`

## Exercise

For five members and points `[1, 0, 1]`, calculate the expected normalized values. Then inspect
`crates/core/src/attributes.rs` and the engine recall test to confirm the midpoint and ordering.
