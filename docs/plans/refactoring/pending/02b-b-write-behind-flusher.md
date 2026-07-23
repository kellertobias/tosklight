# 02b-b — Write-behind flusher: autosave interval, flush boundaries, backups per flush

## Context

Split from `02b-interval-show-persistence` (see its Result note). 02b-a landed the
in-memory active-show document: `AppState::active_show_document` caches the loaded
`PortableShowDocument`, `ServerActiveShowUnitOfWork` validates it against the store's O(1)
portable revision at begin, applies commits in memory via
`PortableShowDocument::apply_commit`, and returns it to the cache on drop
(`crates/server/src/runtime/active_show_adapter.rs`). Commits are still **write-through**
(one SQLite transaction per mutation) and backups still run per mutation, so durability
and all client-observable behavior are unchanged.

This chunk finishes api-rules §8: the in-memory document becomes authoritative and the
file is flushed on an interval instead of per mutation.

## Work (from the parent chunk, unchanged in intent)

1. Stop committing per mutation: mutations apply to the in-memory document only; keep a
   dirty accumulation (transaction log or document snapshot diff) for the flusher.
   Mutation-time client-observable behavior must not change (revisions, events, undo
   pairing — the CUE-011 regression tests are the guard). **Undo caution:** today
   `prepare_object_undo` reads history rows from SQLite; with write-behind the undo
   history for unflushed mutations must come from memory or be part of the flush design.
   **Out-of-band writers caution:** 02b-a's revision-check self-healing relies on
   write-through; once writes are deferred, every direct `ShowStore` writer against the
   active show (selective import adapter, MVR apply, object_api non-typed kinds,
   playback_layout, store_api, preload authority, command presets …) must either funnel
   through the in-memory document or force a flush-and-reload boundary.
2. The flusher: dirty flag + interval timer (configurable `autosave_interval`, default
   30 s, minimum bound); immediate flush on: show switch/open/close, named-revision save
   (`show_library.rs` `backup_to` must see a flushed file), upload/overwrite, deliberate
   application quit (the shutdown route AND the desktop Tauri window-close path), leaving
   the Show Patch (client signals patch-surface exit or server flushes on the last patch
   mutation's settle), and idle (no mutation for a few seconds).
3. Backups move from per-mutation to per-flush (`ShowMutationBackupPlan` before a flush
   containing changes). Retention unchanged.
4. Recovery semantics: read `docs/acceptance-criteria.md` first; SHOW-005 recovery-backup
   naming/selection moves to per-flush granularity — update `docs/testing/` scenario text
   and test expectations deliberately.
5. Crash-consistency test: kill the server between mutation and flush; restart loads the
   last flushed state cleanly.
6. Operator-facing configuration: `autosave_interval` in the desk settings UI alongside
   backup retention (bounded range, 30 s default visible), persisted via desk
   configuration; documented in operator help; refresh help screenshots only if that page
   is captured.

## Definition of done

Parent chunk's DoD minus what 02b-a delivered: no disk write on the mutation hot path;
flush on interval + all named hard boundaries (desktop quit and Show Patch exit each
provably flush); backups per flush; configurable interval; kill-test green; SHOW-005
scenarios updated and green; no client-observable event/revision change.

## Verification

```sh
cargo test -p light-show -p light-application -p light-server
npm run test:unit
bash tools/test.sh e2e --grep "SHOW-"
npm run test:e2e   # full suite gate
npm run open       # real desk: program, kill the process, restart, inspect
```

## Decisions

All decided (api-rules §8; operator-facing setting 2026-07-23). None open.
