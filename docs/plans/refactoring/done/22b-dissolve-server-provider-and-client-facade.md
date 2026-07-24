# 22b — Dissolve ServerProvider and the LightApiClient facade

## Context

After 22a removes the final v1 routes and callers, the remaining Chunk 22 work is the frontend
capstone: replace the broad `ServerProvider` controller with stable, owner-scoped provider
composition; retire `composeServerContextValue` and obsolete `features/server` glue; and remove
the `LightApiClient` facade while preserving the focused v2 transports.

## Work

1. Give connection/session lifecycle one stable owner whose effect cannot churn with
   app-state-derived callback identity.
2. Mount focused providers directly through a thin pure-composition component in App,
   ScreenApp, StageViewApp, and ProductDemoApp.
3. Move shared v2 transport construction/configuration to its owning modules; delete
   `LightApiClient`, `ServerContext`, and obsolete glue after callers reach zero.
4. Unskip and pass the complete product demo Show Patch phase.
5. Remove the ServerProvider source-size violation rather than waiving it.

## Definition of done

- `ServerContext.tsx` and `LightApiClient.ts` are deleted or reduced to genuinely narrow v2
  ownership with no broad facade.
- Product demo, architecture, source-size, command-boundary, unit, E2E, desktop smoke, and
  real desktop boot gates pass.

## Verification

```sh
node tools/check-architecture.mjs
node tools/check-source-size.mjs
node tools/test-command-boundaries.mjs
npm run test:unit
npm run test:e2e
npm run test:desktop-smoke
npm run open
```

## Decisions

Inherited from Chunk 22. No open decisions.

## Result

- Replaced the broad server provider with a stable connection owner and focused capability
  composition shared by the main app, screen, stage, and product-demo surfaces.
- Replaced `LightApiClient` with an explicit capability registry and migrated server consumers to
  their owning API modules.
- Kept a narrow contract-only `ServerContext` export for legacy virtual-module mocks while deleting
  the broad runtime context and composition facade.
- Completed the product-demo Patch phase, including typed v2 fixture seeding, patch repair, Preload
  range writes, and fresh playback authority at Preload GO.
- Made Preload UI acceptance checks wait for the authoritative commit boundary before advancing
  virtual time.

Verification completed:

- `node tools/check-architecture.mjs`
- `node tools/check-source-size.mjs`
- `node tools/test-command-boundaries.mjs`
- `npm run test:unit` (277 frontend files / 1,992 frontend tests plus all Rust workspace tests)
- `npm run test:e2e` (287 passed, 9 intentionally skipped)
- `npm run test:desktop-smoke` (2 passed)
- `npm run open`
- `curl -fsS http://127.0.0.1:5000/api/v2/readiness` (`status: ready`)
