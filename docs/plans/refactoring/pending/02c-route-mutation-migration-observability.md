# 02c — Publish migration write-backs riding along route mutations

## Context

Chunk 02 made compatibility-migration write-backs observable for the generic object path:
`ActiveShowService::commit_object_changes` now collects them from `PortableShowCommit::written_objects()`
(`crates/application/src/active_show/service.rs`, `migration_changes`), publishes them in the
`active_show_objects_changed` application event, returns them on
`MutateActiveShowObjectsResult`/`UndoActiveShowObjectResult`, and the server funnel emits legacy
`show_object_changed` per write-back (`crates/server/src/runtime/active_show_objects.rs`,
`emit_migration_object_changes`).

Two commit paths still leave migration revision bumps unpublished:

1. **Route mutations** — `mutate_output_route` (`service.rs`) commits a transaction prepared by
   `prepare_route_mutation` → `prepare_show_candidate`, so pending object migrations join the
   commit, but only `output_route_changed` is published; the commit result's written objects are
   discarded.
2. **Route migration write-backs themselves** are invisible on every path:
   `ActiveShowObjectKind::from_storage_kind` has no `route` variant, so `migration_changes`
   skips migrated routes (routes have their own `OutputRouteChange` event family).

Show-open (`crates/server/src/runtime/show_compile.rs::commit_migration`) also commits migrations
without per-object events, but the client fully re-bootstraps on `show_opened`, so nothing goes
stale — deliberately out of scope.

## Work

1. In `mutate_output_route`, collect non-route migration write-backs from the commit (reuse
   `migration_changes`) and publish them via `active_show_objects_changed`; surface them on
   `MutateOutputRouteResult` so the server can emit legacy `show_object_changed`.
2. Decide how migrated routes should be published (extra `output_route_changed` events per
   migrated route, on both the object path and the route path), and implement it.
3. Regression test: seed a pending object migration (e.g. legacy `chaser_xfade_millis: 0`
   cue_list), mutate a route, assert the migration bump is published.

## Verification

```sh
cargo test -p light-application
npm run test:e2e-api
npm run test:e2e   # full suite gate
```
