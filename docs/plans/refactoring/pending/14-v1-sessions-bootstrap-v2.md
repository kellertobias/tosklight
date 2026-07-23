# 14 — Sessions + bootstrap onto v2; retire the v1 runtime routes

## Context (verified 2026-07-23)

`apps/control-ui/src/api/client/runtime.ts` (base class of `LightApiClient`) still drives:

- `GET /api/v1/bootstrap` (`http_router.rs:38`) — also bench + tests.
- `POST /api/v1/sessions` (`:123`), `DELETE /api/v1/sessions/{id}` (`:124`) — also bench + tests.
- `GET /api/v1/readiness` (`:35`) — bench + `desktop-smoke.mjs` (moves to v2, see below).
- `GET /api/v1/patch` (`:39`) — client `fixtures.ts` (the patch snapshot; v2 patch surface
  exists at `show_patch_http.rs`).
- `GET /api/v1/diagnostics` (`:37`) — root tests only.

Consumers upstream: `features/server/connectionBootstrap.ts`, `useServerState.ts`,
`features/deskSnapshot` scoped store (already reads through the composed provider).

## Work

1. Design the v2 session + bootstrap routes (reads are whole-object snapshots per
   api-rules §1; session create/delete are intent posts). Wire types in `crates/wire`,
   tolerant typing (use chunk 08's helper).
2. Migrate `runtime.ts` internals (or replace with a scoped feature transport following the
   `features/deskSnapshot` pattern) to the v2 routes; migrate bench
   (`lightBench.ts`) and tests.
3. Move the patch snapshot read (`fixtures.ts` → `/api/v1/patch`) onto the v2 patch
   snapshot; migrate its readers.
4. Delete `GET /api/v1/bootstrap`, `POST/DELETE /api/v1/sessions*`, `GET /api/v1/patch`.
   **DECIDED (maintainer, 2026-07-23): readiness and diagnostics move to v2 too** —
   re-register as `GET /api/v2/readiness` and `GET /api/v2/diagnostics`, delete the v1
   forms, and migrate every caller in the same chunk: bench (`lightBench.ts`),
   `desktop-smoke.mjs`, the tests using diagnostics (03-network-output-protocols,
   07-move-in-black, cueSemanticContracts), **and the AGENTS.md verification section**
   (`curl …/api/v1/readiness` and the health/readiness timing guidance) plus any other
   docs quoting the v1 paths (grep `docs/` for `/api/v1/readiness|health|diagnostics`).
5. Startup/recovery paths touch persisted data — re-read `docs/acceptance-criteria.md`
   first; test malformed/legacy active-show recovery after the bootstrap change.

## Definition of done

- Desk boots entirely off v2 bootstrap/session routes; v1 bootstrap/sessions/patch routes
  deleted; bench and tests migrated.
- SHOW-005-style recovery still green.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e-api
npm run test:e2e            # full suite gate
npm run test:desktop-smoke  # desktop boot path uses readiness + bootstrap
```

## Decisions

Decided (2026-07-23): readiness and diagnostics move to `/api/v2` with the rest. No open
decisions remain in this chunk.
