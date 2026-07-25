# Exclusive Active-show Mutation Boundary

## Goal

Make `ActiveShowService` the only owner of validation, backup/checkpoint, persistence revision,
runtime compilation/installation, undo, and event ordering for active-show mutations.

Estimated effort: 0.4–0.7 Codex day.

## Required work

1. Characterize Stage Layout, User Layout, and Patch Layer direct-write behavior.
2. Add typed application commands for those object families.
3. Route `stage_layout_http.rs` and `commit_direct_object` callers through
   `ActiveShowService::run_prepared_transaction`.
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
