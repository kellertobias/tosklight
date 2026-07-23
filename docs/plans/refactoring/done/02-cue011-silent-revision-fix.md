# 02 — CUE-011: fix the silent cue_list revision bump, unskip both tests

## Context

CUE-011 @ui and @supplemental-ui remain skipped
(`tests/06-cuelist-view-and-settings.spec.ts:23` skip reason, `:186` hard `test.skip`).
The full mechanism was verified against the code on 2026-07-23:

1. **Stored zero survives the merge.** `crates/playback/src/model/cue.rs:147` —
   `chaser_xfade_millis: u64` is `#[serde(default, skip_serializing_if = "is_zero_u64")]`.
   Canonical serialization omits the zero, so in
   `crates/application/src/lossless_json.rs` (`merge_typed_request`, L33-54; delta apply
   L46; key removal only for keys present in `before`, L79) a stored raw
   `chaser_xfade_millis: 0` is never touched and persists into the saved body.
   The merge is invoked from `crates/application/src/playback_topology/candidate.rs:368-381`
   (`cue_list_body`, called at `:168` in `save_cue_list`).
2. **Migration write-back is silent.** `crates/application/src/show_compiler/migrations/objects.rs:32-82`
   (`migrate_cue_list`) unconditionally strips `chaser_xfade_millis` (`:40`) and re-derives
   `chaser_xfade_percent`; an update is produced when bytes differ (`:29`). The write-back
   happens via `stage_updates` in `crates/application/src/show_compiler/migrations/mod.rs:60-70`
   using bare `transaction.put` (`:67`) — **no `ActiveShowObjectChange`, so no
   `show_object_changed` event is ever published** for the revision bump.
3. Revision bumps are unconditional: `crates/application/src/active_show/objects.rs:98`
   (`next_revision`, defined `:235`), and the change-event emit is caller-supplied by
   construction (`apply_mutation` returns the change, `:107-123`).

Net effect: save a cue_list through the topology path with a legacy
`chaser_xfade_millis: 0` in its stored raw body → next non-preserving prepare pass
(`show_compiler/prepare.rs:31/:70`; the topology change path itself defers via
`prepare_show_candidate_exact_transaction`, `change.rs:6/:61`) rewrites the body and
bumps the revision with no request and no event. The UI can never learn the new revision.

## Work

Fix **both** halves (either alone leaves a class of silent bumps):

1. **Stop the echo persisting:** after `cue_list_body` merges, strip fields the canonical
   model skip-serializes when they hold the skip value (minimally `chaser_xfade_millis: 0`),
   or teach the merge to drop stored keys absent from both canonical serializations.
2. **Make normalize write-backs observable:** route migration `stage_updates` writes through
   a path that produces `ActiveShowObjectChange` / publishes `show_object_changed` (this also
   covers the group normalize-once bump documented in the §6 undo-test fix).
3. Reproduce first with a failing server-side test: store a cue_list carrying
   `chaser_xfade_millis: 0`, trigger a document read/prepare, assert either no revision bump
   (half 1) or an emitted `show_object_changed` (half 2).
4. Unskip CUE-011 @ui (`:23` `skip:` entry) and @supplemental-ui (`:186`).

## Definition of done

- New server-side regression test(s) covering both halves.
- Both CUE-011 tests unskipped and green in the full suite.

## Verification

```sh
cargo test -p application
npm run test:unit
npm run test:e2e -- tests/06-cuelist-view-and-settings.spec.ts
npm run test:e2e   # full suite, no net new regressions
```

CUE-011 was flaky-adjacent historically — re-run in isolation before trusting a failure.

## Decisions

None — but if half 2 turns out to require touching every migration call site, record the
scope increase in the result note rather than silently expanding.

## Result

- **Half 1 (echo strip):** `lossless_json::strip_zero_u64_echo` removes a merged
  `chaser_xfade_millis: 0` echo; applied in both cue_list merge paths —
  `normalize_cue_list` (`active_show/objects.rs`) and `cue_list_body`
  (`playback_topology/candidate.rs`). The canonical model treats the zero as absent, so
  merged bodies stay byte-stable across migration passes and topology saves stop
  re-persisting the legacy key.
- **Half 2 (observable write-backs):** `commit_object_changes` now collects migration
  write-backs from `PortableShowCommit::written_objects()` (dedup against requested
  changes), publishes them inside the one `active_show_objects_changed` event, and
  returns them as a new `migration_changes` field on `MutateActiveShowObjectsResult`
  and `UndoActiveShowObjectResult`. The server funnel
  (`runtime/active_show_objects.rs::emit_migration_object_changes`) emits legacy
  `show_object_changed` (with `"source":"migration"`) per write-back for both the
  mutation and undo paths — central, so every server call site is covered.
- Regression tests: `cue_list_mutation_drops_the_stored_zero_chaser_xfade_echo`
  (verified to fail without the fix) and
  `committed_migration_write_backs_are_published_as_object_changes` in
  `active_show/tests.rs`.
- Both CUE-011 tests unskipped; the focused spec run passes 7/7.
- Suite numbers: application 389 passed; server 408 passed; full e2e
  **276 passed / 11 skipped / 1 failed** (the pre-existing user-dirty product-demo run)
  vs baseline 274/13/1 — two skips converted to passes, no net new regressions.
- Surprises / scope notes: migration write-backs riding along **route** mutations (and
  route-kind migrations themselves) remain unpublished — `mutate_output_route` discards
  the commit's written objects and `from_storage_kind` has no route variant. Filed as
  `pending/02c-route-mutation-migration-observability.md` instead of expanding here.
  Show-open migration commits stay event-less by design (full client re-bootstrap).
