# 17c — Move client removal to v2 and finish the playback action audit

## Context

Client removal remains on `DELETE /api/v1/clients/{id}`. Live playback UI already uses WebSocket
actions, while legacy HTTP cuelist, pool, explicit-page, and master action routes remain for tests,
OSC scenarios, or integrators. Their ownership must be audited before retirement.

## Work

1. Add a typed v2 client-removal edit with request identity and migrate UI plus history/removal tests.
2. Delete the v1 client-removal route and add absence coverage.
3. Prove the desk UI has no legacy cuelist/pool live-action caller; keep plain HTTP action forms only
   where required by API rule 2, under v2 naming.
4. Migrate or retire eligible legacy playback action/read routes while preserving explicitly deferred
   test/integrator surfaces.
5. Add a dedicated pass covering current-page versus explicit-page playback addressing.

## Definition of done

- Client removal uses v2 and the v1 route is absent.
- No desk UI live-control action uses v1 HTTP.
- Every retained legacy playback route has an explicit deferred owner; current-page and explicit-page
  scenarios remain green.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e -- tests/04-osc-api-and-cross-surface.spec.ts tests/22-client-history-and-removal.spec.ts tests/34-active-playback-colors.spec.ts
npm run test:e2e
```

## Decisions

Inherited from chunk 17. No open decisions.
