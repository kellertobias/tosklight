# 22 — Retire ServerProvider composition + LightApiClient; final v1 sweep

## Context (verified 2026-07-23)

`useServer()` is gone, but the machinery behind it survives:

- `apps/control-ui/src/api/ServerContext.tsx` — `ServerProvider` (**323 lines**, a
  current source-size "new function" violation), used by `App.tsx`, `ScreenApp.tsx`,
  `ProductDemoApp.tsx`. It composes ~20 feature providers and wires
  `useServerConnection`, `useServerState`, `useServerPolling`, `useCommandLineController`,
  `useFileAccess`, `composeServerContextValue`, `useServerFeatureBoundaries`
  (all `features/server/*`).
- `apps/control-ui/src/api/LightApiClient.ts` binds ~16 client modules under
  `api/client/*`; consumed by `features/server/connectionBootstrap.ts`, `system.ts`,
  `useServerState.ts`, `features/screens/actions.ts`, `features/patch/PatchFeatureBoundary.tsx`,
  `features/files/actions.ts`, `components/shell/ConnectionState.tsx`, …

This is the capstone: it only becomes possible after chunks 13–21 empty the v1 client
modules. Do not start it while any `api/client/*` module still carries a live v1 call.

## Work

1. Verify preconditions: `rg -F '/api/v1' apps/control-ui/src apps/control-ui/e2e tests`
   returns only deliberate exceptions (readiness/diagnostics per chunk 14's decision,
   files/help if chunk 21 chose exemption).
2. Dissolve `ServerProvider`: each composed feature provider mounts directly in
   `App`/`ScreenApp`/`ProductDemoApp` (or a thin `DeskProviders` component that is pure
   composition, no logic). Delete `composeServerContextValue` and the `features/server/*`
   glue that only fed the composition; keep genuinely shared pieces (connection state,
   polling that survives as v2 event handling) in their owning features.
3. Delete `LightApiClient` + emptied `api/client/*` modules (`transport.ts`/`runtime.ts`
   survive only if the v2 transports still extend them — prefer moving what's needed into
   the v2 transport layer and deleting the rest).
4. Final server sweep: delete any v1 route registration still standing outside the
   documented exceptions; delete `with_test_routes` v1 test paths only if bench has
   migrated (they may stay — they're test-gated).
5. Gate with `node tools/check-architecture.mjs`, `node tools/check-source-size.mjs`
   (ServerProvider's 323-line violation must disappear, not be waived), and
   `node tools/test-command-boundaries.mjs`.

## Definition of done

- `ServerContext.tsx` and `LightApiClient.ts` deleted (or reduced to pure v2 composition
  with no v1 vocabulary); `rg '/api/v1' crates/server` shows only the named exceptions.
- Architecture/source-size/command-boundary checks clean; full suite green.

## Verification

```sh
node tools/check-architecture.mjs
node tools/check-source-size.mjs
node tools/test-command-boundaries.mjs
npm run test:unit
npm run test:e2e
npm run test:desktop-smoke
npm run open   # real desk boot; curl -fsS http://127.0.0.1:5000/api/v1/readiness (or its successor)
```

## Decisions

None new — but this chunk consumes the decisions from 14 (readiness/diagnostics home) and
21 (files/help). It cannot be claimed before those are resolved and 13–20 are done.
