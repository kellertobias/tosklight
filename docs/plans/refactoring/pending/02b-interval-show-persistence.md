# 02b — Write-behind show persistence: autosave interval instead of per-mutation commits

## Context (maintainer decision 2026-07-23, recorded in api-rules §8)

Today every active-show mutation opens the `.show` SQLite file, loads the whole document,
takes a **full-file backup copy**, applies one immediate transaction, and commits — all
synchronously per mutation:

- Unit of work: `crates/server/src/runtime/active_show_adapter.rs:171-175` (`begin` →
  `ShowStore::open` + `portable_document()`), commit at `:237`
  (`apply_portable_transaction`), engine snapshot install `:275`.
- Per-commit backup: `ServerActiveShowUnitOfWork::backup` (`active_show_adapter.rs:216`),
  invoked by `crates/application/src/active_show/service.rs:44,135,281`; plan in
  `crates/server/src/runtime/show_mutation_backup.rs` (full `backup_to` copy + retention).
- Durability today is already soft: `crates/show/src/connection.rs:4` sets WAL +
  `synchronous=NORMAL`, so a power loss can drop the newest commits regardless.

Decision: the active show becomes authoritative **in memory**; the file is flushed on a
configurable autosave interval (default **30 s**, exposed in desk configuration alongside
`backup_retention`), plus immediate flushes at hard boundaries. Accepted trade-off:
power loss loses at most the last interval of programming.

## Work

1. Rework the active-show unit of work to hold the loaded `PortableDocument` in memory
   (single writer, behind the existing `state.active_show` locking) instead of re-loading
   from disk per mutation. Mutations apply to the in-memory document and produce
   events/revisions/undo exactly as now — **mutation-time behavior must not change**
   (revisions, `show_object_changed`, replay windows, undo pairing all stay
   client-observably identical; this protects the chunk 02 invariant).
2. Add the flusher: dirty-flag + interval timer (configurable `autosave_interval`,
   default 30 s, minimum bound); flush writes one transaction with all accumulated
   changes. Immediate flush on: show switch/open/close, named-revision save
   (`show_library.rs:57` `backup_to` must see a flushed file), upload/overwrite,
   shutdown, and idle (no mutation for a few seconds).
3. Move automatic backups from per-mutation to per-flush: `ShowMutationBackupPlan` runs
   before a flush that contains changes, not before every commit. Retention unchanged.
4. Recovery semantics: `docs/acceptance-criteria.md` first; SHOW-005 recovery-backup
   naming/selection moves from per-mutation to per-flush granularity — update the
   scenario text in `docs/testing/` and the test expectations deliberately, not silently.
5. Crash-consistency test: kill the server between mutation and flush; on restart the
   show must load cleanly at the last flushed state (no torn document, WAL intact).
6. Configuration surface: expose `autosave_interval` in desk configuration (v2 config
   route per the queue's other chunks); document in the Show-Setup help if operator-facing.

## Definition of done

- No disk write on the mutation hot path; flush on interval + hard boundaries; backups
  per flush; configurable interval with 30 s default; kill-test green; SHOW-005-family
  scenarios updated and green; no change in any event/revision ordering observable by
  clients (unit-covered).

## Verification

```sh
cargo test -p show -p application -p server
npm run test:unit
bash tools/test.sh e2e --grep "SHOW-"
npm run test:e2e   # full suite gate
npm run open       # real desk: program for a minute, pull the process, restart, inspect
```

## Decisions

Interval default 30 s and configurability are decided (api-rules §8). One small open
point: whether the interval is operator-facing (help + settings UI) or an internal
desk-configuration key only — cheap either way; propose internal-only in the result note
unless the maintainer says otherwise.

Sequence: after 02 (CUE-011 — its regression tests are exactly the guard this rework
must keep green). Before 16 (object intent updates then build on the write-behind path
instead of being reworked twice). This chunk is large; split `02b-a` (in-memory document)
/ `02b-b` (flusher + backups) at execution time if needed.
