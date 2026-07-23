# 01 — Delete orphaned v1 routes (zero callers)

## Context

Verified 2026-07-23 against the code: these v1 routes have **no callers anywhere** —
not in `apps/control-ui/src` (incl. `api/client/*`), not in `apps/control-ui/e2e/bench`,
not in root `tests/`, not in `apps/hardware-controls` (which talks only over the Tauri
OSC bridge, no HTTP). Pure deletion; no client migration needed.

## Routes to delete (all registered in `crates/server/src/runtime/http_router.rs`)

| Line | Route |
|---|---|
| 34 | `GET /api/v1/health` |
| 36 | `GET /api/v1/version` |
| 90 | `GET /api/v1/media/{fixture_id}/thumbnail` |
| 129 | `POST /api/v1/desk-lock/force-unlock` |
| 131 | `PUT /api/v1/users/{id}` |
| 131 | `DELETE /api/v1/users/{id}` |
| 139 | `DELETE /api/v1/shows/{id}` |
| 187 | `GET /api/v1/qlists/{number}` |
| 189 | `POST\|PUT /api/v1/qlists/{number}/{action}` |
| 204 | `POST\|PUT /api/v1/control-desks/{id}/paged-playbacks/{slot}/{action}` (dead duplicate of the used `page-playbacks` route) |
| 232 | `POST /api/v1/programmer/set` |
| 240 | `GET /api/v1/update/targets` (superseded by v2 `programming-update/targets`) |
| 248 | `GET /api/v1/midi/inputs` |

**Caution:** `/api/v1/health` appears in AGENTS.md's diagnostics guidance
(`curl /api/v1/health` when the app looks stuck). Before deleting it, either keep it
deliberately (note why in the route file) or update AGENTS.md in the same chunk.
`/api/v1/readiness` stays — the bench and `desktop-smoke.mjs` use it.

## Work

1. Per route: re-run the caller grep (`rg -F '<path-fragment>'` across `apps/`, `tests/`,
   `crates/`, `docs/`) to confirm still-zero callers at execution time.
2. Remove the route registration and its now-unreferenced handler(s); delete dead wire
   types and server tests that only exercised the deleted route.
3. `cargo fmt`.

## Definition of done

- All 13 registrations and their orphaned handlers are gone; `cargo build` clean;
  no `#[allow(dead_code)]` left behind to hide leftovers.
- AGENTS.md no longer instructs curling a deleted route.

## Verification

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e   # full suite gate, no net new regressions
```

## Decisions

None — deletion policy is already decided (prerelease, only OSC is frozen).

## Result

- All 13 registrations deleted from `http_router.rs`, with their orphaned handlers:
  `health`/`version` (operator_api), `force_unlock_desk` (boundaries), `update_user`/
  `delete_user`/`midi_inputs` (sessions), `delete_show` (show_library_mutations),
  `media_thumbnail` (media_api), `set_programmer` + private helpers (event_ws),
  `update_targets` + `legacy_menu_entry`/`legacy_update_target`/`legacy_object_target`
  (update_plans). Orphaned wire types removed: runtime `ThumbnailQuery`,
  `UpdateTargetsQuery`, `UpdateMenuResponseEntry`. `ProgrammerSet` stays (WS handler uses
  it); `UpdatePreviewResponse`/`UpdateApiTarget` stay (update preview/apply). One
  now-unused `std::env` import dropped from `runtime.rs`. No `#[allow(dead_code)]` added.
- AGENTS.md, `docs/engineering/build-and-test-commands.md`, `docs/engineering/test-map.md`,
  and `docs/todo-completion-audit.md` no longer instruct curling `/api/v1/health`
  (readiness + bootstrap remain).
- Server test migrations: `security_event_tests` probes the desk boundary via
  `/api/v1/readiness` instead of `/health`; `operational_flow_tests` sets intensity via
  the command line (`FIXTURE 1 AT 50 TIME 0`) instead of legacy `programmer/set`;
  `desk_http_tests` citp thumbnail test asserts the media cache directly instead of the
  deleted GET route. Deleted outright: `programming_interaction_adapter_tests.rs` (whole
  file was legacy programmer/set), the v1 `update_targets_endpoint_keeps_the_legacy_shape`
  test, and the show-deletion steps in `show_overwrite_tests`/`operational_flow_tests`
  (show deletion no longer exists on any surface until v2 lifecycle, chunk 15).
- Surprises: the chunk's caller grep missed `DELETE /api/v1/shows/{id}` uses inside server
  tests because they interpolate (`format!("/api/v1/shows/{source_id}")`) — literal
  `{id}` greps don't catch those. Worth remembering for chunks 13–21.
- Suite numbers: fresh baseline recorded in README (274 passed / 13 skipped / 1 failed —
  the known user-dirty `product-demo` run). After the change: server unit tests
  408 passed; `test:e2e-api` 85 passed / 1 skipped; full `test:e2e` 274 passed /
  13 skipped / 1 failed (same product-demo) — no net new regressions. `npm run test:unit`
  exits 1 both before and after this chunk from pre-existing source-size ratchet
  violations in the dirty worktree (ServerProvider et al.) — not from this change.
- Follow-ups: none filed; no scope discoveries beyond the grep caveat above.
