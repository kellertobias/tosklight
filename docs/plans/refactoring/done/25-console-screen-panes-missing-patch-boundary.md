# 25 — Console-screen panes have no Patch authority

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

## Result

Completed on 2026-07-24.

- Confirmed the ownership mismatch: the main app and dedicated Stage window mounted
  `PatchFeatureBoundary`, while `ScreenApp` mounted pane content directly under
  `AppProvider`.
- Wrapped the complete console-screen surface in the existing lazy Patch boundary.
  This gives Stage, Fixture Sheet, Channels, and DMX panes the same shared patched-
  fixture authority without widening activation into every `ServerRuntime` consumer.
- Added a focused structural regression test proving the screen surface and its
  connection/loading overlays are inside Patch authority while the desk lock remains
  outside it. No screen-window E2E existed to extend.
- Focused test, TypeScript, architecture/source-size checks, and `npm run test:unit`
  passed; the unit gate now reports 282 frontend files and 1,998 frontend tests.
- `npm run test:e2e` passed cleanly with 287 passed / 9 skipped, including the product
  demo and the previously intermittent `COLOR-RANGE-001 @ui` case.
- `npm run open` rebuilt both Tauri applications, launched the current ToskLight
  bundle, and passed its canonical server ownership/readiness gate.
