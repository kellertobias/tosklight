# Software Encoder Relative Controls

## Status

**Specification only.** This plan records a future rework of the software encoder surface. It does not implement UI behavior, programmer behavior, API behavior, OSC behavior, hardware behavior, documentation, or executable tests.

## Problem

The current software encoder controls are presented and operated like vertical faders. That suggests an absolute-value workflow: drag to a specific percentage or scalar position and write that value directly into the programmer.

For encoder-style parameter editing, operators usually need relative changes. A small movement should nudge the selected parameter from its current value, preserve the context of the current selection, and support repeated fine adjustment without forcing the operator to hit an exact absolute point on a fader.

This is especially important for attributes such as Pan, Tilt, Zoom, Focus, Iris, Gobo rotation, color parameters, location values, and other scalar controls where the useful operation is often "a little more" or "a little less" rather than "set exactly to 37%".

## Goal

Rework the software encoder surface so it behaves as a bank of encoders instead of a bank of faders:

- turning or dragging an encoder applies a relative delta to the current selection;
- the current resolved or programmer value remains visible as feedback;
- direct absolute entry remains available as a deliberate modal or command action;
- fine and coarse adjustment are distinct and reachable without ambiguity;
- mouse, trackpad, touch, keyboard, OSC, and attached hardware paths keep compatible semantics where they intentionally expose the same operation; and
- the visual presentation no longer implies that the encoder is a playback or channel fader.

The result should feel like operating desk encoders on a touch screen, not like moving intensity faders.

## Operator model

Each software encoder card represents one editable parameter or paired encoder function. The card shows the parameter name, current value or mixed-value summary, and any target context the operator needs to trust the adjustment.

Primary movement changes the value relatively. The implementation must define the movement source and scaling before work begins. Candidate inputs include horizontal drag, circular drag, wheel or trackpad scroll, repeated step buttons, keyboard focus with arrow keys, or a combination. The chosen interaction must work on touch screens as well as desktop pointers.

Fine adjustment must be explicit. It may use a modifier key, a press-and-hold mode, a segmented coarse/fine toggle, or a separate fine-control gesture. The operator must be able to make a small repeatable nudge without fighting touch precision.

Absolute value entry remains available through the existing encoder value modal or a revised direct-entry modal. Opening that modal is a separate action from relative movement. The modal may continue to support exact values, ranges, and through-spread syntax, but it must not be the only way to make normal encoder adjustments.

## Relative value semantics

Relative encoder movement changes programmer values using the same authoritative programmer service as command line, OSC, HTTP, hardware controls, and existing encoder direct entry.

The implementation must define, per attribute type:

- the unit of one normal encoder step;
- the unit of one fine encoder step;
- whether values clamp, wrap, or use fixture-profile-specific bounds;
- how signed and centered values behave;
- how indexed wheel, slot, or mode attributes react to small deltas;
- how mixed selections are adjusted while preserving relative offsets where appropriate;
- how null, released, Highlight, Preload, and unpatched fixture states are shown and changed; and
- how undo groups repeated encoder movement into operator-meaningful mutations.

Relative movement must not silently rewrite unrelated programmer values. If a selected attribute cannot be adjusted relatively, the card must make that disabled or constrained state visible.

## Software and hardware parity

This plan is about software encoders, but it must not create a competing semantics for attached hardware encoders.

Where the software encoder and hardware encoder expose the same named action, both must reach the same command path and use the same delta scaling, clamping, wrapping, mixed-value handling, undo grouping, and feedback vocabulary. Where touch-specific interaction differs from physical encoder turn or press-turn behavior, the difference must be intentional and documented.

The hardware-connected layout may still render hardware encoder cards differently from software-only touch mode. Visual density and geometry can remain mode-specific, but the operator meaning of a relative encoder change must agree.

## Surface requirements

Future implementation must update compatible surfaces together:

- software-only encoder UI;
- hardware-connected encoder UI where it mirrors or summarizes software behavior;
- keyboard and accessibility focus behavior for encoder cards;
- command/API endpoints for relative encoder deltas if they do not already exist;
- OSC input and feedback where relative software encoder actions are exposed;
- WebSocket feedback for current values and mixed summaries;
- operator help and manual screenshots; and
- acceptance scenarios under `docs/testing` plus focused executable coverage.

The UI must avoid fader-specific vocabulary, fader rail visuals, and absolute position affordances for normal encoder movement. If a direct absolute-entry affordance is visible, it must be visually secondary to the relative encoder action.

## Acceptance coverage

Future implementation must cover at least:

1. Software encoder cards are visually distinct from faders and do not use a vertical fader rail as their primary control.
2. Primary software encoder movement applies a relative delta instead of setting an absolute value from pointer position.
3. Fine and coarse adjustments produce documented, repeatable step sizes.
4. Direct absolute value entry remains available through an explicit modal or command path.
5. Pan, Tilt, intensity-like scalar attributes, color attributes, and at least one indexed or wheel-like attribute handle relative deltas according to their documented type rules.
6. Mixed selections update predictably, preserving relative offsets where the attribute type supports that behavior.
7. Clamping, wrapping, signed ranges, released values, Highlight, Preload, and unpatched fixtures behave visibly and safely.
8. Repeated encoder movements are grouped into useful undo entries without collapsing unrelated mutations.
9. Software UI, command/API, OSC where exposed, and attached hardware encoder paths reach the same authoritative programmer operation.
10. Hardware-connected encoder display feedback remains compatible with software encoder feedback while preserving its mode-specific layout.
11. Touch, mouse, trackpad, keyboard, and accessibility focus paths can make relative changes without relying on hover-only affordances.
12. Help documentation and deterministic screenshots describe encoder-relative operation and distinguish it from fader operation.

## Open decisions

Before implementation, settle:

1. Primary touch gesture: horizontal drag, circular drag, step buttons, wheel/scroll, or another control.
2. Coarse and fine step sizes per attribute family.
3. Whether mixed selections preserve offsets for all scalar attributes or only selected families.
4. Wrapping behavior for pan, rotation, indexed wheels, and bounded scalar parameters.
5. Exact undo grouping timeout and commit boundary for continuous movement.
6. OSC address shape for relative software encoder deltas, if exposed externally.
7. Whether the existing direct-value modal is reused unchanged or redesigned around encoder cards.

## Deferred work

This plan does not define a new dynamics engine, preset editor, fixture-profile schema, playback fader behavior, or channel-fader behavior. It also does not remove absolute entry from encoders; it only makes relative adjustment the normal encoder operation and keeps absolute entry explicit.
