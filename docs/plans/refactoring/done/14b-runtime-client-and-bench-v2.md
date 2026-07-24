# 14b — Move the production runtime client and bench onto v2

## Context

After 14a the replacement runtime routes coexist with v1. Migrate the production desk and
shared test bench before deleting any compatibility route.

## Work

1. Move `LightClientRuntime` bootstrap/session create/session close to the v2 routes and
   update its unit contracts.
2. Move `fixtures.ts` Patch reads to the existing typed `GET /api/v2/patch` snapshot,
   adapting the v2 projection to current consumers without reintroducing a broad legacy
   read.
3. Move `apps/control-ui/e2e/bench/api.ts`, `lightBench.ts`, and operator-output helpers to
   v2 sessions/bootstrap/readiness/Patch.
4. Migrate feature-local production and bench tests touched by those transports; keep the
   remaining root acceptance/operational callers for 14c.

## Definition of done

- A normal browser desk boots, authenticates, closes its session, and reads Patch entirely
  through v2 runtime routes.
- Shared bench scenarios use v2 for runtime lifecycle and readiness.
- V1 routes remain temporarily registered for the explicit 14c caller tail.

## Verification

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e -- tests/00-generate-show-files.spec.ts
```

Manual: `npm run open`; verify readiness and a populated Patch view.

## Decisions

Inherited from Chunk 14. No open decisions.

## Result

- Moved the production browser runtime to v2 bootstrap, session-create, and session-close routes.
- Replaced the fixture compatibility read with the existing authenticated v2 Patch snapshot and
  its typed decoder.
- Moved shared bench readiness/login and operator-output lifecycle helpers to v2.
- Added client contract coverage for v2 bootstrap, authentication, decoded Patch, and close.

Verification passed:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e -- tests/00-generate-show-files.spec.ts
npm run open
```

The opened desktop runtime reported a v2 bootstrap for the active Demo Show and an authenticated
Patch snapshot containing 66 fixtures and 14 referenced profiles.
