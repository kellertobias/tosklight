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

## Result

- `mutate_output_route` now reports and publishes both rider classes from its commit:
  typed-object migration write-backs (`migration_changes`, published in one
  `active_show_objects_changed` event) and route-kind migration write-backs
  (`migrated_routes`, each published as its own `output_route_changed` with the
  requested route excluded). Decision recorded: migrated routes are published through
  the route event family, not as object changes.
- `commit_object_changes` (object mutations + undo) gained the same route-rider
  collection via the new `migrated_route_changes` helper, so route migrations riding
  any object commit are published too — closing the `from_storage_kind` gap noted in
  chunk 02.
- Server funnels emit the legacy `show_object_changed` (kind `route`,
  `"source":"migration"`) for migrated routes at all three sites: object mutation,
  object undo, and route action.
- Regression test `route_mutation_publishes_migration_write_backs` seeds a legacy
  cue_list echo plus a destination-less legacy route, mutates a different route, and
  asserts both riders in the result, the persisted normalizations, and the exact event
  sequence (requested route → object riders → route rider).
- Suite numbers: light-application 390 and light-server 411 passed; `test:e2e-api`
  85 passed / 1 skipped; full e2e **276 passed / 11 skipped / 1 failed** (pre-existing
  user-dirty product-demo) — no net new regressions.
- No surprises; scope as filed.
