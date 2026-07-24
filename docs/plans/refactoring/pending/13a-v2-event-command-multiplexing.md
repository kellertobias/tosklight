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
