# Rough venue architecture in Architect

Tracked by [TL-843](https://plans.tokenet.de/TL/843). Requested 20 October 2026.

## Operator outcome

An operator can sketch enough of a venue to place a rig, make useful plans and understand the
lighting in 3D. This is deliberately simple venue modeling, with dimensions and snapping rather
than architectural drafting or construction documentation.

- Walls have length, angle, height and thickness. Their endpoints snap to other wall endpoints.
- Windows belong to a wall: rectangular, round, or flat-bottomed with an arched top.
- Doors belong to a wall: single or double, opening inward or outward, with a plan swing symbol.
- Pillars can be circular, square or rectangular, with width, depth and height.
- Closed wall outlines create ceilings with a configurable height. Independent roofs and balcony
  slabs have polygon outlines, elevation above floor and thickness. Multiple slabs can form stepped
  theatre balconies.
- Tables are circular or rectangular with custom dimensions. Chairs have three simple styles:
  black with metal legs, decorative white, and bar stool. Bar parts are enclosed solid counters
  with adjustable dimensions and height.
- These objects appear in Architect views, exported plans and native 3D rendering.

## Shared representation

Use the existing portable visual-only Venue fixture path. Keep placed dimensions and pose as the
single authority. Add optional typed architecture details to the placement's scenery options;
absence preserves existing objects and their serialization. Store lengths and local outlines in
integer millimetres, as the existing patch placement does.

A wall owns its openings by stable identity and local offset. Moving or rotating the wall moves its
openings; deleting or duplicating the wall handles the complete assembly in one undoable operation.
Do not model a window as a decal over a solid wall: the aperture must also exist in physical shadow
and beam geometry. Polygon slabs must reject self-intersections, zero area and invalid dimensions.
A local planar outline and explicit elevation support independent, stackable balcony objects.

Geometry must travel through show persistence, Architect undo/redo, show-sync transactions, typed
wire projections and the Control renderer. Architect-only metadata is not an alternate geometry
store: it does not reach the Control scene compiler. Retain old show loading without a SQLite
schema bump for these additive optional fields. New semantic kinds require an explicit compatibility
policy before enabling authoring; an old reader must not silently rewrite away authored details.

## Camera-aware cutaway

Resolve the camera before constructing the visible scenery list. An exterior camera hides the
near enclosing walls and ceiling surfaces that prevent viewing the interior. An interior camera
sees the enclosing walls. Base visibility on the wall loop and side of each enclosing surface,
not simply its distance to the camera. Keep independent stacked balconies visible when appropriate.

Separate visible geometry from physical light blockers. A cutaway wall or roof remains in shadow
and beam occlusion, so moving the viewing camera does not change the lighting inside the venue.
Open doors and windows differ: they are genuine openings in both passes. Apply the same policy in
all 3D rendering qualities. CAD and exported plans retain the complete venue drawing.

## Implementation and verification

1. Establish typed placement details, geometric validation and legacy serialization coverage.
2. Carry those details through typed patch transport and show persistence/synchronization. Verify
   save/reopen, legacy objects and undo/redo before exposing authoring.
3. Generate shared geometry for walls/openings/slabs and parametric furniture. Use the same outlines
   for plan projection, picking and exports, with unit/pose tests.
4. Add Architect authoring and Info fields, endpoint snapping and closed-loop ceiling creation.
5. Add renderer cutaway with separate physical occlusion, then verify an interior/exterior camera,
   stacked balconies, all window/door forms and representative furniture.
6. Verify actual native interaction, export a representative plan and take bounded native render
   captures. Leave the renderer off outside testing. Update operator help only for available UI.

The first implementation slice is the shared typed geometry foundation. This does not constitute
completion of the authoring, plan export or renderer acceptance criteria; TL-843 remains open until
those paths are implemented and verified. Human acceptance is separate from source/test completion.
