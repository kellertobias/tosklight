# 15c — Migrate remaining show-library callers and retire v1

## Context

15a added the typed, replay-safe v2 service and 15b migrated the production UI client. Bench,
root acceptance, desktop smoke, operational tools, and server route tests still use v1 lifecycle
paths and must move before those routes can be removed.

## Work

1. Migrate bench helpers, root tests, desktop smoke, server route tests, and operational scripts
   from v1 show-library and MVR routes to the v2 snapshot/actions/blob contracts.
2. Keep v1 show-object, preset-store, and preload-store routes for chunks 16 and later.
3. Delete only the v1 show-library and MVR registrations and add explicit 404 assertions.
4. Run SHOW-005, malformed/legacy recovery, full E2E, desktop smoke, and the manual
   open/rename/overwrite/download/MVR-preview path.

## Definition of done

- No caller or registration remains for the v1 show-library or MVR routes.
- Show lifecycle and MVR acceptance coverage exercises v2.
- Persisted compatibility and startup recovery remain unchanged.

## Verification

```sh
cargo test -p light-server --no-default-features
npm run test:unit
npm run test:e2e -- tests/05-virtual-time-persistence-and-recovery.spec.ts
npm run test:e2e
npm run test:desktop-smoke
```

Manual: `npm run open` → open, rename, overwrite, download a show; import an MVR preview.

## Decisions

Inherited from 15a. No open decisions.
