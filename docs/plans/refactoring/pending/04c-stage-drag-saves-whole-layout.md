# 04c — Stage-view drag/inspector saves still overwrite the whole stage layout

## Context (filed from chunk 04, 2026-07-23)

Chunk 04 moved the multi-fixture StageCommandControls paths onto the v2
`POST /api/v2/stage-layout/actions` intent route (`move_selection`), but the remaining
stage-layout writers still build and PUT the **entire** `stage_layout/main` object
(api-rules §3 violation — whole-object overwrite instead of intent):

- `apps/control-ui/src/windows/stageWindow/useStageLayout.ts:49` `save()` — used by the 2D
  drag end (`useStage2dGestures.ts:79`) and the 3D inspector blur (`Stage3dView.tsx:40`).
- `useStageLayout.ts:59` `savePosition3d()` — 3D drag end (`Stage3dView.tsx:106`), merges a
  single fixture into `positions3d` and saves the whole layout.

These are single-fixture *absolute* placements, not fan-outs, so the right shape is a second
`StageLayoutAction` variant (e.g. `set_position { fixture_id, position }`) on the existing
route/wire enum from chunk 04 (`crates/wire/src/v2/stage_layout.rs`,
`crates/server/src/runtime/stage_layout_http.rs`) — same replay cache, same
`show_object_changed` emission. `camera3d` persistence (also saved through the whole-layout
PUT) needs a decision: piggyback a `set_camera` intent or accept the whole-object PUT for
camera-only state.

## Work

1. Add the absolute-placement action variant(s) to the chunk 04 wire enum + handler, with
   route tests following `crates/server/src/runtime/tests/stage_layout_route_tests.rs`.
2. Migrate `useStageLayout.save`/`savePosition3d` call sites; keep interaction/display logic
   client-side.
3. Retire `StageLayoutActions.saveStageLayout`/`putStageLayout` if no caller remains (check
   camera3d first).

## Definition of done

- No stage-view interaction path issues a whole-layout PUT (or the remaining PUT is
  camera-only and documented as such).
- Server tests cover the absolute placement incl. creating a fixture's first position entry.

## Verification

```sh
cargo test -p light-server stage_layout
npm run test:unit
npm run test:e2e
```
