# 16a — Add v2 show-object snapshots and typed output-route intents

## Context

Chunk 16 combines the largest remaining v1 storage surface with several distinct production
writers. This first landing establishes typed v2 reads and one complete writer family.

## Work

1. Add authenticated v2 collection and exact-object snapshots for the active show, with an
   optional show guard and authoritative show revision.
2. Add typed, tolerant, request-identified create/update/delete intents for output-route objects.
3. Reuse bounded session replay semantics and return the committed object/revision authority.
4. Migrate `features/server/output.ts` and its client tests to the v2 route intent.
5. Keep generic v1 object routes intact for 16b/16c.

## Definition of done

- Production output-route editing no longer sends whole objects through generic v1 PUT/DELETE.
- Snapshot reads and route mutations use typed v2 contracts with replay and stale-show protection.
- Output behavior and persisted route compatibility remain unchanged.

## Verification

```sh
cargo test -p light-wire
cargo test -p light-server --no-default-features show_object_v2
npm run test:unit
npm run test:e2e-api
```

## Decisions

Output routes are the first writer family because they exercise create, update, and delete without
overlapping the already-dedicated Patch and Stage Layout v2 services. No open decisions.

## Result

Added authenticated, active-show-scoped v2 collection and exact-object snapshots with
authoritative show revisions. Added typed tolerant output-route create/update/delete intents with
bounded session-and-show replay, optimistic object revisions, committed route projections, and the
existing application mutation boundary.

The production output editor, generic API client reads, cue-transfer repair reads, and focused
snapshot transport now use the v2 routes. Generic v1 object routes remain registered for 16b/16c.
Wire generation, focused server and client coverage, the full unit suite, and API E2E passed.
