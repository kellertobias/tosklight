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
