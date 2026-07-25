# Patch Performance Benchmarks

## Goal

Retain end-to-end evidence for the Patch mutation budgets already specified in
`../major-refactoring.md`, rather than relying only on functional large-mode tests.

Estimated effort: 1–2 Codex days.

## Queue dependency

Pending, blocked until plan 02's Patch presentation/adapters and plan 05's client-store warm-up and
visible-paint path are stable. The benchmark must measure the accepted production path rather than
an adapter or component structure still being replaced.

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
