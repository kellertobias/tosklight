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
