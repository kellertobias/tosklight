# 16 — Show-object writes become intent updates; retire whole-object PUT

## Context (api-rules §3 violation + v1 retirement, verified 2026-07-23)

The generic object surface is the biggest remaining v1 block
(`http_router.rs:161-177`):

- `GET /api/v1/shows/{id}/objects/{kind}` (list) and
  `GET/PUT/DELETE /api/v1/shows/{id}/objects/{kind}/{object_id}` (`:163-166`).
- `PUT` maps to `put_object` (`runtime/object_api.rs:233-282+`): opaque
  `Json<serde_json::Value>` **whole-object replace**, guarded only by `If-Match`
  (`parse_if_match`, `object_api.rs:240`) — no request identity, no field-partial intent.
- Also here: `POST …/objects/{kind}/{object_id}/undo` (`:169`),
  `POST …/presets/{preset_id}/store` (`:173`, tests only),
  `POST …/preload/store` (`:176`).

Client callers:

- `api/client/showObjects.ts:38-50` `putObject` + `api/ShowObjectSnapshotTransport.ts`
  (reads). Writers via `LightApiClient.putObject` (`LightApiClient.ts:170`):
  `features/server/output.ts:30`, `layouts.ts:22`, `preload.ts:74`, `patch.ts:15`,
  `api/ServerDeskBoundaries.tsx:38` (stage layout — see chunk 04).
- Bench + root tests read/write objects directly.

Note: the CUE-011 fix (chunk 02) and the normalize-once write-back live on this same
mutation path (`crates/application/src/active_show/objects.rs`) — land 02 first.

## Work

1. Design `POST /api/v2/objects/{kind}/{id}/update` (loaded show implied; optional show
   guard header from chunk 12): typed partial body per kind, request id + replay window
   (reuse `ReplayCache`), returns the new revision + emits `show_object_changed`.
   Also `…/delete`. Reads: v2 snapshot routes for list/get (align with
   `ShowObjectSnapshotTransport`'s needs; it already consumes v2 events for invalidation).
2. Apply api-rules **§7 (undo and concurrency)** to this surface:
   - **Undo is a desk-scoped programmer action**, not a generic object operation. The
     generic `POST …/objects/{kind}/{object_id}/undo` route does not survive as a public
     v2 route; the `object_history` mechanism may remain as the *internal* backing for a
     desk's programmer undo of a recording (undoing a stored cue/preset from the desk
     that recorded it). Route the desk's undo through the programmer undo action.
   - **Revision conflicts are stale-client protection, not user locks** (last-write-wins
     between users is by design). On a 409/conflict the client re-reads and reapplies the
     intent instead of surfacing a blocking error for deliberate operator actions.
   - **Creating writes are server-assigned**: storing a cue never sends a cue number the
     client computed from a stale list — the server assigns the next number/slot, so two
     desks storing onto the same cuelist yield two cues.
3. Convert writers one at a time (window settings, desk settings, layouts, output routes,
   preload store) — each becomes a typed partial update; delete each `putObject` call as
   it migrates. The `deskSnapshot`/`showObjects` scoped stores keep their revision
   bookkeeping (`installAuthoritativeObjects` semantics unchanged).
4. Migrate bench/tests (they seed objects — give the bench a v2 seeding path).
5. Delete the v1 object routes when callers reach zero; `presets/store` moves to the v2
   recording surface (`preset_recording_routes.rs`) or a test-support route.
6. Persisted-data caution: this touches every stored object kind — re-read
   `docs/acceptance-criteria.md`; old-show compatibility must stay (normalization-on-open
   behavior is pinned by SHOW-004).

## Definition of done

- No desk-UI whole-object PUT remains; every stored-config edit is a typed partial intent
  with request identity; v1 object routes deleted; bench/tests migrated.
- Undo reaches the api-rules §7 shape: desk-scoped programmer undo (with recording
  rollback), no public generic object-undo route; the `active_object_undo` unit suite is
  updated to the new shape and green.

## Verification

```sh
cargo test -p server -p application
npm run test:unit
npm run test:e2e-api
npm run test:e2e   # full suite gate — object-heavy scenarios (groups, cues, windows)
```

## Decisions

**This chunk is large — split it at execution time** into (a) route + one writer,
(b) remaining writers, (c) v1 deletion, as `16a/16b/16c` files in `pending/`. No open
maintainer decision; typed-partial per kind follows the decided intent style.
