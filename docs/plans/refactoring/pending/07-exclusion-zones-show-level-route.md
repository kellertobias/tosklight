# 07 — Virtual-playback exclusion zones: drop the desk segment, retire the v1 pair

**What exclusion zones are (maintainer, 2026-07-23):** within one exclusion zone, only
one playback can be active at a time. They are show-level configuration that barely ever
changes.

**Transport (maintainer, 2026-07-23):** plain REST snapshot loaded when the window
opens, plus a change event so open windows reload when a zone is edited. No polling, no
desk-lifetime store, no telemetry lane — this is the low-frequency end of the api-rules
read model (snapshot read + invalidation event). If no v2 event exists for zone changes
yet, add one (wire type in `crates/wire/src/v2/events.rs`, published from the zone save
path) rather than keeping any other refresh mechanism.

## Context (api-rules §6, decided 2026-07-23; verified against code)

Storage is already show-level: `crates/server/src/runtime/playback_api.rs:109-122` —
setting key `virtual_playback_exclusion_zones:{show_id}` (`virtual_playback_exclusion_setting`,
`:109-111`); `read_virtual_playback_exclusion_store` (`:113-122`) and
`write_virtual_playback_exclusion_surface` (`:163-185`) both key by show only (desk id is a
nested map key inside the blob).

Two parallel route families serve it:

- **v2 (desk segment = the authentication artifact to drop):**
  `crates/server/src/runtime/virtual_playback_zones_http.rs:22-32` —
  `GET/PUT /api/v2/shows/{show_id}/desks/{desk_id}/virtual-playback-exclusion-zones[/{surface_id}]`
  (snapshot `:49`, filter to `session.desk.id` at `:58`). Merged at `http_router.rs:14`.
- **v1 (still called by the facade client):** `http_router.rs:214-221` —
  `GET /api/v1/virtual-playback-exclusion-zones`, `PUT …/{surface_id}` (handlers
  `playback_api.rs:187,207`), called from `apps/control-ui/src/api/client/playback.ts:150,161`.

## Work

1. Re-route v2 to `…/virtual-playback-exclusion-zones[/{surface_id}]` **without** the desk
   segment (show scoping handled per chunk 12's optional header — if 12 hasn't landed,
   keep `{show_id}` for now and note it; do not invent a third scheme). Desk identity comes
   from the session, as the v1 handlers already do.
2. Decide per-desk filtering semantics explicitly: the store is show-level but the current
   handlers filter to the calling desk. Exclusion zones are show-level per the decision, so
   the snapshot should return the show-level store; keep any desk filtering only if the UI
   depends on it (check `apps/control-ui/src/features/…` consumers and
   `VirtualPlaybackZones` provider) — if it does, filter client-side (display concern).
3. Migrate the client v1 calls (`playback.ts:150,161`) to the v2 route, following the
   transport pattern above: fetch on window mount, reload on the zone-change event.
4. Delete the v1 route pair + handlers; grep bench/tests for the v1 paths and migrate them.
5. While touching the zone save path, verify the enforcement semantic is server-side and
   tested: activating a playback inside a zone releases/blocks the zone's other active
   playback (one active playback per zone). If enforcement currently lives client-side
   anywhere, that's a §4 violation — move it in this chunk and note it in the result.

## Definition of done

- One route family, no `{desk_id}` segment, v1 pair deleted, client and tests migrated.
- Zone create/edit still round-trips in the UI.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e   # full suite gate; exclusion-zone scenarios specifically
```

## Decisions

None new — the desk-segment drop is already decided. Flag in the result note if step 2
reveals the UI depends on server-side desk filtering.
