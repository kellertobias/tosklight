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

## Result

- Removed the generic v1 whole-object PUT and DELETE route registration while retaining the
  explicitly deferred v1 GET projections.
- Replaced the retired public mutation handlers with a private test-seeding boundary and migrated
  Rust route setup plus the operator-output bench to typed v2 or test-only seeding paths.
- Removed the dead generic object mutation methods from the control client and added explicit
  METHOD_NOT_ALLOWED coverage alongside typed v2 output-route lifecycle coverage.
- Verified formatting, architecture, persisted-show normalization, old-show recovery, application
  and server tests, 2,002 frontend unit tests, 86 API E2E tests with one intentional skip, and 285
  full E2E tests with 11 intentional skips. The known selection-action cross-test ordering flake
  appeared once in the standalone Rust aggregate, then passed in isolation and in the complete
  `test:unit` run.
