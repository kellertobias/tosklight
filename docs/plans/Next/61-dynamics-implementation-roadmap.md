# Dynamics Implementation Roadmap

## Status

**Specification only.** This plan captures the next implementation direction for Dynamics. It does not remove the implementation gate in `docs/plans/Later/02-dynamics.md` and does not implement backend behavior, UI behavior, persistence, command grammar, or tests.

## Goal

Turn the existing Dynamics experiment and product notes into an implementation-ready backend, runtime, and programmer workflow.

Dynamics should become flexible animated attribute content that can be active in the Programmer, stored into Presets or Cues, assigned to Playbacks, and managed as reusable show objects.

## Programmer workflow

In the programmer encoder view, the Dynamics button should open a simplified editor for the attributes that exist on the current selection. The operator can build a Dynamic against the current selection and current attributes, keep it active in the Programmer, then store it into:

- a Preset that contains the Dynamic content;
- a Cue that contains the Dynamic content; or
- a dedicated Effect/Dynamic object that can later be assigned independently.

With a selection, the created Dynamic is pre-targeted to that selection. Without a selection, the created Dynamic behaves like a reusable preset-like object that can later be applied to targets.

## Object behavior

- Dynamics need a dedicated pool/object model with empty-slot behavior compatible with empty-selection Preset and Effect target selection.
- A Dynamic can run independently when assigned to a playback.
- Programmer, Preset, Cue, Playback, Preload, Update, release, and show reload behavior must all define whether they reference a Dynamic object or copy its content.
- Selecting two Dynamics and storing a new Dynamic should merge them, but the product decision must settle whether this stores references, copies, or a composed snapshot.

## Acceptance coverage

1. A simplified Dynamics editor opens from the programmer encoder view for the current selected attributes.
2. Dynamics can be active in the Programmer and stored into Presets and Cues.
3. Dynamics can be created with and without a current selection.
4. Assigned Dynamics can run independently from playbacks.
5. Empty-selection object tapping works consistently for Presets, Effects, and Dynamics.
6. Merging two selected Dynamics has a documented reference-versus-copy policy before implementation starts.
7. The existing `Later/02-dynamics.md` implementation gate is resolved before coding begins.
