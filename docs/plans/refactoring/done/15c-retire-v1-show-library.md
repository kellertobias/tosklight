# 15c — Migrate acceptance, desktop, and tool show-library callers

## Context

15a added the typed, replay-safe v2 service and 15b migrated the production UI client. This
landing migrates external acceptance, desktop, and tool callers. Server route-test migration and
route deletion are isolated in 15d.

## Work

1. Add typed show-library helpers to the shared E2E API driver.
2. Migrate bench helpers, root acceptance, desktop smoke, and operational scripts from v1
   show-library and MVR routes to the v2 snapshot/actions/blob contracts.
3. Keep server route tests and v1 registrations intact until 15d.

## Definition of done

- No acceptance, desktop-smoke, bench, or operational-tool caller remains for v1 show-library
  or MVR routes.
- Show lifecycle and MVR acceptance coverage exercises v2.
- Persisted compatibility and startup recovery remain unchanged.

## Verification

```sh
npm run test:unit
npm run test:e2e -- tests/05-virtual-time-persistence-and-recovery.spec.ts
npm run test:e2e
npm run test:desktop-smoke
```

Manual: `npm run open` → open, rename, overwrite, download a show; import an MVR preview.

## Decisions

Server route-test migration and v1 deletion are split into 15d. No open decisions.

## Result

- Added shared E2E driver helpers for typed v2 show snapshots and semantic create, open,
  default, rollback, rename, overwrite, revision-save, and revision-open actions.
- Migrated root acceptance scenarios, bench setup, help/show generation, desktop smoke, and
  direct Playwright callers to v2 show-library actions and download routes.
- Updated production snapshot authentication after the focused UI gate exposed that the v2
  library is authenticated, unlike the retired v1 listing behavior.
- Verified no acceptance, desktop-smoke, bench, or operational-tool caller remains for the v1
  show-library or MVR route families; remaining v1 show paths are object/preset/preload routes.
- Verified `npm run test:unit` (277 frontend files, 2,001 frontend tests),
  `npm run test:e2e -- tests/05-virtual-time-persistence-and-recovery.spec.ts` (31 passed),
  `npm run test:e2e` (285 passed, 11 skipped), and `npm run test:desktop-smoke` (2 passed).
