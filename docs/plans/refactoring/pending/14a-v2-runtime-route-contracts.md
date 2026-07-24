# 14a — Add typed v2 session, bootstrap, readiness, and diagnostics routes

## Context

Chunk 14 spans a public server contract, the production client, bench and root acceptance
callers, operational scripts, desktop smoke, and route deletion. Establish the replacement
server contract first while the v1 routes remain available to keep each commit independently
verifiable.

The v2 Patch snapshot already exists at `GET /api/v2/patch`; this chunk must not add another
Patch read.

## Work

1. Add wire DTOs under `crates/wire/src/v2` for the session-create request/response and
   bootstrap snapshot, reusing existing public DTOs where practical instead of exporting
   runtime-internal structs.
2. Register `GET /api/v2/bootstrap`, `POST /api/v2/sessions`,
   `DELETE /api/v2/sessions/{id}`, `GET /api/v2/readiness`, and
   `GET /api/v2/diagnostics`.
3. Use `TolerantJson<T>` for session creation so unknown fields are accepted and logged
   without values; retain current authentication, recovery, session ownership, and shutdown
   semantics.
4. Add focused route tests for response parity, malformed known fields, tolerated unknown
   fields, legacy/malformed active-show recovery, and the still-present v1 compatibility
   routes.

## Definition of done

- Typed v2 runtime routes exist with focused coverage and no change to persisted desk/show
  formats or startup recovery behavior.
- The production client is not migrated yet and every v1 route remains registered.

## Verification

```sh
cargo fmt --all -- --check
cargo test -p light-server --no-default-features runtime_v2
cargo test -p light-wire --no-default-features
npm run test:unit
```

## Decisions

Inherited from Chunk 14. No open decisions.
