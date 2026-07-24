# 17 — Screens, control desks, playback pages/slots onto v2; retire their v1 routes

## Context (verified 2026-07-23)

`apps/control-ui/src/api/client/playback.ts` still drives desk-surface configuration over
v1 (`http_router.rs:179-226`):

- Screens: `GET /api/v1/screens` (`playback.ts:103`), `PUT/DELETE /screens/{id}`
  (`:107,115`), `PUT /screens/{id}/page` (`:121`).
- Control desks: `PUT /api/v1/control-desks/{id}/page` (`:227` client), `PUT /control-desks/{id}`
  (`:235` client).
- Playback pages: `PUT/DELETE /api/v1/playback-pages/{page}/slots/{slot}` (`:180,203` client).
- `DELETE /api/v1/clients/{id}` (`playback.ts` — client removal).
- `POST/PUT /api/v1/cuelists/{number}/{action}` (`playback.ts:142`) — live control; should
  already be superseded by chunks 09/11 (v2 playback actions / WS). Verify before touching.

Test-only v1 playback routes (playbacks/{id}/{action}, playback-pool, page-playbacks,
GET cuelists/{number}, GET /playbacks, PUT /master) are used by root tests/OSC scenarios —
migrate the tests to v2 equivalents as the routes fall, or keep them alive until 22.

Operator caution: **current-page vs explicit-page addressing** is a contract
(AGENTS.md) — page-change routes need both cases tested.

## Work

1. v2 intent updates for screens / control-desk settings / page slots (typed partial
   bodies, request ids; follows chunk 16's pattern — land 16 first).
2. Migrate `playback.ts` callers (features/screens actions, LayoutPersistence,
   playback-pages UI) to the new transports; then delete each v1 route with a caller grep
   (bench + root tests included).
3. Cuelist action route: confirm desk UI no longer calls it (after 09/11); leave the HTTP
   action form only if integrators need it under v2 naming (`/api/v2/…/cuelists/{n}/go`
   style GET/POST per api-rules §2).
4. Explicit-page vs current-page playback addressing gets a dedicated test pass.

## Definition of done

- Screens/desk/page-slot configuration flows entirely over v2 intent routes; the listed v1
  routes deleted; tests migrated; page-addressing scenarios green.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e -- tests/<playback pages / screens specs>
npm run test:e2e   # full suite gate — incl. 04-osc, 34-active-playback-colors
```

## Decisions

None. Depends on 16 (intent-update pattern) and 09/11 (live-control WS) landing first.

## Result

Execution-time caller and persistence-boundary inventory showed three independent route
families with different failure modes. The work was split into 17a (desk and screen
configuration), 17b (show playback-slot topology), and 17c (client removal plus live-action
retirement audit) so each typed contract, migration, absence check, and full gate can land
separately. No production behavior changed in this planning split.
