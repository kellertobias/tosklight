# 15b — Migrate show-library callers and retire v1

## Context

15a provides the typed, replay-safe v2 service while v1 coexists. This landing migrates all
production, bench, test, tool, and desktop callers and then removes the v1 show-library routes.

## Work

1. Migrate `features/showLifecycle`, Quick Setup, recovery, bench helpers, root tests,
   desktop smoke, and operational scripts to the v2 show-library contracts.
2. Keep v1 show-object, preset-store, and preload-store routes for chunks 16 and later.
3. Delete only the v1 show-library and MVR route families and add explicit 404 assertions.
4. Run SHOW-005, malformed/legacy recovery, full E2E, desktop smoke, and the manual
   open/rename/overwrite/download/MVR-preview path.

## Definition of done

- No caller or registration remains for the v1 show-library or MVR routes.
- Show browser, recovery, overwrite, revisions, downloads, and MVR transfer use v2.
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
