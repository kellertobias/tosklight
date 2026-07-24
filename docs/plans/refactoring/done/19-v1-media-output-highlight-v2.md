# 19 — Media, visualization, DMX diagnostics, highlight onto v2; retire their v1 routes

## Context (verified 2026-07-23)

Two v1 client modules remain here:

- `api/client/media.ts` (via `features/mediaServers`, `features/dmxDiagnostics`,
  `VisualizationRuntimeTransport.ts`): `GET /api/v1/visualization` (`http_router.rs:84`),
  `GET /api/v1/media` (`:85`), thumbnails/preview refresh (`:87,:92`), preview blob
  (`:96`), `GET /api/v1/dmx` (`:99`, also bench + tests).
- `api/client/output.ts` (via `features/dmxDiagnostics`, `features/highlight`):
  `PUT /api/v1/dmx/override` (`:100`), `GET /api/v1/highlight` (`:241`),
  `POST /api/v1/highlight/action` (`:242`, also tests),
  `PUT /api/v1/patch-preview-highlight` (`:244`).

Classification per api-rules §1: highlight actions and DMX override are live control
(→ WS for desk UI + HTTP action form, see chunks 09–11 pattern); visualization/media/dmx
reads are snapshots; thumbnail/preview refreshes are fire-and-forget intent posts.
v2 output runtime already exists (`runtime/output_runtime_v2.rs`) — extend it rather than
inventing a parallel family.

## Work

1. Move DMX read + override and highlight state/actions onto/next to the v2 output-runtime
   surface; desk UI sends highlight + override as WS frames (pattern from 09), HTTP forms
   remain for integrators/bench.
2. v2 snapshot routes for visualization + media lists; refresh intents; preview blob GET
   under v2.
3. Migrate `features/mediaServers`, `features/dmxDiagnostics`, `features/highlight`,
   `VisualizationRuntimeTransport`, DmxWindow, ProductDemoApp callers, bench + tests.
4. Delete the listed v1 routes with caller greps.

## Definition of done

- media.ts and output.ts are deleted (no v1 paths left in them); the listed v1 routes are
  gone; DMX monitor, media previews, highlight (incl. patch-preview highlight from
  PatchWindow) all work.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e -- tests/03-network-output-protocols.spec.ts
npm run test:e2e   # full suite gate — DMX + highlight + product-demo scenarios
```

## Decisions

None.

## Execution

Claimed 2026-07-24; no maintainer decision is open.

## Result

Completed 2026-07-24.

- Replaced the two remaining v1 client modules with one typed
  `MediaOutputApiClient`; visualization, media-server, preview, and DMX snapshots now use
  v2 routes, while DMX override, Highlight, and Patch Preview Highlight use one correlated
  WebSocket frame from the desk UI.
- Added shared Rust/TypeScript wire DTOs for the touched actions and media refresh intents.
  HTTP integrator forms use tolerant typed decoding, optional Show/Desk context guards, and
  the same transport-neutral mutation helpers as WebSocket.
- Preserved Highlight's existing authenticated status reconciliation, ownership, selection,
  OSC feedback, and Patch Preview cleanup semantics. The surviving snapshot polling remains
  assigned to pending Chunk 22's explicit polling-to-events migration.
- Migrated the visualization runtime transport, DMX window/product demo callers, bench, root
  acceptance helpers, and WebSocket-aware Highlight error coverage. All listed v1 routes are
  absent and covered by a 404 regression test.

Verification:

- `cargo test -p light-server --no-default-features`: 475 passed, 1 ignored, plus 14
  benchmark tests.
- `npm run test:unit`: passed, including frontend build, workspace Rust tests, generated
  contracts, and UI unit tests.
- `npm run test:e2e -- tests/03-network-output-protocols.spec.ts`: passed with the two
  documented DMX-008 skips.
- Highlight ownership, WebSocket UI, and WebSocket error-presentation focused reruns passed.
- `npm run test:e2e -- --reporter=dot`: 285 passed, 11 skipped.
- `npm run test:architecture`, `cargo fmt --all -- --check`,
  `cargo test -p light-wire --test generated_contracts`, and `git diff --check`: passed.
