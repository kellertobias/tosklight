# Fixture Mounting and Multipatch Positioning

## Status

**Specification only.** This feature is planned for later. It records show-patch and paperwork behavior only and does not add executable tests or runtime implementation.

## Operator intent

Fixtures in the show patch need a way to move together when one physical object is mounted to another. A truss, pipe, scenic element, floor stand, dolly, or parent fixture can carry other fixtures so the operator edits the parent position once and the mounted fixtures follow.

This is a patch, Stage setup, visualization, and paperwork feature. It must not change show execution, programmer values, cue playback, fixture selection identity, DMX output, or attribute resolution.

## Multipatch default behavior

A multipatch instance is mounted to its primary patched fixture by default.

When the operator moves only the primary fixture in the show patch or Stage position editor, all of its multipatch instances move by the same transform so their relative offsets remain unchanged.

When the operator selects the primary fixture together with one or more of its multipatch instances and performs a move, spread, distribute, align, rotate, or other position operation, each selected item is edited as its own selected item. The multipatch instances must not receive the parent's propagated movement a second time.

The operator may unmount a multipatch instance when it needs an independent position. An unmounted multipatch instance keeps its current absolute position and no longer follows the primary fixture until it is mounted again.

## General fixture mounting

Any fixture-like show-patch item that has a Stage position may be mounted to another fixture-like item. For example, a moving light can be mounted to a truss fixture. When the truss moves, rotates, or is repositioned as part of show setup, the mounted moving light follows while preserving its relative position and orientation.

Mounting records a parent-child relationship plus the child's relative transform from the parent. The child still keeps its normal fixture identity, number, label, profile, patch address, groups, presets, cues, and programming behavior. Only its physical position in the show setup is derived from the mount.

Mounted children may themselves have mounted children. The resulting hierarchy must resolve from the topmost parent down to all descendants.

## Position editing rules

Moving, rotating, aligning, or otherwise transforming an unselected parent applies the same effective parent transform to all mounted descendants that are not independently selected for the same operation.

If a parent and child are both selected for the same position operation, the explicit operation wins for both selected items. The child is not additionally moved through parent propagation during that operation.

If only a mounted child is selected, the operator edits the child's relative transform under its current parent. Its absolute Stage position changes, but the parent does not move.

If several mounted children are selected without their parent, spread and distribution operations operate on those children in their visible absolute Stage positions and then store the resulting relative transforms back under their parent relationships.

Unmounting a child keeps its currently resolved absolute transform. Remounting a child to the same or a different parent keeps the child's current absolute transform and stores a new relative transform unless the operator explicitly chooses a different mount point or offset.

## Patch and Stage UI expectations

The Patch and Stage position workflows should expose mounting as a normal setup action. The operator must be able to:

- see whether a fixture or multipatch instance is mounted;
- see its parent fixture;
- mount selected fixtures to a chosen parent;
- unmount selected mounted fixtures;
- move mounted children between parents;
- edit a mounted child's offset relative to its parent; and
- identify descendants that will follow before applying a parent move.

The UI should use the term **Mount** rather than Attach.

Stage and patch views should make the relationship visible enough that an operator understands why a fixture moves when a truss or parent fixture is moved. The first version does not need a full rigging-tree editor, but it must avoid silent or surprising movement.

## Paperwork and reporting direction

Mount relationships are portable show data because they describe the physical rig. Future paperwork, plots, exports, and print workflows should be able to group or print whole mounted structures, such as a complete truss with every lamp mounted on it.

Reports should be able to show both the absolute resolved position of each fixture and the parent relationship that explains how that position was derived. A truss paperwork view may later list all mounted descendants in rig order.

## Persistence and compatibility

Mount relationships belong to the show patch and travel with the show file. They are not desk-local layout state.

Old shows without mount data load with no explicit mounts except for multipatch instances, which receive the default mount-to-primary behavior unless legacy data already represents an independent absolute multipatch position that must be preserved.

The implementation must reject cycles, missing parents, self-mounting, and relationships that would make position resolution ambiguous. If a parent fixture is deleted, imported without its child, or otherwise unavailable, mounted children must fail visibly and keep a safe resolved or last-known absolute position rather than disappearing or moving to the origin.

Show revision, undo, save/reload, selective import, MVR import/export, and future paperwork export must preserve mount relationships and relative transforms.

## Required implementation contract and tests

Future implementation must cover at least:

1. Multipatch instances mounting to the primary fixture by default.
2. Moving only the primary fixture moves mounted multipatch instances by the same transform.
3. Moving or spreading the primary fixture and selected multipatch instances edits the selected items once and does not double-apply parent movement.
4. Mounting ordinary fixtures to parent fixtures such as trusses, pipes, scenic objects, dollies, or other fixture-like patch items.
5. Parent move, rotate, align, and position-edit behavior propagating to unselected descendants.
6. Child-only edits updating the child's relative transform while leaving the parent in place.
7. Selection, spread, distribute, and align operations across mounted children with and without their parent selected.
8. Nested mount hierarchy resolution, stable order, cycle rejection, self-mount rejection, and missing-parent recovery.
9. Mount, unmount, remount, and move-to-another-parent workflows in Patch and Stage setup.
10. Save/reload, show switching, undo/revision handling, old-show migration, selective import, and MVR import/export compatibility.
11. Paperwork/export data exposing parent relationships, relative transforms, and resolved absolute positions.
12. Proof that show execution, programmer values, cue playback, DMX output, groups, presets, fixture identity, and unpatched-fixture semantics are unchanged by mounting.

## Deferred work

This plan does not define detailed rigging hardware, load calculations, cable planning, collision detection, full paperwork templates, or a complete truss-print layout. Those can build on the mount hierarchy later without changing the core patch-position contract.
