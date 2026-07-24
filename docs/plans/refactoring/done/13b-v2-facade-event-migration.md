# 13b — Migrate facade notifications and runtime to v2 events

## Context

After 13a, v2 can carry commands but the broad ServerProvider facade still consumes
legacy `ServerEvent` notifications from the v1 broadcast. Existing typed application
events cover focused stores; remaining facade invalidations and operator notifications
need a v2 application-event representation until the facade is retired in chunk 22.

## Work

1. Inventory the facade-consumed legacy kinds and use existing typed v2 payloads where
   they already own the behavior.
2. Add one explicit v2 facade-notification payload for the remaining snapshot
   invalidations and imperative operator notifications, published through the
   application event bus by the compatibility `emit` boundary.
3. Migrate `LightClientRuntime` to subscribe and command over `/api/v2/events`, decode
   the v2 envelope, and preserve the facade's existing routing behavior.
4. Keep the v1 route temporarily for bench/test callers until 13c.

## Definition of done

- The production control UI opens no `/api/v1/events` socket.
- Commands and all ServerProvider refresh/operator behavior arrive through v2.
- Patch, Show open/close, configuration, command history, file input, and desk actions
  retain live behavior without polling regressions.

## Verification

```sh
cargo test -p light-server --no-default-features
npm run test:unit
npm run test:e2e
```

## Decisions

Inherited from parent chunk 13. The temporary facade payload is removed with
ServerProvider in chunk 22; it is not a new public operator protocol.

## Result

- `LightClientRuntime` now opens `/api/v2/events`, subscribes to the System facade
  stream, and multiplexes its existing typed command requests on that socket.
- The compatibility `emit` boundary publishes lossless `facade_notification`
  application events on a dedicated EventBus. Keeping this temporary stream separate
  prevents compatibility notifications from duplicating authoritative typed events or
  changing their sequence domain.
- The generated v2 wire contract carries the legacy revision, kind, and payload, while
  gaps and errors close the facade socket so the existing reconnect bootstrap repairs
  state.
- Unit coverage proves the draft metadata, wire translation, compatibility publication,
  authenticated v2 delivery, runtime decoding, command multiplexing, and gap recovery.
- Verification passed: the full unit gate (449 server tests plus 14 benchmark tests,
  1 server test ignored, and 2,000 frontend tests) and the full E2E gate (285 passed,
  11 skipped).
