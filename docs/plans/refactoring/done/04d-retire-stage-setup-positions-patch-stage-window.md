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
