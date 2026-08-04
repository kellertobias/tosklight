---
slug: add-a-capability
title: Add a Capability
components: [backend, control-ui]
order: 30
---

# Add a Capability

The canonical rules are [Architecture Boundaries](02-architecture-boundaries.md) and
[State Ownership](19-state-ownership.md). This page shows the corresponding steps in
shipped code, plus the checklist a slice has to pass.

## Reference slice

Show Objects is the most complete migrated slice:

- `crates/light/src/active_show/`, events in `crates/light/src/event/`
- `crates/light/contracts/wire/src/v2/`
- `crates/light/adapters/headless/src/runtime/` (`*_v2.rs`, `*_http.rs`, `*_wire.rs`)
- `apps/light-desktop/src/features/showObjects/`

## Steps

### 1. Declare the state

Which of the six [lifetimes](glossary:state-lifetimes)? Answer all seven questions: persistence,
migration, reconnect, restart, Save As, deletion. If you cannot, you do not know where the feature
belongs yet.

### 2. Domain types and rules

In the relevant domain crate. No transport types, no `light-wire`, no HTTP.

### 3. Command family and service

`crates/light/src/<capability>/`:

- commands implementing `ApplicationCommand` with their own `type Value`
- an outcome union covering changed, no-change, replay, and conflict
- ports as injected traits, supertraits of `ActiveShowPorts` if you mutate the show
- revision preconditions from `ActionContext`

Do not extend a shared enum, and do not reach for a concrete database or socket.

### 4. Typed events

One semantic transition, one event, whatever the origin. Include sequence, time, source, correlation
identity, and stable identity for snapshot repair.

Ask whether the capability produces automatic transitions. If so, they publish the same event as a
manual command.

### 5. Wire DTOs

`crates/light/contracts/wire/src/v2/<capability>.rs`, deriving `Serialize, Deserialize, JsonSchema, TS`, with
`deny_unknown_fields` on requests. Then:

```sh
cargo run -p light-wire --example generate-contracts
```

Commit the regenerated `light-wire.ts` and schemas. `crates/light/contracts/wire/tests/generated_contracts.rs` fails
if you skip this.

### 6. Server adapter

`crates/light/adapters/headless/src/runtime/<capability>_v2.rs` plus router registration. DTO to command, event to
wire. Nothing else.

### 7. Frontend slice

`apps/light-desktop/src/features/<capability>/`:

| File | Role |
| --- | --- |
| `contracts.ts` | Types and the port the store depends on |
| `transport.ts` | Snapshot fetch, event subscription, strict decoding |
| `store.ts` | External store, revisions, optimistic overlays, reconciliation, gap repair |
| `session.ts` | Reference-counted lifecycle, scope replacement, disposal |
| `*View.tsx` / hooks | What mounted views consume |
| `testFixtures.ts` | Shared test data |

Compose its store/session through the owning feature boundary and activate it only for mounted
views. Shared connection plumbing may be registered through
`features/server/useServerFeatureStores.ts`; capability authority remains feature-local.

## Checklist

Backend:

- [ ] Adapter contains no business rules
- [ ] Command carries `ActionContext` with expected revision
- [ ] No-change publishes and persists nothing
- [ ] Replay is idempotent
- [ ] One semantic transition publishes exactly one event, from every origin
- [ ] Mutations go through `ActiveShowService`; no router writes SQLite
- [ ] Unknown stored fields survive
- [ ] Nothing added to the render tick

Frontend:

- [ ] Unmounted views make no request, open no socket, subscribe to nothing
- [ ] Loading never falls back to stale bootstrap values
- [ ] Optimistic overlay keyed by request identity, with rollback
- [ ] Handles either response/event arrival order
- [ ] Repairs gaps from a snapshot; malformed events fail closed
- [ ] Rejects late work after authority replacement
- [ ] Unrelated global consumers do not rerender
- [ ] Action is immediate, or shows pending/progress/error state

Contracts:

- [ ] Wire DTOs regenerated and committed
- [ ] OSC behaviour unchanged
- [ ] `pairedScenario` coverage for API and UI
- [ ] `npm run test:architecture` passes

## Anti-patterns

| Tempting | Why not |
| --- | --- |
| Add a field to `ServerContextValue` | That facade is being deleted |
| Add a branch to a broad bootstrap refresh | Replaced by narrow snapshot plus subscription |
| Poll for a runtime change | Automatic transitions publish events |
| Put a rule in a v1 WebSocket handler | Behaviour-frozen compatibility code |
| Loop a per-item request over N fixtures | Batch commands are one transaction |
| Extend a shared command enum | Bounded families per capability |
| Import wire DTOs in a component | Fails `npm run test:architecture` |
