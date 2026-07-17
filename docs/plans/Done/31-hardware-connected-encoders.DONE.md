# Hardware-Connected Encoder Display

## Completion

Implemented. Hardware-connected Programmer and Stage surfaces now retain six numbered feedback slots, use explicit display cards rather than draggable faders, distinguish fine turn from coarse press-turn input, format normalized and discrete targets, expose pointer value entry as a separate action, and clear all mappings in Direct mode. OSC encoder events are scoped to the attached desk alias and update the target shown in the corresponding slot.

## Status and scope

Review and correct the encoder presentation used when attached hardware is connected. The current compressed touch-fader representation must become an unambiguous display of the six physical encoder targets and their authoritative values.

## Display contract

Show exactly six stable slots, **Enc 1** through **Enc 6**, in physical order. Each assigned slot shows the attribute label, formatted target value, unit or discrete label, and relevant Color/Dynamics state. An unassigned slot remains visible and numbered instead of allowing later parameters to shift position.

Hardware-connected mode is feedback for physical encoders, not a misleading bank of draggable software faders. Physical turn and press-turn input updates the same target that the slot displays. Direct pointer editing is available only if it is intentionally designed as a separate touch action and cannot be mistaken for the hardware encoder itself.

Changing attribute family, ordered fixture selection, logical head, Direct mode, or hardware connection state must remap all slots deterministically and clear stale labels/values. The Stage dual-encoder representation must use the same numbering and feedback vocabulary while retaining its turn versus press-turn distinction.

The precise geometry needs visual review against the supported hardware-connected viewport before implementation; labels and values must remain readable without clipping or overlap.

## Acceptance criteria

1. Six numbered physical slots remain stable across short, long, missing, and discrete attribute labels.
2. Target values update from programmer state and physical encoder feedback without stale intermediate values.
3. Family, fixture, head, and connection changes remap or clear every slot predictably.
4. Unassigned slots remain visibly numbered and non-interactive.
5. Color, Dynamics, release/default, and Stage dual-encoder states remain distinguishable.
6. Measured layout checks cover representative hardware-connected sizes and long localized labels.

## Verification

- `ParameterControls.test.tsx` covers stable numbering, unassigned slots, Direct remapping, physical fine/coarse changes, discrete values, and absence of slider semantics.
- `ENCODER-DISPLAY-001` measures all six production cards at the supported hardware-connected bench viewport, checks ordering and readable bounds, drives Enc 1 through attached OSC, and verifies deterministic Direct-mode clearing.
- The production TypeScript/Vite build and focused Playwright scenario pass.
