# 26 — Console-screen panes have no Patch authority

## Context (found during chunk 04d, 2026-07-24)

`App` mounts `PatchFeatureBoundary` (`apps/control-ui/src/App.tsx:30`), giving every main-
window surface the shared patched-fixture authority. `ScreenApp`
(`apps/control-ui/src/ScreenApp.tsx`) does not — so panes on a console-screen OS window
that read `usePatchedFixturesView` (stage pane, fixture sheet pane, channels pane, DMX
pane) render with no fixtures. The dedicated Stage View window (chunk 04d) hit the same
gap and now mounts the boundary itself; console screens still lack it.

## Work

1. Reproduce on a console screen (Screens setup → open a screen with a stage or fixture
   sheet pane) and confirm the empty render.
2. Mount `PatchFeatureBoundary` in `ScreenApp` (mirror `StageViewApp`), or lift it into
   `ServerProvider` if every surface wants it — check the lazy-activation comment in
   `PatchFeatureBoundary.tsx` before widening.
3. Regression coverage in the screen-window E2E if one exists.

## Verification

```sh
npm run test:unit
npm run test:e2e
npm run open   # open a console screen with a stage pane; fixtures must render
```
