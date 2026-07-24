# 13a — Multiplex typed commands on the v2 event WebSocket

## Context

The facade runtime still sends typed live-control command envelopes over the v1 event
socket. The v2 event socket currently accepts only subscribe and repair messages, so it
cannot replace v1 until it can dispatch the existing authenticated command channel.

## Work

1. Extend the v2 socket client-message boundary to distinguish event control messages
   from the existing typed command envelope.
2. Require the first frame to remain an event subscription, then accept repair and
   command frames on the live socket.
3. Return command responses without weakening v2 event filtering, authentication, gap,
   or repair semantics.

## Definition of done

- `/api/v2/events` carries the existing typed command request/response channel after
  subscription.
- Invalid command envelopes return a correlated command failure when possible and never
  terminate filtered event delivery.
- Existing event subscription and command identity tests remain green.

## Verification

```sh
cargo test -p light-server --no-default-features event_transport
cargo test -p light-server --no-default-features websocket_commands
npm run test:unit
```

## Decisions

Inherited from parent chunk 13. No open decisions.

## Result

- The v2 event socket now distinguishes subscription/repair frames from the existing
  typed command envelope after the required initial subscription.
- Command frames dispatch through the same authenticated command boundary as v1 and
  return the existing correlated response shape; malformed correlated commands retain
  their request id, while malformed uncorrelated frames return a v2 event error.
- Focused coverage proves a command executes on an active subscription and filtered
  event delivery continues afterward.
- Verification passed: 31 focused event-transport tests, the focused WebSocket command
  ownership test, 447 server tests plus 14 benchmark tests (1 server test ignored), and
  1,999 frontend unit tests.
