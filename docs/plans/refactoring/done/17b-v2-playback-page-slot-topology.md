# 17b — Move playback page-slot topology onto v2

## Context

Playback definitions and page-slot mappings are active-show objects, but save and clear still use
whole-operation v1 URLs. Their revisions and atomic two-object mutation behavior must remain intact.

## Work

1. Add typed v2 playback-slot configure and clear intents with request identity, tolerant bodies,
   optional show guard, and typed outcomes.
2. Migrate the control client, playback configuration scenarios, persistence/recovery helpers, and
   other root-test seed callers.
3. Delete the v1 playback-page slot PUT/DELETE route and add route-absence coverage.
4. Preserve atomic playback/page revisions, replay behavior, recovery, and explicit page/slot
   addressing.

## Definition of done

- Playback page-slot configuration uses typed v2 intents exclusively.
- The v1 playback-page slot mutation route is absent.
- Atomic revisions, persistence recovery, and explicit page/slot addressing remain green.

## Verification

```sh
cargo test -p light-server active_show_playback
npm run test:unit
npm run test:e2e -- tests/playbackConfiguration
npm run test:e2e
```

## Decisions

Inherited from chunk 17. No open decisions.

## Result

- Reused the existing typed playback-topology application service as the sole configure/clear
  boundary, including request replay, optional Show guards, exact object authority, and atomic
  Playback/Page outcomes.
- Made playback-topology request bodies tolerant through the shared unknown-field extractor while
  keeping known authority fields required and typed.
- Migrated playback-configuration, persistence/recovery, and Matter seed helpers to the v2 action
  contract; removed dead whole-operation client methods and the parallel v1 PUT/DELETE handlers.
- Preserved lossless unknown portable fields, exact explicit page/slot addressing, no-op revision
  behavior, multi-page clear transactions, stale-write rejection, and runtime recovery.
- Verified focused wire, topology-route, and active-show playback tests; the control UI build;
  59 focused owning E2E tests; `npm run test:unit` (2,004 frontend tests and all Rust workspace
  tests); `npm run test:e2e` (285 passed / 11 skipped); and `npm run test:architecture`.
- The plan's `tests/playbackConfiguration` Playwright argument matches no spec directly; the actual
  owner `tests/07-playback-configuration.spec.ts` was run with the persistence and Matter owners.
