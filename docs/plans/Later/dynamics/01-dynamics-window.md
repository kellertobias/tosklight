# Dynamics Window

## Status

**Specification only.** This chunk records the future full Dynamics window and object-management UI. It does not implement frontend behavior, backend behavior, persistence, command grammar, or tests.

## Goal

Create the primary Dynamics built-in/window for managing reusable Dynamic objects.

The first implementation chunk is UI-first: it defines how the operator creates, edits, names, copies, moves, deletes, selects, and inspects Dynamic objects. It should not implement final runtime sampling or output contribution unless the runtime chunk has become implementable.

## Window Behavior

The Dynamics window behaves like a ToskLight show-object surface:

- numbered Dynamic objects with stable identities;
- empty-slot behavior compatible with Presets and future Effects;
- create, name, copy, move, delete, and update workflows;
- object tiles that show name, number, active/running state, and validation status;
- an editor for the selected Dynamic;
- clear missing, invalid, or unsupported-lane states; and
- no hidden dependency on the current Programmer selection.

The window should let the operator build reusable Dynamics with or without a current selection. Without a selection, the object remains reusable until applied to targets. With a selection, the window may offer to use that selection as an initial target, but selecting or editing a Dynamic object must not silently mutate Programmer values.

## Editor Shape

The editor presents one or more independent attribute lanes. It may show lane rows, cards, or a compact timeline-like view, but each lane remains one scalar attribute over time. Coincident keyframes may be shown together for convenience, but the data model must not become a combined multi-attribute node.

The first editor should cover:

- Dynamic name and number;
- lane list;
- lane attribute;
- lane value mode;
- speed source and multiplier;
- phase/distribution overview;
- validation messages; and
- save/update actions.

Detailed lane runtime semantics stay in [Runtime and Application Semantics](03-runtime-and-application-semantics.md).

## Acceptance Coverage

1. Dynamics is available as a normal built-in/window when the chunk is marked implementable.
2. The window shows numbered Dynamic object slots, including empty slots.
3. The operator can create, name, copy, move, delete, and select Dynamic objects without starting output accidentally.
4. The editor shows independent attribute lanes and does not present a shared multi-attribute path.
5. Invalid or incomplete Dynamics display actionable validation state.
6. Selecting a Dynamic without a target does not mutate Programmer values.
7. Empty-selection object tapping behaves consistently with Presets and future Effects.
