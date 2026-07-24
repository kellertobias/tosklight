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

## Result

**What changed.** New v2 intent route `POST /api/v2/stage-layout/actions` (wire types in
`crates/wire/src/v2/stage_layout.rs`, handler `crates/server/src/runtime/stage_layout_http.rs`)
using chunk 03's fan-out vocabulary: ordered `fixture_ids` + typed operation
(`move_selection { axis, delta }`) + `request_id` with a session-scoped replay cache
(fingerprint compare, 409 on request-id reuse for a different action). The server resolves each
selected fixture's base position exactly like the stage views (stored `positions3d` entry →
migrated legacy 2D entry → patch-order default grid slot; unpatched ids skipped), applies the
uniform delta, and persists **only the touched entries** of `stage_layout/main` through the same
normalize/validate/backup/put/activate/emit path as the generic object PUT, so revisions and
`show_object_changed` reconciliation behave identically. `StageCommandControls` (numeric fader
and hardware-encoder paths share `update`) now sends one request; the per-fixture delta loop and
whole-layout save are gone. Tolerant typing per api-rules §5 (no `deny_unknown_fields`;
unknown-field logging deferred to chunk 08's helper).

**Suite numbers.** `cargo test -p light-server` 424 passed (4 new route tests) /
`-p light-application` 391 passed; `npm run test:unit` green (tsc + 1988 vitest incl. 3 new
client suites); full `npm run test:e2e` **281 passed / 12 skipped / 0 failed** — exactly at the
README baseline. Manual acceptance done against the real desktop app (`npm run open` + served
operator UI): selecting the 8-fixture Front group in the Demo Show and nudging X persisted
exactly 8 entries, all moved by the identical delta, display refreshed via the change event.

**Surprises.**
- The chunk's DoD reading of the `:39` guard ("skips fixtures missing a 3D position") was wrong
  in practice: the client backfills every *patched* fixture (legacy-2D migration or default
  grid), so a skip-only server made the first nudge on a show without a `stage_layout` object
  (the shipped Demo Show!) a silent no-op. The server now mirrors the full backfill; the
  default-grid index uses the authoritative patch order (object-id ascending), which every
  surface shares. Net behavior change vs. the old client: unselected fixtures' backfilled
  positions are no longer persisted as a side effect — deliberate, that was the §3 violation.
- Chunk 03b left the checked-in `programming-values-action-request.schema.json` stale after
  restoring `deny_unknown_fields`; regenerating contracts surfaced it (committed as its own
  sync commit).
- `npm run open` was broken on this dev machine independent of this chunk: the Tauri setup hook
  (8s) and `tools/build.sh`'s launchd wait (10s) both killed the bundled server mid-startup,
  since a debug build on grown desk data needs ~14s (persisted-session restore dominates).
  Both waits now match dev.sh's 60s window (`fix(dx)` commit).
- No dedicated stage-move Playwright spec exists (product-demo covers stage rendering only and
  stays deliberately skipped), so e2e coverage of this path is indirect; server + unit tests
  carry the contract.

**Follow-ups filed.**
- `pending/04c-stage-drag-saves-whole-layout.md` — the 2D/3D drag and inspector paths
  (`useStageLayout.save`/`savePosition3d`) still PUT the whole layout; absolute-placement
  intent variant proposed on this route. Note: the maintainer has since asked to remove the
  Stage window's Setup-positions surface and open a dedicated stage view from the Patch
  instead — re-scope 04c against that outcome.
