# Patch Performance Benchmarks

## Goal

Retain end-to-end evidence for the Patch mutation budgets already specified in
`../major-refactoring.md`, rather than relying only on functional large-mode tests.

Estimated effort: 1–2 Codex days.

## Queue dependency

Doing. Plan 02's Patch presentation/adapters and plan 05's client-store warm-up and visible-paint
path are stable. The benchmark measures the accepted production path rather than an adapter or
component structure still being replaced.

## Required work

1. Add warmed single-fixture and 100-fixture `PatchFixtures` benchmarks through the real
   Show Patch/active-show service boundary.
2. Record request/response bytes and validation, conflict detection, backup/checkpoint,
   persistence, compile, runtime install, event publication, client store update, and visible paint
   phases.
3. Enforce server p95 budgets of 250 ms for one fixture and 500 ms for 100 fixtures on the agreed
   release profile.
4. Add informational action-to-visible-UI p50/p95 first; promote a stable 500 ms p95 gate only
   after sufficient runner history.
5. Publish Patch results in the existing post-release performance report and Pages details.

## Acceptance and verification

- One batch causes one show transaction, backup decision, compile, runtime replacement, and event.
- No fixture-library, configuration, shows, media, or unrelated desk refresh occurs.
- The benchmark uses the released artifact and retains raw phase evidence.
- Functional 2,000-mode coverage and atomic rollback/conflict tests remain green.

## Result

Completed on 2026-07-26.

### Changes

- Added the release-only `--patch-gate` probe alongside the existing render and incremental-show
  probes. It warms and records 40 single-fixture and 40 100-fixture samples against a shared
  2,000-mode profile.
- The measured server boundary now uses the production Patch request/outcome DTO conversion,
  `ShowPatchService`, the active-show snapshot and transaction lifecycle, a real SQLite show,
  safety-backup copies, full candidate compilation, engine preparation and installation,
  reconciliation, and event publication. Raw totals, per-phase distributions, request/response
  bytes, and boundary counters are retained in schema-v4 JSON.
- Enforced release-profile server p95 budgets of 250 ms for one fixture and 500 ms for 100
  fixtures. Each measured sample asserts one transaction, backup decision, persistence commit,
  runtime preparation/replacement, reconciliation, and event, plus the expected separate
  authoritative snapshot boundary and zero unrelated reads.
- Added Patch frontend diagnostics for optimistic store publication, decoded response,
  authoritative store publication, and the following visible paint. The snapshot exposes
  informational action-to-visible p50/p95 with `gateEnforced: false`.
- Added focused browser acceptance coverage that performs 30 warmed operator Patch edits,
  verifies one `POST /api/v2/patch/fixtures` per action, rejects unrelated fixture-library,
  configuration, shows, and media reads, and retains the informational timing evidence.
- Extended the post-release status, durable report bundle, summary, and Pages output with Patch
  server results and a dedicated performance details page. A missing Patch probe is now
  `unknown`; a failed Patch gate is `degraded`.

### Verification

- `cargo test -p light-application show_patch --lib` — 31 passed, including shared 2,000-mode,
  atomicity, replay, rollback, and conflict coverage.
- `cargo test -p light-headless-runtime show_patch --lib --no-default-features` — 7 passed against
  the real HTTP/adapter routes.
- `cargo test -p light-headless --bin light-benchmark --no-default-features` — 15 passed.
- `npm test --workspace @tosklight/light-desktop -- --run
  src/features/frontendWarmup/diagnostics.test.ts src/features/patch/PatchFeature.test.ts` — 29
  passed.
- `node --test tools/run-release-performance.test.mjs` — 4 passed.
- `npm run test:e2e -- tests/64-patch-performance.spec.ts` — 1 passed after rerunning outside the
  filesystem/network sandbox so Playwright could bind loopback.
- `cargo build --release --locked --no-default-features -p light-headless --bin light-benchmark`
  and a released-binary `--patch-gate` run — passed. The retained local report measured 48.885 ms
  p95 for one fixture and 61.521 ms p95 for 100 fixtures; both gates passed. The 100-fixture
  request/response path also retained its approximately 52.9 KB request and 1.2 MB response size.
- The complete local post-release driver produced and retained Patch evidence successfully. Its
  overall status was `degraded` only because this local macOS run missed the separate
  32-universe/100 Hz render floor; both Patch gates were green.
- The ordinary desktop typecheck initially observed unrelated concurrent UI-library errors, but
  the focused E2E command subsequently completed `tsc --noEmit` and the production Vite build
  successfully after those concurrent files settled.

### Limitations

- UI p50/p95 remains informational by design and is produced by the focused browser run; the
  headless release probe cannot manufacture a browser paint. The compact release status therefore
  labels it informational and does not enforce the proposed 500 ms p95 until runner history is
  sufficient.
- HTTP authentication and Axum scheduling remain outside the released server probe. The probe
  does include the exact production Patch JSON DTO decode/encode and application boundary.
- The local render-floor result is not release-runner evidence. The post-release workflow reruns
  the same released executable on its declared GitHub release-runner profile and publishes that
  machine metadata.

### Commit

- `feat(performance): retain real Patch mutation budgets`
