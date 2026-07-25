# Programmer Relative Encoders, Touch Controls, and Fade-Time Scope

## Status

**Specification only.** This plan records a future programmer, encoder, touch-control, and timing behavior change. It does not implement UI behavior, programmer behavior, command-line behavior, persistence, API behavior, OSC behavior, hardware behavior, documentation, or executable tests.

**Priority: first in Next.** Encoder-originated values must become immediate and receive permanent cross-surface regression coverage before later encoder refactoring can change timing behavior.

## Goal

Define encoder changes as relative programmer operations by default, define the software touch encoder interaction model, and define exactly where Programmer Fade time is allowed to affect live output.

This is a non-minor behavior plan because it touches live programmer output, software encoders, hardware encoders, command-line operations, Preset recall, Preload Go, channel faders, undo grouping, and operator settings.

## Problem

The current software encoder controls are presented and operated like vertical faders. That suggests an absolute-value workflow: drag to a specific percentage or scalar position and write that value directly into the programmer.

For encoder-style parameter editing, operators usually need relative changes. A small movement should nudge the selected parameter from its current value, preserve the context of the current selection, and support repeated fine adjustment without forcing the operator to hit an exact absolute point on a fader.

This is especially important for attributes such as Pan, Tilt, Zoom, Focus, Iris, Gobo rotation, color parameters, location values, and other scalar controls where the useful operation is often "a little more" or "a little less" rather than "set exactly to 37%".

## Relative encoder behavior

Encoder changes should be relative by default. A movement means "more" or "less" from the current value rather than "set to the pointer's absolute position."

If all selected fixtures have the same value, the UI may show an absolute-looking value, fader, or scalar readout for clarity. The underlying encoder operation still applies a relative change. Direct absolute entry remains available only as a deliberate path, such as a value modal or explicit command.

Mixed selections must preserve operator intent. A relative encoder movement should adjust the selected values by the same meaningful delta where the attribute type supports it, rather than collapsing the selection to one absolute value.

The software encoder surface should behave as a bank of encoders instead of a bank of faders:

- turning or dragging an encoder applies a relative delta to the current selection;
- the current resolved or programmer value remains visible as feedback;
- direct absolute entry remains available as a deliberate modal or command action;
- fine and coarse adjustment are distinct and reachable without ambiguity;
- mouse, trackpad, touch, keyboard, OSC, and attached hardware paths keep compatible semantics where they intentionally expose the same operation; and
- the visual presentation no longer implies that the encoder is a playback or channel fader.

The result should feel like operating desk encoders on a touch screen, not like moving intensity faders.

## Decided touch interaction model

Encoders never write an absolute value from a gesture position. An absolute-looking readout is allowed only when every selected fixture shares the same value; that is display, not semantics.

The touch encoder card becomes a vertical zone stack with the step labels rendered directly on the card, top to bottom:

```text
+10
 +1
Set Value
 -1
-10
```

- **Tap and release** on a step zone applies that discrete relative step: `+10`, `+1`, `-1`, or `-10` in the attribute's display unit.
- **Tap and release in the center** opens the Set Value modal. The separate set-value or direct-input button is removed; the center label is the affordance.
- **Touch, hold, and drag** is a continuous relative change with rate proportional to displacement: slightly above the start point adds slowly; further up adds faster; further still, faster again. Downward movement is symmetric. The finger position controls the speed of change, never a target value. Releasing stops the change.

Step sizes per attribute family, the exact rate curve or tiers, and undo grouping remain open decisions before implementation.

## Operator model

Each software encoder card represents one editable parameter or paired encoder function. The card shows the parameter name, current value or mixed-value summary, and any target context the operator needs to trust the adjustment.

Fine adjustment must be explicit. The decided touch model provides `+1` and `-1` zones as fine steps and `+10` and `-10` zones as coarse steps. Other pointer, wheel, trackpad, keyboard, OSC, or hardware paths must define their matching fine/coarse operation before implementation.

Absolute value entry remains available through the encoder Set Value modal. Opening that modal is a separate action from relative movement. The modal may continue to support exact values, ranges, and through-spread syntax, but it must not be the only way to make normal encoder adjustments.

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

Where the software encoder and hardware encoder expose the same named action, both must reach the same command path and use the same delta scaling, clamping, wrapping, mixed-value handling, undo grouping, and feedback vocabulary. Where touch-specific interaction differs from physical encoder turn or press-turn behavior, the difference must be intentional and documented.

The hardware-connected layout may still render hardware encoder cards differently from software-only touch mode. Visual density and geometry can remain mode-specific, but the operator meaning of a relative encoder change must agree.

## Programmer Fade time scope

Encoder-originated changes must ignore Programmer Fade time for live output. This is an output-timing rule, not merely an encoder-display rule: the encoder readout, Fixture Sheet resolved value, Stage visualization, and physical output all reach the new value immediately, even when Programmer Fade is non-zero.

The immediate rule applies to:

- turning a physical hardware encoder;
- turning, dragging, scrolling, stepping, or using wheel input on a software or virtual encoder;
- moving an encoder's current fader-style touch control while that presentation still exists;
- touch movement on an encoder surface;
- absolute **Set value** entry opened from an encoder; and
- equivalent encoder-originated REST, WebSocket, or OSC operations.

An encoder operation must not create a delayed transition and must not schedule intermediate output frames from the old value to the new value. Implementations may coalesce a rapid stream of movement samples, but every accepted sample is the new immediate target and must not be stretched over Programmer Fade.

Programmer Fade time should apply to:

- setting Presets where the operator expects a faded recall into the Programmer; and
- Preload Go, where the staged transition is explicitly time-based.

Programmer Fade time should not apply to:

- channel faders;
- absolute values entered through encoder **Set value** or other direct-entry paths;
- relative values entered through software encoders;
- relative values entered through hardware encoders; or
- encoder-style wheel, scroll, drag, and step interactions.

Whether command-line `AT` operations use Programmer Fade time must be decided before implementation. That decision must be configurable in settings if command-line fade support is retained.

## Recorded timing

An ordinary encoder movement or encoder **Set value** operation has immediate live timing but must not accidentally store an explicit zero-second Cue fade. Unless the operator deliberately enters timing through an explicit timed command, the resulting programmer value carries no per-value timing override when recorded. Cue playback then uses the normal Cue or playback timing fallback.

This separates:

- immediate live busking from an encoder;
- an explicit timed programmer command;
- Programmer Fade used by supported Preset recall or Preload Go; and
- the timing later used when the value is recorded and played back.

Implementation must not solve the live-output requirement by writing a persistent `0s` per-value fade that makes every recorded Cue snap.

## Surface requirements

The same semantics must hold across:

- software-only encoder UI;
- hardware-connected encoder UI;
- physical hardware encoders;
- channel faders;
- command-line value entry;
- Preset recall;
- Preload Go;
- REST or WebSocket programmer operations where these paths are exposed;
- OSC input and feedback where these paths are exposed; and
- operator help and acceptance scenarios.

No surface may implement its own local fade or relative-delta policy that disagrees with the authoritative programmer operation.

## Required documentation changes

Implementation must update operator help and test-contract Markdown in the same behavior chunk.

In particular, [`docs/help/30-Programmer/01-command-line.md`](../../help/30-Programmer/01-command-line.md) currently says that an encoder target changes immediately while Fixture Sheet and physical output still interpolate over Programmer Fade. That statement describes the current behavior and must be replaced when this plan is implemented: encoder-originated target, resolved value, and output are all immediate.

The documentation must also explain that:

- ordinary encoder changes do not acquire a recorded `0s` timing override;
- explicit timed commands remain distinct;
- Preset recall and Preload Go retain their specified Programmer Fade behavior; and
- command-line `AT` timing follows the separately decided command-line setting.

Add or update the relevant scenario under `docs/testing/` and its coverage-catalog entry under `docs/help/99-Development/`. Do not claim executable coverage until the corresponding root Playwright and focused engine/programmer tests exist.

## Permanent regression coverage

Tests must set Programmer Fade to a clearly non-zero duration, such as five seconds, begin from a known output value, perform one encoder-originated change, and inspect authoritative resolved/output state immediately and at intermediate virtual times. Checking only the encoder label or stored target is insufficient.

Focused coverage must include:

- software-only tap-zone and hold-drag touch gestures;
- center tap opening Set Value without applying a relative movement;
- mouse, trackpad, keyboard, and accessibility focus paths for encoder cards;
- virtual encoder wheel, scroll, or step input where separately implemented;
- hardware-connected UI and a physical/OSC encoder turn;
- encoder **Set value** absolute entry;
- mixed-selection relative changes;
- channel-fader immediacy where it shares the same programmer writer;
- REST/WebSocket output projection for the accepted operation; and
- negative controls proving that Preset recall and Preload Go still use their documented fade.

The executable scenario must fail if a future refactor reattaches Programmer Fade to any encoder writer, even when the encoder's visual target still jumps immediately.

## Acceptance coverage

1. With Programmer Fade non-zero, software encoder turn, drag, touch, wheel, step, and current fader-style movement update resolved and physical output immediately.
2. Hardware encoder movement and OSC-equivalent input apply the same immediate relative-delta semantics.
3. Direct absolute **Set value** entry through an encoder updates resolved and physical output immediately.
4. Tests inspect output at the operation time and intermediate virtual times rather than accepting a target-label-only change.
5. Encoder operations do not create delayed transitions or intermediate fade frames.
6. Ordinary encoder changes record without an explicit `0s` per-value override, so later Cue timing retains its normal fallback.
7. Channel faders ignore Programmer Fade time.
8. Preset recall into the Programmer applies Programmer Fade time where specified.
9. Preload Go applies its specified timing and remains compatible with the Preload Go LTP and playback review.
10. Mixed selections can be adjusted relatively without collapsing all fixtures to one value.
11. Command-line `AT` fade behavior is explicitly decided and controlled by settings if supported.
12. UI, command/API, OSC, hardware, Fixture Sheet, Stage, and physical output report the same value and timing.
13. Software encoder cards are visually distinct from faders and do not use a vertical fader rail as their primary control.
14. The touch encoder card exposes the vertical `+10`, `+1`, `Set Value`, `-1`, and `-10` zones directly on the card.
15. Tap-and-release on a step zone applies the documented relative step instead of setting an absolute value from pointer position.
16. Tap-and-release in the center opens Set Value and does not apply an incidental relative change.
17. Touch-hold-drag applies a continuous relative change whose rate follows displacement above or below the start point; releasing stops the change.
18. Fine and coarse adjustments produce documented, repeatable step sizes.
19. Pan, Tilt, intensity-like scalar attributes, color attributes, and at least one indexed or wheel-like attribute handle relative deltas according to their documented type rules.
20. Clamping, wrapping, signed ranges, released values, Highlight, Preload, and unpatched fixtures behave visibly and safely.
21. Repeated encoder movements are grouped into useful undo entries without collapsing unrelated mutations.
22. Hardware-connected encoder display feedback remains compatible with software encoder feedback while preserving its mode-specific layout.
23. Touch, mouse, trackpad, keyboard, and accessibility focus paths can make relative changes without relying on hover-only affordances.
24. Operator help, human-readable testing scenarios, coverage catalogs, focused engine/programmer tests, and root Playwright coverage all protect the immediate-output and relative-touch rules.
