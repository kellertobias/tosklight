# 15 — Show library/lifecycle onto v2; retire the v1 show routes

## Context (verified 2026-07-23)

`apps/control-ui/src/api/client/shows.ts` (consumed via `features/showLifecycle`) drives
the full v1 show library surface (`http_router.rs:134-159`): list/create shows,
default/open, rollback, `{id}/open`, rename, download, overwrite, revisions (+ open),
MVR preview/apply/download. Bench + tests also call shows list/open/revisions.

These are **library routes** — the show id is the operand, which api-rules §6 permits.
The work is re-homing them as v2 with intent-shaped writes, not de-scoping.

## Work

1. Design the v2 show-library surface: snapshot reads (show list incl. revisions metadata),
   intent posts (`open`, `rename`, `overwrite`, `rollback`, `revision open`, MVR
   preview/apply). Edits carry request ids with a replay window (api-rules §3) — an
   interrupted `overwrite`/`open` must not re-execute on retry. Wire types in
   `crates/wire`, tolerant typing (chunk 08 helper).
2. Downloads (`{id}/download`, `{id}/mvr*`) are GET blob endpoints — keep GET, move under
   v2 paths.
3. Migrate `features/showLifecycle` (QuickSetupModal, ShowRecoveryModal paths) onto the new
   transport; migrate bench helpers and root tests.
4. Delete the v1 show routes as their last caller moves; `presets/{id}/store`
   (`http_router.rs:173`, used only by `tests/support/updateHighlight/highlight.ts`) and
   `preload/store` (`:176`, client `showObjects.ts`) belong to chunk 16's object surface —
   leave them.
5. Persisted-data caution: show open/rollback/recovery — re-read
   `docs/acceptance-criteria.md`; keep portable-show vs desk-data separation.

## Definition of done

- Show browser, open/save/overwrite, named revisions, MVR transfer all work on v2 routes;
  v1 show routes deleted; bench/tests migrated.
- SHOW-005 (recovery backup) still green.

## Verification

```sh
cargo test -p server -p application
npm run test:unit
npm run test:e2e -- tests/<shows/recovery specs>
npm run test:e2e   # full suite gate
```

Manual: `npm run open` → open, rename, overwrite, download a show; import an MVR preview.

## Decisions

None beyond route naming (follow api-rules; propose in-PR).
