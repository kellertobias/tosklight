# 13c — Remove the v1 event route and broadcast plumbing

## Context

After 13b, only bench and root acceptance helpers use `/api/v1/events`. The server still
retains its separate broadcast sender solely for that route and compatibility-focused
tests.

## Work

1. Migrate bench and root operator-output/playback helpers to subscribe to v2 and decode
   the needed event payloads.
2. Migrate server tests that inspect the compatibility broadcast to application-event
   subscriptions.
3. Delete `/api/v1/events`, its WebSocket handler, the broadcast sender, and the
   now-unused connection plumbing.
4. Audit the repository for every remaining `/api/v1/events` caller or registration.

## Definition of done

- `/api/v1/events` is unregistered and absent from client, bench, and test callers.
- No separate server broadcast sender remains for legacy events.
- Full acceptance retains live Patch, Show lifecycle, configuration, and output updates.

## Verification

```sh
cargo test -p light-server --no-default-features
npm run test:unit
npm run test:e2e
npm run test:e2e
```

Manual: `npm run open`, edit Patch and open another Show; every window updates without
reload.

## Decisions

Inherited from parent chunk 13. No open decisions.

## Result

- Bench and acceptance helpers now subscribe to the multiplexed v2 event stream and decode
  facade notifications from typed event envelopes.
- Server route tests observe the facade `EventBus` directly; the separate Tokio broadcast
  sender, connection counter, v1 handler, and `/api/v1/events` registration are removed.
- The retired route has an explicit `404 Not Found` regression assertion, and repository-wide
  auditing found no remaining caller or registration.
- Verification passed: `cargo fmt --all -- --check`, `npm run test:unit` (including 449
  light-server tests plus 14 benchmark tests and 2,000 frontend tests), the focused v2
  WebSocket/OSC/operator-output acceptance paths, and the full Playwright suite. The full
  Playwright run had one transient OSC output-sampling failure among 284 passes and 11 skips;
  that exact scenario passed immediately in isolation.
