# 04 — Multi-fixture stage-position move: one server request, not a layout overwrite

## Context (api-rules §4 + §3 violation, verified 2026-07-23)

`apps/control-ui/src/components/control/StageCommandControls.tsx:35-41` computes a
per-fixture delta relative to the anchor fixture (`first`, `:34`) across the whole
selection (`:39`) and then saves the **entire** stage layout:
`saveStageLayout({ version: 2, positions, positions3d })` (`:40`, also from the encoder
handler at `:56`). The save path is
`features/stageLayout/StageLayoutActionsProvider.tsx:47` → `putStageLayout` →
`api/ServerDeskBoundaries.tsx:38` → `client.putObject(showId, "stage_layout", "main", …)` —
a whole-object PUT of every fixture's position, violating both §4 (UI computes per-fixture
show values) and §3 (whole-object overwrite instead of intent).

## Work

1. Add a server-side intent route (api-rules §3 style), e.g.
   `POST /api/v2/…/stage-layout/actions` with a typed body carrying the ordered selection,
   the axis/key, and the delta (or absolute anchor target); the server computes each
   fixture's new position and persists only those entries. **Use chunk 03's shared
   fan-out vocabulary** (selection + operation payload) — this is the same family
   (uniform delta instead of interpolation); do not invent a parallel request shape. Carry a client `request_id`
   and add a replay window (reuse the existing `ReplayCache` pattern, e.g.
   `crates/application/src/show_patch/replay.rs`).
2. Wire types in `crates/wire` (tolerant typing per §5), regenerate contracts.
3. Migrate `StageCommandControls.tsx` (both the numeric `update` and the encoder handler)
   to the new request; drop the client-side delta loop.
4. Leave single-fixture drag-in-stage-view saves as they are unless they share the same
   whole-layout PUT — if they do, note it in the result and file a follow-up chunk rather
   than expanding scope.

## Definition of done

- Multi-fixture stage move is one HTTP/WS request carrying selection + delta; the UI no
  longer iterates the selection to compute positions.
- Server test covering delta application over an ordered selection incl. fixtures missing
  a 3D position (current client skips them — preserve that semantic, `:39` guards
  `if (nextPositions[id])`).

## Verification

```sh
cargo test -p server -p application
npm run test:unit
npm run test:e2e -- tests/<stage spec>   # locate the stage-view spec covering moves
npm run test:e2e                          # full suite gate
```

Manual: `npm run open`, select several fixtures, nudge X via StageCommandControls, confirm
all move by the same delta and unrelated fixtures' stored positions are untouched.

## Decisions

None — route shape follows api-rules §1/§3 (stored-config edit → object-intent update).
Chunk 16 (generic object `/update` intent) is related but not a prerequisite; this route is
a purpose-built spread intent like the programming-update routes.
