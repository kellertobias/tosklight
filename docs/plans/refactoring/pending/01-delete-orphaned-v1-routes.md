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
