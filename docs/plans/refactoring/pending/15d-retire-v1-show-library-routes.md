# 15d — Migrate server tests and retire v1 show-library routes

## Context

15a added the typed service, 15b migrated production UI, and 15c migrated external acceptance
callers. Server route tests still exercise v1 show-library paths before the registrations can be
deleted.

## Work

1. Migrate server route tests from v1 show-library and MVR routes to v2 actions and snapshots.
2. Keep v1 show-object, preset-store, and preload-store routes for chunks 16 and later.
3. Delete only the v1 show-library and MVR registrations and add explicit 404 assertions.
4. Run the full server suite and repeat the persisted compatibility/recovery gates.

## Definition of done

- No caller or registration remains for the v1 show-library or MVR routes.
- Explicit route tests prove those retired paths return 404.
- Persisted compatibility and startup recovery remain unchanged.

## Verification

```sh
cargo test -p light-server --no-default-features
npm run test:unit
npm run test:e2e -- tests/05-virtual-time-persistence-and-recovery.spec.ts
```

## Decisions

Inherited from 15a. No open decisions.
