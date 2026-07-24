# 16b — Migrate remaining production show-object writers

## Context

16a establishes v2 snapshots and typed output-route intents. Remaining production callers still
use generic whole-object writes for layouts, preload compatibility, and patch compatibility.

## Work

1. Migrate user-layout/window settings to typed partial intents.
2. Route patch callers through the existing v2 Patch service.
3. Move preload storage into its typed recording surface.
4. Preserve scoped store revision installation and stale-client re-read/reapply behavior.

## Definition of done

- No production desk UI whole-object PUT remains.
- Every migrated writer carries request identity and typed semantic intent.
- Existing persisted shows and operator behavior remain unchanged.

## Verification

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from chunk 16. No open decisions.

## Result

- Added typed, idempotent v2 object-intent routes for user layouts and patch layers, plus
  server-owned recording routes for dynamics and Preload.
- Moved object identity into route operands where applicable, kept show identity as the
  optional guard header, and preserved unknown persisted fields during partial updates.
- Migrated the production UI writers away from generic whole-object PUTs. Revision conflicts
  now re-read authoritative state and deliberately reapply the operator intent.
- Reused the existing Preload commit boundary so the v1 compatibility route and v2 intent
  route retain identical persistence, activation, event, and active-Preload release behavior.
- Added focused server and client coverage for typed payloads, ownership, revision conflicts,
  unknown-field preservation, replay idempotency, and server-side dynamic composition.

Verification completed:

```text
cargo test -p light-wire
cargo test -p light-server --no-default-features show_object_intents_v2_route_tests
npm --prefix apps/control-ui run typecheck
npm --prefix apps/control-ui run test -- --run src/features/server/patch.test.ts src/api/LightApiClient.test.ts
npm run test:architecture
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```
