# 16c — Migrate seeding/tests and retire generic v1 show-object mutations

## Context

16a and 16b migrate production reads and writers. Bench, acceptance, and server tests still seed
portable objects through generic v1 routes before those mutations can be removed.

## Work

1. Add typed v2 seeding helpers and migrate bench, acceptance, and server route tests.
2. Move preset/preload store compatibility onto their typed recording services.
3. Replace public generic object undo with desk-scoped programmer recording undo.
4. Delete generic v1 object PUT/DELETE/undo routes when callers reach zero.
5. Retain only read compatibility if a later plan explicitly owns its removal.

## Definition of done

- Generic v1 whole-object PUT/DELETE and public object undo routes are absent.
- Recording undo is desk-scoped and programmer-owned.
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
