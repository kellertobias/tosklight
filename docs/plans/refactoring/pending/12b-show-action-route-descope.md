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
