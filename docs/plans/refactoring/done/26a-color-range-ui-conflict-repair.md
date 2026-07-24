# 26a — Color-range UI conflicts after Programmer repair

Status: complete.

## Context

Chunk 24's full UI and E2E gates repeatedly exposed one focused failure outside the loading
paths: `COLOR-RANGE-001 @ui` reaches the Color Special Dialog, then reports
`Programmer values outcome still conflicts after repair` and never installs the expected
per-fixture assignments. The paired API scenario passes.

Observed on 2026-07-24 in:

- `npm run test:e2e-ui`: 103 passed / 5 skipped / 1 failed;
- isolated `tests/26-color-special-dialog-alignment.spec.ts`: API passed, UI failed;
- `npm run test:e2e`: 286 passed / 9 skipped / 1 failed.

The failure reproduced three times. Chunk 24 does not touch the Color dialog, Programming
transport, or repair policy, so expanding a loading-state refactor into that value-write path
would mix unrelated behavior and reduce reviewability.

A later full gate on 2026-07-24 passed `COLOR-RANGE-001 @ui` and finished at
287 passed / 9 skipped. Treat the defect as intermittent and preserve both the failing
conflict message and the later green run when diagnosing it.

## Work

1. Reproduce the software-to-hardware Color range transition in
   `tests/26-color-special-dialog-alignment.spec.ts`.
2. Trace the request identity, expected Programmer authority, conflict response, snapshot
   repair, and retry through the production UI transport.
3. Determine whether the conflict is caused by stale test authority, a missed post-repair
   retry, or a real cross-surface write race.
4. Fix the production or bench boundary that owns the defect without weakening the
   authoritative assignment assertions.

## Verification

```sh
npm run test:e2e -- tests/26-color-special-dialog-alignment.spec.ts
npm run test:e2e-ui
npm run test:e2e
```

## Outcome

The UI did not encounter a real cross-surface write race. The command facade and event
transport serialized the same Rust `f32` authority with different JSON number spellings:
for example, `0.3400000333786011` in the command outcome and `0.34000003` in the event
projection. Exact JavaScript number comparison treated those projections as divergent,
requested two unnecessary repairs, and displayed the conflict status. That status changed
the dialog layout during the pointer gesture, so the final saturation coordinate was wrong.

Projection comparison now recognizes non-integer values with the same `f32` representation
as equal while revisions, ordering, timing, and other integers remain exact. Store and writer
tests cover event-first compact values paired with widened command outcomes.

Evidence gathered on 2026-07-24:

- pre-fix reproduction: 4 of 10 UI repetitions failed while all 10 API repetitions passed;
- post-fix focused unit coverage: 35 tests passed;
- post-fix focused E2E repetition: 20 passed, covering 10 API and 10 UI repetitions;
- `npm run test:unit`: all checks passed except the sandbox-blocked CITP loopback tests,
  which passed separately with local socket access (5 passed);
- `npm run test:e2e-ui`: 104 passed / 5 skipped;
- `npm run test:e2e`: 287 passed / 9 skipped.
