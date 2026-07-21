# Programmer Relative Encoders and Fade-Time Scope

## Status

**Specification only.** This plan records a future programmer, encoder, and timing behavior change. It does not implement UI behavior, programmer behavior, command-line behavior, persistence, API behavior, OSC behavior, hardware behavior, documentation, or executable tests.

## Goal

Define encoder changes as relative programmer operations by default, and define exactly where Programmer Fade time is allowed to affect live output.

This is a non-minor behavior plan because it touches live programmer output, software encoders, hardware encoders, command-line operations, Preset recall, Preload Go, channel faders, undo grouping, and operator settings.

## Relative encoder behavior

Encoder changes should be relative by default. A movement means "more" or "less" from the current value rather than "set to the pointer's absolute position."

If all selected fixtures have the same value, the UI may show an absolute-looking value, fader, or scalar readout for clarity. The underlying encoder operation still applies a relative change. Direct absolute entry remains available only as a deliberate path, such as a value modal or explicit command.

Mixed selections must preserve operator intent. A relative encoder movement should adjust the selected values by the same meaningful delta where the attribute type supports it, rather than collapsing the selection to one absolute value.

## Programmer Fade time scope

Encoder changes must ignore Programmer Fade time for live output.

Programmer Fade time should apply to:

- setting Presets where the operator expects a faded recall into the Programmer; and
- Preload Go, where the staged transition is explicitly time-based.

Programmer Fade time should not apply to:

- channel faders;
- absolute values entered through encoder direct-entry paths;
- relative values entered through software encoders;
- relative values entered through hardware encoders; or
- encoder-style wheel, scroll, drag, and step interactions.

Whether command-line `AT` operations use Programmer Fade time must be decided before implementation. That decision must be configurable in settings if command-line fade support is retained.

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

## Acceptance coverage

1. Software encoder movement applies a relative delta and does not use Programmer Fade time for live output.
2. Hardware encoder movement applies the same relative delta semantics and does not use Programmer Fade time for live output.
3. Direct absolute entry through an encoder path remains available but does not use Programmer Fade time for live output.
4. Channel faders ignore Programmer Fade time.
5. Preset recall into the Programmer applies Programmer Fade time where specified.
6. Preload Go applies its specified timing and remains compatible with the Preload Go LTP and playback review.
7. Mixed selections can be adjusted relatively without collapsing all fixtures to one value.
8. Command-line `AT` fade behavior is explicitly decided and controlled by settings if supported.
9. UI, command/API, OSC, and hardware paths report the same resulting programmer state and output timing.
10. Help and testing documentation distinguish relative encoder movement, direct absolute entry, channel faders, Preset fade, command-line fade, and Preload Go.
