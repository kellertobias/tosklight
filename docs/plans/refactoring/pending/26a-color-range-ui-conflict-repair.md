# 26a — Color-range UI conflicts after Programmer repair

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
