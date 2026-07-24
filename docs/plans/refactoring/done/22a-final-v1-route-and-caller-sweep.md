# 22a — Final v1 route and caller sweep

## Context

Chunk 22 discovery found two independently executable concerns: remaining v1 route/caller
retirement, and the broader provider/client facade teardown. The server still exposes generic
show-object GET compatibility plus three test-gated v1 utilities; root acceptance and Rust tests
still call those reads. These can be migrated without changing provider composition.

## Work

1. Migrate every remaining show-object read caller to the existing typed
   `/api/v2/objects/{kind}[/{id}]` snapshots with show-scope headers.
2. Rename the test-gated clock/output routes and all callers to `/api/v2/test/*`.
3. Delete the generic v1 show-object GET registrations and migrate feature-local Rust tests.
4. Remove stale operator/testing v1 labels and retain route-absence assertions only where they
   add regression value without leaving literal v1 vocabulary in production code.

## Definition of done

- No served v1 route remains, including test-gated routes.
- Apps, E2E, root tests, server sources, and current documentation contain no live v1 caller.
- Typed v2 snapshot semantics, test-clock behavior, and output-failure injection remain green.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e
```

## Decisions

Inherited from Chunk 22. No open decisions.

## Execution

Claimed 2026-07-24 immediately after the parent split. No maintainer decision is open.

## Result

Completed 2026-07-24.

- Removed the final served v1 show-object routes and their dead compatibility handlers.
- Migrated root, bench, and server tests to authenticated v2 active-show snapshots, including the
  snapshot envelopes and explicit show activation required by that contract.
- Moved the gated clock and output-failure utilities to `/api/v2/test/*`.
- Removed obsolete route-absence coverage, updated current operator/developer wording, and added an
  architecture guard against new production TypeScript v1 callers.
- Preserved inactive-show verification through authenticated v2 show downloads rather than
  reintroducing a compatibility read route.

Verification passed:

- `npm run test:architecture`
- `cargo check -p light-server`
- `cargo test -p light-server` (469 library tests and 14 benchmark tests passed; one standard Matter
  network test remains ignored)
- `npm run test:unit` (277 frontend files / 2008 frontend tests plus the Rust workspace)
- `npm run test:e2e -- tests/00-generate-show-files.spec.ts` (4 passed)
- focused `SHOW-005` Playwright run (2 passed)
- `npm run test:e2e` (285 passed, 10 skipped; one unrelated CUE-014 UI interaction failed once)
- focused `CUE-014` Playwright rerun (3 passed)
