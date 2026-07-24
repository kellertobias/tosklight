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
