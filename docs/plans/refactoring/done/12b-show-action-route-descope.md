# 12b — De-scope retained show-action routes

## Context

Depends on 12a's shared optional context extractors. These retained v2 routes still use
the active Show as a path scope rather than as server authority plus an optional guard.

## Work

Remove `{show_id}` scope segments from playback topology, programming update, Group
management, Group recording, Cue recording, Cue transfer, Preset recall, and Preset
recording routes. Resolve the active Show through the shared extractor, migrate each
UI/bench/test caller with the route, and send `X-Tosk-Show` from desk UI edits.

## Definition of done

- None of these eight route families carries a show scope segment.
- Missing guards remain valid for integrators; matching guards pass and mismatches fail
  without mutation.
- Typed request identity, replay, revisions, and outcomes are unchanged.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from parent chunk 12. No open decisions.

## Result

- Removed active-Show path scope from playback topology, Programming Update, Group
  management/recording, Cue recording/transfer, and Preset recall/recording routes.
- Extended the shared Show context boundary to resolve the active Show while preserving
  headerless integrator calls and rejecting mismatched guards before mutation.
- Migrated desk transports, bench helpers, server tests, and lock-boundary route matching
  while preserving typed errors, revisions, replay, and outcomes.
- Verified with `cargo test -p light-server --no-default-features` (443 passed, 1
  ignored), `npm run test:unit` (including 1,999 frontend tests),
  `npm run test:e2e-api` (86 passed, 1 skipped), and `npm run test:e2e` (285
  passed, 11 skipped).
