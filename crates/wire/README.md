# light-wire

`light-wire` owns versioned serialized contracts. It deliberately has no dependency on a Light
domain or application crate: HTTP and WebSocket adapters translate between these DTOs and typed
application actions.

The Rust DTOs are the source of truth. Regenerate the checked-in JSON Schemas and TypeScript
bindings after changing them:

```sh
cargo run -p light-wire --example generate-contracts
```

`cargo test -p light-wire` fails when a generated artifact is missing or stale. Request schemas
describe deserialization; response and event schemas describe serialization, so optional fields
match the actual direction in which each contract crosses the boundary.

The v2 event socket is `/api/v2/events`. Offer `light.events.v2` (or `light.v2`) together with the
existing `light.token.<session token>` subprotocol, then send one `EventClientMessage::Subscribe`.
The server derives the desk from that session; clients cannot subscribe as another desk. Capacity
is bounded, filters are explicit, and optional rate limits apply only to replaceable projection or
telemetry topics. A `gap` requires an authoritative `/api/v2/events/playback-snapshot` read followed
by `EventClientMessage::Repair` with the returned cursor before incremental delivery resumes.
