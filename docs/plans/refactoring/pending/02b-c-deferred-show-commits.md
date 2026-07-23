# 02b-c — Defer the SQLite commit itself: true write-behind for the active show

## Context

Final stage of the api-rules §8 write-behind decision, split from `02b-b` (see its
Result). Landed so far: the in-memory document (02b-a) and interval-gated recovery
checkpoints with the operator-facing `autosave_interval_seconds` setting (02b-b) — the
per-mutation full-file backup copy is gone from the hot path. What still happens per
mutation is the small SQLite portable transaction (write-through), which keeps every
disk reader and out-of-band writer trivially consistent.

## Why this is its own chunk (read/write inventory required)

Deferring the commit makes disk stale between flushes, so **every** active-show disk
touch must be handled first:

- **Readers** (would serve stale data): `store_api` object list/GET, `object_api`
  validation reads, `preset_recall_ports`, `preload/authority`, `playback_layout`,
  `command_presets`, `selective_import_adapter`, `auth_backup`, MVR export, show
  download, named-revision save. Options per site: read through
  `state.active_show_document`, or flush-before-read at a funneled
  "open active show store" helper.
- **Direct writers** (their put + a later flush replay would revision-conflict):
  `object_api` non-typed kinds (e.g. `user_layout`), `store_api` puts against the
  active show, MVR apply, selective import. Each needs flush-first + cache
  invalidation, or funneling through the unit of work.
- **Undo**: `prepare_object_undo` reads history rows from SQLite; unflushed mutations'
  undo state must come from memory, or undo becomes a flush boundary.

## Work

1. Inventory and funnel the readers/writers above (helper:
   `open_active_show_store(state)` that flushes pending work first).
2. Defer commits: mutations apply to the in-memory document only; keep the ordered
   pending-transaction log; flush replays it (revision counters and undo rows must end
   byte-identical to write-through — the client-observable revision/event stream is
   already per-mutation and must not change).
3. Flush triggers: autosave interval (reuse `autosave_interval_seconds`), idle
   (no mutation for a few seconds), and hard boundaries — show switch/open/close,
   named-revision save, upload/overwrite, `POST /shutdown` AND the desktop Tauri
   window-close path, leaving the Show Patch (client flush intent or last-patch-settle).
4. Take the recovery checkpoint before each flush that contains changes (the 02b-b
   gate already runs at most once per interval).
5. Crash-consistency test: kill the server between mutation and flush; restart loads
   the last flushed state cleanly (no torn document).
6. Dedicated coverage for the two maintainer-named boundaries: desktop quit and Show
   Patch exit each provably flush.

## Verification

```sh
cargo test -p light-show -p light-application -p light-server
npm run test:unit
bash tools/test.sh e2e --grep "SHOW-"
npm run test:e2e   # full suite gate
npm run open       # real desk: program, kill -9, restart, inspect
```

## Decisions

All decided (api-rules §8). Accepted trade-off: power loss loses at most the last
autosave interval of programming.
