# 13 — Retire the v1 events WebSocket

## Context (verified 2026-07-23)

Two event sockets coexist:

- **v1:** `GET /api/v1/events` (`runtime/http_router.rs:249`), opened by the facade client
  at `apps/control-ui/src/api/client/runtime.ts:184`; also used by bench and the
  e2e operator-output helpers.
- **v2:** `/api/v2/events` (`runtime/event_transport.rs`), opened at `runtime.ts:94-106`,
  carrying the request_id `command()` channel and all v2 event classes incl. telemetry.

## Work

1. Inventory what still *consumes* v1 event payloads: `ServerProvider` internals
   (`features/server/useServerState.ts`, `useServerPolling.ts`, `connectionBootstrap.ts`)
   — map each consumed v1 event type to its v2 equivalent (or to a snapshot re-read).
2. Migrate those consumers to the v2 event classes; where a v1 event has no v2 equivalent,
   add the v2 event (wire types in `crates/wire/src/v2/events.rs`, regenerate contracts)
   rather than keeping v1 alive.
3. Migrate bench (`apps/control-ui/e2e/bench`) and the operator-output e2e helpers off
   `/api/v1/events`.
4. Delete the v1 socket route + server-side publisher plumbing that only fed it.
5. Watch for the three silent-no-op patterns (scope guard, stale If-Match, v1 mutation
   without v2 event) — this chunk is exactly where pattern 3 bites: any v1-only event that
   something still relied on will surface as a silent staleness bug.

## Definition of done

- `/api/v1/events` no longer registered; no client/bench/test opens it.
- All previously v1-event-driven UI updates still arrive (spot-check: patch changes,
  show open/close, configuration changes propagate live).

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e   # full suite gate — this one is regression-prone; run twice if flaky
```

Manual: `npm run open`, edit patch + open another show; every window updates without
reload.

## Decisions

None. This is the riskiest v1-retirement chunk — take it before the bulk data-route
migrations (14–20) so the event backbone is settled first.
