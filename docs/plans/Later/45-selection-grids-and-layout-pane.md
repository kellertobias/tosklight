# Selection Grids and Layout Pane

## Status

**Specification only.** This feature is planned for later. It does not yet describe implemented behavior, and this plan does not add executable tests.

## Operator intent

Every fixture selection and every Group has a spatial grid in addition to its ordinary ordered fixture sequence. The grid provides a stable spatial representation for viewing and selecting fixtures now and will later provide the spatial foundation for Dynamics.

The ordered sequence and the grid remain separate concepts. Building or changing a grid does not silently change the sequence. The operator explicitly chooses when to derive a new sequence from the current grid.

## Grid sources

Every selection and Group has a grid. The default method is **2D Stage**.

- Selecting one Group uses that Group's grid method and current grid.
- Selecting several Groups re-grids the combined selection with their common method when every Group uses the same complete method configuration.
- If the selected Groups use different methods or different axis origins, the combined selection uses 2D Stage.
- Adding individual fixtures to a selection retains the selection's current method and recalculates its grid.

Groups store their ordered fixture membership and grid configuration. Their current cells follow the fixtures' Stage positions. Moving a fixture on Stage therefore changes its Group-grid cell, but does not rewrite the Group's stored linear sequence.

Legacy Groups without grid data load with a 2D Stage grid while retaining their existing ordered membership. Intentionally stored empty Groups remain distinct from absent or deleted Groups and have an empty grid.

## Grid construction methods

Manual Rows or Columns packing is not part of this feature. Rows-first and Columns-first are used only when converting an existing grid back into a linear selection order.

The available Stage-derived grid methods, in cycling order, are:

1. **2D Stage**
2. **Top to Bottom**
3. **Bottom to Top**
4. **Front to Back**
5. **Back to Front**
6. **Left to Right**
7. **Right to Left**
8. **Horizontal axis (X)**
9. **Vertical axis (Z)**
10. **Room-depth axis (Y)**

The coordinate convention follows the existing Stage model:

- X is horizontal, from left to right;
- Z is vertical, from bottom to top; and
- Y is depth into the room or Stage.

The six directional 3D methods project the selected fixtures orthographically onto a 2D plane from the named viewing direction.

The three axis methods use a cylindrical unwrap around the selected axis. Horizontal (X) keeps X as the natural horizontal dimension, Vertical (Z) keeps Z as the natural vertical dimension, and Room-depth (Y) keeps Y as the depth dimension. The other grid dimension follows the angular sweep around the axis. Each axis uses an editable XYZ origin that defaults to the Stage origin at `0,0,0`.

Projected coordinates are converted into deterministic row and column ranks. Each fixture occupies its own cell. Fixtures that share an exact projected position receive stable additional ranks rather than being placed in the same cell. Empty cells between ranked positions remain available to represent the spatial layout.

Logical heads follow the ordinary selectable-item rules. A multipatched logical fixture or head remains one selection item rather than appearing once for every physical output instance. Unpatched fixtures remain part of the grid and selection even though they produce no DMX output.

## Changing the grid with Shift and ALL

A short `[SHIFT] [ALL]` applies the next grid method in the ordered list above. Repeated presses continue through the list and wrap from Room-depth axis (Y) to 2D Stage. Changing the method recalculates the grid but leaves the current linear selection sequence unchanged.

Holding `[SHIFT] [ALL]` for 650 ms opens a Grid Settings menu similar to the Record and Update menus. It allows the operator to choose a grid method directly and, for an axis method, edit the XYZ axis origin. The held gesture must not also invoke the short cycle or ordinary ALL.

Software uses its normal latched Shift behavior. Attached hardware uses held Shift. Keyboard, OSC, REST, WebSocket, software, and attached hardware must invoke the same authoritative grid action.

Unshifted ALL retains its existing behavior of restoring the complete remembered Highlight selection. The new shifted gesture must never leak an ordinary ALL action.

## Reordering the selection from the grid

Reordering is explicit and operates on the current grid without changing its projection method.

`[SHIFT] [NEXT]` applies Rows-first ordering and cycles through four traversals:

1. start at top-left, traverse each row left to right, then continue downward;
2. start at top-right, traverse each row right to left, then continue downward;
3. start at bottom-left, traverse each row left to right, then continue upward; and
4. start at bottom-right, traverse each row right to left, then continue upward.

A further `[SHIFT] [NEXT]` wraps to the first Rows-first traversal.

`[SHIFT] [PREV]` applies Columns-first ordering and cycles through four corresponding traversals:

1. start at top-left, traverse each column top to bottom, then continue rightward;
2. start at bottom-left, traverse each column bottom to top, then continue rightward;
3. start at top-right, traverse each column top to bottom, then continue leftward; and
4. start at bottom-right, traverse each column bottom to top, then continue leftward.

A further `[SHIFT] [PREV]` wraps to the first Columns-first traversal.

Each action immediately rewrites the current ordered selection from the non-empty grid cells in that traversal. It does not change Group storage by itself. Recording or updating a Group stores the resulting sequence through the normal Group workflow.

Unshifted PREV and NEXT retain their existing Highlight step-through behavior.

## Group recording and membership changes

Recording or overwriting a Group stores both the current ordered membership and the current grid configuration. Selecting that Group later restores its sequence and resolves its grid against the current Stage positions.

Merge, Subtract, and Group Update retain their existing ordered-membership rules. The spatial grid is recalculated for the resulting membership with that Group's method. These operations do not implicitly apply a Rows-first or Columns-first traversal; the operator uses `[SHIFT] [NEXT]` or `[SHIFT] [PREV]` when a new grid-derived sequence is wanted.

Live and derived Groups re-resolve membership according to their existing rules and then rebuild their grid. Frozen Groups retain their frozen membership but still resolve those fixtures against current Stage positions.

## Layout pane

Add **Layout** as a normal configurable pane and full-window choice. Each Layout pane selects one stored Group in Pane Settings and shows that Group using its current grid.

Each occupied cell shows the fixture identity together with only the resolved live intensity and color. The first version does not show other attributes, edit Stage positions, visualize Preload, or provide Dynamics controls.

The Layout pane is also a fixture-selection surface:

- tapping a fixture starts a normal selection;
- the standard additive/toggle modifier extends or removes from the selection;
- Shift range and marquee selection follow the desk's ordinary spatial-selection behavior; and
- selection changes feed the same authoritative programmer selection used by Stage, Fixture Sheet, Groups, keyboard, OSC, and attached hardware.

If the configured Group is empty, the pane shows an empty Group state. If it was deleted or is unavailable, the pane identifies the missing Group and allows another Group to be selected without silently changing the pane to an unrelated Group.

## Persistence and compatibility

Group grid configuration belongs to portable show data. The current selection grid has the same ownership and lifetime as the authoritative selection. A Layout pane's selected Group belongs to that pane's persisted Desktop configuration.

Old shows must load without losing Group membership or order. Grid fields require backward-compatible defaults, malformed grid configuration must fail safely, and save/reload, show switching, undo/revision handling, and application restart must preserve the documented state.

Stage-position changes and any affected live grid state must be published consistently so connected software, browser, OSC, and hardware surfaces do not disagree about cells or sequence order.

## Required implementation contract and tests

Future implementation must cover at least:

1. Default 2D Stage grids for direct selections, new Groups, legacy Groups, and intentionally empty Groups.
2. Every directional and rotational projection, editable axis origins, coordinate orientation, deterministic ranks, exact-position ties, and method cycling.
3. A single Group reusing its method, several matching Groups using their common method, and mixed configurations falling back to 2D Stage.
4. Stage movement updating Group cells without silently changing the stored sequence.
5. All four Rows-first and all four Columns-first orders, cycling and wrapping, holes in sparse grids, and stable fixture identity.
6. Short and 650 ms held `[SHIFT] [ALL]` gestures remaining mutually exclusive and never invoking ordinary ALL.
7. Shifted and unshifted ALL, PREV, and NEXT remaining distinct across software, keyboard, REST, WebSocket, OSC, and attached hardware.
8. Group Record, overwrite, Merge, Subtract, Update, derived/frozen behavior, revision protection, save/reload, and legacy-show migration.
9. Unpatched fixtures, logical heads, multipatch identity, missing fixtures, empty selections, and single-fixture selections.
10. Layout pane Group selection, intensity/color rendering, live value updates, fixture click/range/marquee selection, empty Groups, missing Groups, pane persistence, and multiple Layout panes showing different Groups.

## Deferred work

This feature deliberately does not include Dynamics or effects, manual Rows/Columns grid construction, editable grid cells, point projection, or arbitrary projection cameras. Dynamics may later consume these grids to move effects across spatially ordered fixtures without changing this feature's selection and Group contracts.
