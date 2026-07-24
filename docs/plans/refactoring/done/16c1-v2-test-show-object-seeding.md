# 16c1 — Add v2 test show-object seeding and migrate acceptance callers

## Context

Root Playwright and bench scenarios still seed portable show objects through the generic v1
whole-object mutation routes. Production writers no longer use those routes.

## Work

1. Add typed test-bench-only v2 seed and delete routes for active-show object fixtures.
2. Keep the routes loopback/test-bench gated and reuse the production validation, activation,
   event, and revision boundaries.
3. Migrate shared root-test helpers and direct acceptance callers off generic v1 PUT/DELETE.
4. Keep v1 read compatibility for later owning chunks.

## Definition of done

- Root tests contain no generic v1 show-object PUT or DELETE calls.
- Test seeding cannot be exposed by a production server.
- Seeded active-show behavior and revisions match the former compatibility path.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from chunk 16. No open decisions.

## Result

- Added a test-bench-only v2 show-object fixture route whose put/delete actions reuse the
  production compatibility implementation, including validation, activation, events, backups,
  and optimistic revisions.
- Kept the route behind the manual-clock test router and proved the production router returns
  `404`.
- Moved shared bench helpers and every root Playwright generic object PUT/DELETE caller to the
  gated v2 route; retained v1 reads and the separately owned undo compatibility calls.
- Verified Rust formatting/checks, TypeScript typechecking, architecture checks, all server and
  unit tests, and all 86 API E2E scenarios. The full E2E run passed 284 scenarios with 11 skips
  and one unrelated telemetry sampling timeout; that telemetry scenario passed immediately in
  isolation.
