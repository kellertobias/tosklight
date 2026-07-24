# 14c — Retire v1 runtime routes and migrate the caller tail

## Context

After 14b only root acceptance helpers, operational scripts/docs, and desktop smoke may
still call v1 bootstrap, sessions, Patch, readiness, or diagnostics.

## Work

1. Migrate every remaining caller of `/api/v1/bootstrap`, `/api/v1/sessions*`,
   `/api/v1/patch`, `/api/v1/readiness`, and `/api/v1/diagnostics`, including root tests,
   `tools/dev.sh`, `tools/build.sh`, `apps/control-ui/e2e/desktop-smoke.mjs`, AGENTS.md,
   engineering/testing/help documentation, and test support helpers.
2. Delete the five v1 runtime route families and the superseded legacy Patch snapshot
   implementation while retaining the v2 Patch service projection.
3. Add explicit 404 regression assertions for every retired route and audit the repository
   for callers or registrations.
4. Verify malformed/legacy active-show recovery, SHOW-005 revision-copy behavior, and the
   packaged desktop boot/ownership path.

## Definition of done

- No caller or registration remains for the retired v1 runtime routes.
- Browser, bench, tools, recovery tests, and desktop smoke use v2 readiness/bootstrap/session
  and Patch contracts.
- Startup recovery and session cleanup semantics are unchanged.

## Verification

```sh
cargo test -p light-server --no-default-features
npm run test:unit
npm run test:e2e-api
npm run test:e2e
npm run test:desktop-smoke
```

## Decisions

Inherited from Chunk 14. No open decisions.
