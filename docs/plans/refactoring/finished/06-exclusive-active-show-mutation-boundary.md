# Exclusive Active-show Mutation Boundary

## Status

Finished. This was the lowest-numbered unblocked plan and ran concurrently with the active
Storybook lane because it owned server-side active-show mutation orchestration and backend
verification rather than frontend components, providers, styles, stories, or screenshot runtime.

## Goal

Make `ActiveShowService` the only owner of validation, backup/checkpoint, persistence revision,
runtime compilation/installation, undo, and event ordering for active-show mutations.

Estimated effort: 0.4–0.7 Codex day.

## Required work

1. Characterize Stage Layout, User Layout, and Patch Layer direct-write behavior.
2. Add typed application commands for those object families.
3. Route `stage_layout_http.rs` and `commit_direct_object` callers through the typed
   `ActiveShowService::mutate_objects` entry point, which owns the existing prepared-transaction
   lifecycle (`transact`/`transact_with_unit` internally).
4. Preserve request identity, revision handling, unknown stored fields, checkpoint cadence, and
   exact event ordering.
5. Add an architecture guard forbidding direct active-show `ShowStore` writes outside the service
   adapter and deliberate library/import boundaries.

## Acceptance and verification

- Each operation performs one validation, transaction, runtime replacement, revision, undo step,
  and semantic event.
- Failure before commit leaves persistence and runtime unchanged; failure after durable commit is
  recoverable and observable.
- Existing and legacy shows retain layout and patch-layer data.
- Run service/persistence/concurrency tests, API/UI acceptance, architecture checks, and real
  startup/reload verification.

## Result

### Changes

- Added Stage Layout, User Layout, and Patch Layer as explicit active-show object families across
  the application model, semantic event transport, generated TypeScript contract, and JSON schema.
- Routed Stage Layout moves and the User Layout/Patch Layer intent routes through
  `ActiveShowService::mutate_objects`; removed `commit_direct_object`.
- Preserved Stage Layout server-side fan-out, ordered duplicate suppression, legacy-position
  migration, default patched-fixture placement, request replay, lossless unknown fields, ETags,
  and no-change behavior.
- Preserved User Layout owner scoping and partial updates, Patch Layer path-owned identity, object
  revision conflicts, request replay, and lossless stored extensions.
- Added the optional `X-Tosk-Show` guard and tolerant JSON extraction to the Stage Layout action
  while preserving its object-intent POST transport.
- Added an architecture guard that permits direct portable-show writes only at the active-show
  unit of work and documented library, import, migration, inactive-show, and isolated test-seed
  boundaries.

The shared service now owns candidate/runtime preparation, checkpoint cadence, the portable WAL
transaction, cached-document revision, runtime installation, undo history, and semantic event
ordering for all three families. Runtime preparation is fallible before the durable commit;
installation and completion are deliberately infallible afterward, so there is no new
post-durable failure window to recover.

### Tests

- `cargo check -p light-application -p light-headless-runtime`
- `cargo test -p light-application --no-default-features active_show` — 50 passed
- `cargo test -p light-headless-runtime --no-default-features stage_layout_route_tests` — 4 passed
- `cargo test -p light-headless-runtime --no-default-features show_object_intents_v2_route_tests`
  — 3 passed
- `cargo test -p light-headless-runtime --no-default-features
  active_show_document_cache_reuses` — 1 passed
- `cargo test -p light-wire` — 87 tests passed, including generated-artifact freshness
- `npm run test:architecture` — passed
- `npm run test:e2e-api` — 23 passed
- `npm run test:e2e -- --grep PRELOAD-003` — 1 passed, including persisted User Layout and restart
- The desktop TypeScript/Vite build passed as part of both E2E commands.

### Limitations

- `npm run test:unit` reached 161/161 bench tests and then failed in the concurrently edited
  Storybook lane because Vitest collected
  `apps/ui-library/storybook/tests/ui-stories.spec.ts`; 18 UI-library suites and 85 tests passed
  before Playwright rejected `test.describe.configure` under Vitest. No plan-06 file participates
  in that failure.
- `npm run open` was not used because it would validate the concurrently changing frontend bundle,
  not this backend-only boundary. The focused UI/restart scenario and full API E2E suite each built
  the desktop, launched the real headless server, and verified persistence/runtime behavior.
- The wire generator continues to print its pre-existing `ts-rs` warnings for
  `deny_unknown_fields`; generated-contract freshness passes.

### Commit

`feat(show): centralize active show mutations` (this implementation and plan move).
