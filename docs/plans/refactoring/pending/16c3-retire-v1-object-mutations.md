# 16c3 — Migrate Rust route tests and retire v1 object mutations

## Context

After root tests and programmer recording undo migrate, only Rust route tests and the generic
v1 PUT/DELETE handlers remain.

## Work

1. Migrate feature-local Rust route tests to typed v2 services or direct pre-open store seeding.
2. Replace tests of generic mutation behavior with typed-service and route-absence coverage.
3. Delete generic v1 object PUT and DELETE handlers and route registration.
4. Retain only v1 object reads where later plans explicitly own their removal.

## Definition of done

- Generic v1 whole-object PUT and DELETE routes are absent.
- No repository caller references those mutation URLs.
- Persisted-data normalization and old-show recovery remain green.

## Verification

```sh
cargo test -p light-server -p light-application
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from chunk 16. No open decisions.
