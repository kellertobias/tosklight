# 17d — Retire legacy playback HTTP compatibility routes

## Context

Chunk 17c proved that the desk UI sends live playback control through
`playback.action` on the established WebSocket and that the typed
`POST /api/v2/playback-actions` mirror already supports cue-list, playback-pool,
current-page, and explicit-page addresses. The remaining v1 routes are owned by
root acceptance helpers, server compatibility tests, and external integrator
compatibility rather than the production desk UI.

The retained routes are:

- `POST /api/v1/playbacks/{id}/{action}` — cue-list-addressed acceptance and server flow tests.
- `POST|PUT /api/v1/cuelists/{number}/{action}` and
  `POST|PUT /api/v1/playback-pool/{number}/{action}` — playback-number-addressed
  acceptance helpers and one legacy-boundary server test.
- `POST|PUT /api/v1/control-desks/{id}/page-playbacks/{slot}/{action}` —
  explicit-page acceptance helpers.
- `GET /api/v1/cuelists/{number}`, `GET /api/v1/playback-pool/{number}`, and
  `GET /api/v1/playbacks` — mixed configuration/runtime assertions that need a
  typed v2 read replacement or composition from playback topology plus the
  narrow runtime snapshot.

## Work

1. Add one root test helper around `POST /api/v2/playback-actions`, preserving
   action surface and request identity, then migrate every v1 action caller.
2. Convert the server legacy-boundary tests to the v2 HTTP mirror and delete the
   four v1 action route families plus dead compatibility handlers.
3. Replace the three legacy read forms with typed v2 topology/runtime reads,
   adding a deliberately bounded projection only where existing v2 reads cannot
   express an assertion.
4. Delete the v1 read routes and their dead serializers.
5. Keep current-page and explicit-page addressing distinct in helper APIs and
   regression coverage.

## Definition of done

- No playback action or read caller remains on the listed v1 routes.
- The listed v1 playback routes return 404.
- Desk live control remains WebSocket-owned; integrator HTTP actions use only
  `POST /api/v2/playback-actions`.
- Full unit and E2E suites remain green.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e -- tests/04-osc-api-and-cross-surface.spec.ts tests/06-cuelist-view-and-settings.spec.ts tests/28-hardware-connected-playback-selection.spec.ts tests/34-active-playback-colors.spec.ts
npm run test:e2e
node tools/check-architecture.mjs
```

## Decisions

Inherited from chunk 17. No open decisions.

## Result

- Added typed v2 playback-action helpers for cue-list, playback-number,
  current-page, and explicit-page addresses, then migrated all root acceptance
  callers away from the legacy playback HTTP routes.
- Added `GET /api/v2/playback-overview` as a bounded, authenticated,
  desk-scoped replacement for legacy configuration/runtime reads.
- Removed the v1 playback read and action route families plus their dead HTTP
  adapters and serializers. Regression coverage proves the retired routes are
  absent and preserves current-page versus explicit-page addressing.
- Verification passed: `cargo test -p light-server` (472 passed, 1 ignored,
  plus 14 benchmark tests), `npm run test:unit` (277 Vitest files / 2005 tests
  plus the Rust workspace), focused playback E2E (68 passed), architecture and
  source-size gates, and the full E2E repeat (285 passed / 11 skipped).
- The first full E2E run had one PBK-006 timing failure; the exact scenario
  passed in isolation and the complete repeat passed every scenario. The
  Playwright process retained a teardown handle after printing the repeat's
  final counts and was stopped after completion. No follow-up chunk is needed.
