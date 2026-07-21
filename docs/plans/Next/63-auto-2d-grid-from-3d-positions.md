# Auto 2D Grid From 3D Positions

## Status

**Specification only.** This plan records a future Patch and Stage layout feature. It does not implement grid generation, UI placement, persistence, paperwork output, or executable tests.

## Goal

Auto-generate the 2D fixture grid from the show's 3D fixture positions, with operator control over how the rig is unwrapped into a 2D working view.

If the operator has not manually edited the 2D grid, the 2D grid should be derived automatically from the 3D positions. Once the operator edits the 2D grid manually, the app must preserve that intent and avoid silently overwriting it.

## Operator workflow

The feature may live in Patch, Stage, or both. The command should ask how the operator wants to unwrap the 3D positions into the 2D grid. Candidate unwrap modes include front, top, side, truss/line projection, venue zone, or another explicitly named projection.

The generated grid should support desk programming and future paperwork use. It should be compatible with Stage selection, fixture layout, Dynamics grid ordering, and the dedicated renderer/paperwork app.

## Acceptance coverage

1. A show with 3D fixture positions but no manual 2D edits receives a deterministic auto-generated 2D grid.
2. Manual 2D grid edits prevent automatic regeneration from overwriting operator layout.
3. The operator can intentionally regenerate the grid and choose an unwrap mode.
4. Patch and Stage views agree on the generated positions when both expose the feature.
5. Generated 2D positions remain suitable for selection, programming, Dynamics ordering, and paperwork.
