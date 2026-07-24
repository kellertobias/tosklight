# 04d — Retire Stage Setup-positions; dedicated stage view from the Patch

## Context (maintainer request, 2026-07-24)

The Stage window's Setup-positions surface (mode tab, six position faders, the "Fixture
position" inspector popover, hardware-encoder position editing) duplicates positioning that
belongs to the Show Patch (physical placement, plus pending 04b patch-table columns). The
maintainer asked to remove it and instead grow the Patch's stage preview.

## Decisions (maintainer, 2026-07-24)

1. **Remove the whole Setup-positions mode.** The Stage built-in keeps Select fixtures and
   Navigate only. The popover, faders, 2D/3D drag-positioning, and the encoder positions
   path all go. Positions are set up in the Patch.
2. **Short-press "Preview Stage"** keeps toggling the in-patch stage overlay, but the
   overlay becomes **movable**.
3. **Long-press "Preview Stage"** opens a **dedicated separate OS window** (Tauri, like
   console screens) with the stage view.
4. That OS window is **3D-only and view-only**: no position editing, but clicking fixtures
   selects them (shared desk programming selection), which highlights/selects the matching
   rows in the patch sheet.

## Work

1. Remove `"setup"` from the stage-mode state machine (types, initial state, reducers,
   persisted-layout migration `setup → select`), the StageHeader tab, Stage3dView inspector +
   drag handlers, Stage2dView/useStage2dGestures setup branches, and the
   StageCommandControls positions surface (Navigate surface stays).
2. Delete the now caller-less stage-layout client write path (useStageLayout write half,
   StageLayoutActionsProvider, ServerDeskBoundaries wiring, stageLayout api client + aliases)
   — the read path (store + event reconciliation) stays for display. The v2
   `stage-layout/actions` server route and its tests stay (04b/04c re-scoped consumers).
3. Make the in-patch stage overlay movable (pointer drag, touch-friendly, clamped).
4. Long-press on "Preview Stage" (existing inline pointer-timer idiom) opens a dedicated
   Tauri window (`index.html?stage-view=1`-style entry like console screens,
   `sessionRole="secondary"`), rendering a 3D view-only StageWindow with selection gestures
   enabled. Desktop-only; browser keeps short-press behavior only.
5. Update operator docs (quickstart, 20-Show-Setup/04, 05-Pane-Reference/01) and the two
   e2e specs that click "Setup positions" (tests/19, tests/02-help-screenshots); keep
   scenery (Venue 0.x) positioning documented via Show Patch placement.

## Verification

```sh
npm run test:unit
cargo test -p light-server stage_layout
npm run test:e2e
npm run open   # patch: short-press toggles movable overlay; long-press opens OS window;
               # selection in the OS window highlights patch rows; Stage built-in has no
               # Setup positions tab
```

## Result

**What changed.** The Stage built-in keeps Select fixtures and Navigate; the whole
Setup-positions surface is gone (mode tab, six position faders, "Fixture position"
inspector, 2D/3D drag-positioning, hardware-encoder positions path). Persisted layouts
carrying `stageMode: "setup"` hydrate to `select`. The caller-less stage-layout client
write path (StageLayoutActionsProvider, useStageLayout write half, stageLayout api
client, whole-layout PUT wiring) was deleted; the read path and the chunk 04 v2 server
route + tests stay for 04b/04c-scoped consumers. "Preview Stage" gained both requested
paths: short press toggles the (now movable, grip-dragged) in-patch overlay; long press
(desktop app; `WindowAction.onLongPress` is now a window-kit primitive) opens a dedicated
view-only 3D Stage View OS window (`open_stage_view_window` Tauri command,
`?stage-view=1` SPA entry, `sessionRole="secondary"`). Clicking fixtures there applies
the shared desk programming selection, so the patch sheet highlights the same rows.
Operator docs (quickstart, 20-Show-Setup/01+04, 05-Pane-Reference/01) rewritten around
patch-based positioning; help screenshots intentionally refreshed (`stage-setup-2d.png`
→ `stage-window-2d.png`).

**Suite numbers.** `npm run test:unit` 275 files / 1982 tests green (tsc, architecture,
source-size, boundaries incl. the shared-control audit); full `npm run test:e2e`
**281 passed / 12 skipped / 0 failed** — at baseline; `npm run manual` verifies (130-page
PDF, all images resolve); help-screenshot spec green after repairs. Browser verification
against the real desk: overlay drag moves by the exact pointer delta; the `?stage-view=1`
surface renders the full rig and its clicks mutate the shared programming selection.

**Surprises.**
- The on-demand help-screenshot spec had been stale since the maintainer's 2026-07-17
  ModalTitleBar/File-Manager rework (`.modal-close` selectors, a Settings-less pane);
  repaired in the same commit as the refresh.
- `ScreenApp` (console screens) never mounts `PatchFeatureBoundary`, so its
  fixture-reading panes render empty — same gap the new Stage View window hit. Filed as
  `pending/26-console-screen-panes-missing-patch-boundary.md`.
- A browser tab restoring a stale primary session loops on 401s in secondary surfaces
  (desk-lock poll) without a visible error — chunk 22/24 territory, noted here only.

**Follow-ups filed.** `pending/26-…` above; `pending/04c` was already re-scoped in its
own file (drag paths no longer exist — its remaining scope is the absolute-placement
intent for patch consumers and camera persistence).
