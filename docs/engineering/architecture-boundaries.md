# Architecture Boundaries

This document is the dependency and state-ownership contract for the [major architecture refactor](../plans/major-refactoring.md). It describes rules that new modules must satisfy while the compatibility adapters are still being removed.

Start with the [architecture overview](architecture-overview.md) for the end-to-end system shape,
then use this document as the enforceable boundary contract.

## Dependency direction

Dependencies point inward from transports to use cases and domains:

```text
control-ui / HTTP / WebSocket / OSC / MIDI / Matter adapters
                              |
                              v
                     light-server composition
                      /                   \
                     v                     v
            light-application          light-wire
                     |
                     v
       domain crates and application-owned ports
```

- `light-wire` owns versioned serialized request, response, outcome, error, event, and subscription DTOs. It has no dependency on another workspace crate and contains no application behavior.
- `light-application` owns use cases, action context, application events, event delivery policy, and ports. It may depend on domain crates, but never on `light-wire`, `light-server`, a desktop host, or a concrete transport.
- Domain crates under `crates/` do not depend on `light-application`, `light-wire`, `light-server`, or a desktop host. They expose stable domain models, commands, queries, and immutable projections.
- `light-server` is the composition root. Its feature adapters authenticate, decode, normalize, invoke one application use case, and translate the typed result. Business rules do not belong in routers or transport callbacks.
- Generated TypeScript wire DTOs are transport-boundary types. Frontend feature and view models must not become aliases for serialized DTOs; the API layer validates and maps decoded data before feature code consumes it.

These rules are checked by `./test architecture`. An intentional boundary change must update this contract and the checker in the same commit.

## Action context

Every application mutation carries an `ActionContext` with stable desk, user, session, source-surface, correlation, and request identities plus the expected revision when the target is revisioned. Adapters may derive this context from HTTP authentication, the shared desk OSC session, attached hardware, a scheduler, or a future Macro host. They may not invent a second ownership model for the use case.

One semantic action has one authoritative outcome and publishes each semantic event once. Compatibility adapters may translate that outcome, but may not repeat the mutation or synthesize competing state.

## State lifetimes

The complete six-lifetime matrix is in [State ownership](state-ownership.md). Every new field must
name one lifetime and document persistence, migration, reconnect, restart, Save As, and deletion.
Storage proximity does not merge lifetimes: for example, desk-interaction command text may be
checkpointed beside a user Programmer while retaining separate ownership and reset rules.

Unknown portable show objects and fields survive load, Save As, revision creation, export, and
selective import. A migration may canonicalize a known representation, but it may not silently
erase data it does not own.

## Concurrency and performance

- A service owns its mutable state and lock ordering. Callers receive commands and immutable projections, not mutexes or registries.
- A user action crosses the application boundary once. Batch size must not become a loop of persistence, compilation, full-state refresh, or transport round trips.
- Timing-critical render and output work consumes immutable snapshots or bounded contribution batches. It does not serialize JSON, wait for clients, read fixture-library storage, persist state, or publish through an unbounded queue.
- Event delivery is bounded. Discrete transitions, errors, safety state, and command outcomes are lossless within the subscriber contract; replaceable telemetry may coalesce to its newest value.
- A sequence gap makes incremental state insufficient. The subscriber requests the named authoritative projection, installs that snapshot and revision, then resumes events after the snapshot cursor.

## Compatibility adapters

REST/WebSocket v1 and current string events remain temporary adapters while their callers migrate. Each compatibility path must call the same application service as its replacement boundary. New use cases are added as bounded application command families and typed events; they are not added to the legacy string command or generic JSON event mechanisms.
