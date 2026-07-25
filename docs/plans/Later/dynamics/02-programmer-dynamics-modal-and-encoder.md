# Programmer Dynamics Modal and Encoder

## Status

**Specification only.** This chunk records the Programmer Dynamics modal opened from the encoder view and the Dynamics encoder itself. It does not implement UI behavior, programmer mutations, runtime sampling, persistence, command grammar, or tests.

## Goal

Add a focused Dynamics workflow inside the Programmer encoder view.

When the operator opens Dynamics from the encoder view, ToskLight shows a modal for building Dynamic content against the current selected attributes. This is the second implementation chunk because it depends on the operator-facing object vocabulary from the Dynamics window but can still be built before final runtime output if guarded by fake or preview-only state.

## Modal Behavior

With a current selection, the modal opens pre-targeted to the current ordered selection and the attributes supported by that selection. Without a selection, it creates reusable Dynamic content that can later be applied to targets.

The modal must support:

- choosing the attribute lane to create or edit;
- choosing the lane value mode;
- choosing a shape or keyframed behavior;
- setting speed, size, phase, blocks, repeats, wings, and spread where supported;
- previewing the target scope and validation state;
- keeping the Dynamic active in the Programmer when runtime semantics exist;
- storing the Dynamic content into a Preset or Cue when those semantics exist; and
- saving it as a dedicated Dynamic object.

The modal is not a replacement for the full Dynamics window. It is a fast programming workflow tied to the current encoder context.

## Dynamics Encoder

The Dynamics encoder is the encoder surface inside the modal. It exposes Dynamic-specific parameters as encoder-controlled values rather than generic form-only controls.

It should cover:

- lane parameter selection;
- shape or mode selection;
- speed and multiplier;
- size or amplitude;
- phase offset and span;
- block size;
- repeats;
- wings;
- current/static value source; and
- explicit spread expression entry where applicable.

The encoder must follow [Programmer Relative Encoders, Touch Controls, and Fade-Time Scope](../../Next/00-programmer-relative-encoders-and-fade-time-scope.md). Relative movement remains relative, Set Value remains explicit, and Programmer Fade must not accidentally attach to encoder-originated Dynamic parameter changes unless a later runtime rule explicitly says so.

## Acceptance Coverage

1. The Programmer encoder view can open the Dynamics modal.
2. With a selection, the modal is pre-targeted to the current ordered selection and supported attributes.
3. Without a selection, the modal creates reusable Dynamic content without mutating unrelated Programmer state.
4. The Dynamics encoder exposes the documented Dynamic parameters.
5. Encoder movement follows the shared relative encoder semantics.
6. The center Set Value path remains the explicit absolute-entry path for Dynamic parameters.
7. The modal can save to a Dynamic object and, once runtime semantics are implementable, keep content active in the Programmer or store it into Presets and Cues.
