# 11c — Speed Group and output-runtime desk writes onto typed WebSocket actions

## Context

Speed Group and global output-runtime writers subscribe through v2 events and repair
through narrow HTTP snapshots, but still POST mutations. Legacy output `master.set` is
not an exact-revision substitute, and Speed Groups have no complete typed WS twin.

## Work

1. Add correlated typed Speed Group and output-runtime WS actions by reusing their v2
   HTTP request conversion, application service, gating, and outcome conversion.
2. Move only each desk writer's mutation leg to the shared live socket; retain HTTP
   snapshot/event/integrator adapters.
3. Remove identical-request client retries. On any ambiguous failure, repair the narrow
   authority, settle/rollback the optimistic request, report the error, and continue.

## Definition of done

- Speed set/adjust/synchronize and combined Grand Master/Blackout actions use typed WS
  frames with exact authority/show revision and request identity.
- Replay, desk lock, active-show stability, persistence/event counts, and typed outcomes
  match retained HTTP behavior.
- Each writer sends once and repairs after failure.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e
```

## Decisions

None. Execute after 11b.

## Result

- Added correlated `speed_group.action` and `output_runtime.action` WebSocket
  commands that reuse the retained v2 request conversion, exact application
  services, active-show/desk gating, replay cache, persistence, events, and
  typed outcomes.
- Moved the control UI's Speed Group and global-output mutation legs to the
  shared live socket while retaining their HTTP snapshot, event, and
  integrator adapters.
- Removed identical-request retries from both optimistic FIFOs. Every failed
  live send now repairs its narrow authority once, rolls back the pending
  operation, reports the error, and continues without resending.
- Made the touched request envelopes forward-compatible at their top level,
  retained strict nested action validation, and regenerated their JSON
  schemas.
- Verified with `cargo test -p light-server` (442 passed, 1 ignored),
  `npm run test:unit` (277 Vitest files / 1,999 tests plus all Rust workspace
  gates), and `npm run test:e2e` (285 passed, 11 skipped). The Playwright
  worker reached terminal results for all 296 cases but remained alive during
  teardown; terminating that single worker allowed the runner to print its
  successful summary and exit 0.
