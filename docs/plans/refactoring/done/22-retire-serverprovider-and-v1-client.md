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
   returns **nothing** — chunks 14 and 21 were both decided as full v2 moves
   (2026-07-23), so no exceptions exist. The only tolerated remnant is the test-gated
   `with_test_routes` block; rename those paths to `/api/v2/test/*` here if the bench
   has migrated, so the final grep is clean.
2. Dissolve `ServerProvider`: each composed feature provider mounts directly in
   `App`/`ScreenApp`/`ProductDemoApp` (or a thin `DeskProviders` component that is pure
   composition, no logic). Delete `composeServerContextValue` and the `features/server/*`
   glue that only fed the composition; keep genuinely shared pieces (connection state,
   polling that survives as v2 event handling) in their owning features.
3. Delete `LightApiClient` + emptied `api/client/*` modules (`transport.ts`/`runtime.ts`
   survive only if the v2 transports still extend them — prefer moving what's needed into
   the v2 transport layer and deleting the rest).
4. Final server sweep: delete every remaining v1 route registration (no documented
   exceptions exist); move the `with_test_routes` test paths to v2 naming alongside.
5. Gate with `node tools/check-architecture.mjs`, `node tools/check-source-size.mjs`
   (ServerProvider's 323-line violation must disappear, not be waived), and
   `node tools/test-command-boundaries.mjs`.

## Definition of done

- `ServerContext.tsx` and `LightApiClient.ts` deleted (or reduced to pure v2 composition
  with no v1 vocabulary); `rg '/api/v1'` across the whole repo (crates/, apps/, tests/,
  docs/ minus historical plan files) returns nothing.
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

None — 14 and 21 are decided (full v2 moves). This chunk cannot be claimed before
13–21 are done.

## Execution

Claimed 2026-07-24 after chunks 13–21 were confirmed in `done/`. No maintainer
decision is open.

## Result

Split after current-tree discovery into two independently verifiable chunks:

- 22a owns the final v1 show-object reads, test-gated routes, callers, and documentation sweep.
- 22b owns the provider/client facade teardown, stable connection lifecycle, source-size
  retirement, and product-demo acceptance.

This keeps a mechanical public-route retirement separate from the frontend lifecycle
rearchitecture while preserving the original definition of done across the two child chunks.

## Known bug to fix here: product-demo black screen on Show Patch (diagnosed 2026-07-23)

The `product-demo` e2e run fails (and is `test.skip`ped until this chunk) because of a
clean unmount/remount lifecycle bug in exactly the stack this chunk retires. Trace
evidence from the failing run:

- The demo reaches the Show modal and clicks **Show Patch**; the click lands on the real
  button and the patch surface even starts loading (`objects/patch_layer` and
  `objects/unresolved_…` GETs return 200).
- Within ~20 ms the **entire page** (app + demo companion panels) unmounts to black and
  stays black — no JS error, no pageerror, no navigation.
- At that instant the client fires `DELETE /api/v1/sessions/{id}` (the
  `useServerConnection` effect **cleanup** closing its owned session) followed by a churn
  of `/api/v2/events` reconnects ("WebSocket is closed before the connection is
  established") and **no new bootstrap** — the provider tree tore down and never
  recovered.
- Trigger correlates with `state.builtIn` flipping to `"patch"` (which also flips
  `showBeamGuides` on the demo's Stage card): some dependency of the connection effect
  changes identity on that state transition, re-running the effect cleanup mid-flight.

When rebuilding the provider composition, make the connection lifecycle immune to app-
state-derived dependency churn (stable identities or a connection owner mounted outside
the app-state tree). **Acceptance:** un-skip `tests/product-demo.spec.ts` and the Show
Patch phase passes.
