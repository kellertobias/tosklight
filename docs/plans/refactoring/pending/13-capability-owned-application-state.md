# Capability-owned Application State

## Goal

Replace the large raw-lock `AppState` container with capability-owned runtime resources and narrow
service/query ports. This is the deliberate wholesale completion of the state-ownership refactor,
performed capability by capability rather than as one untestable rewrite.

Estimated effort: 1–2 Codex days after plans 06 and 07 establish mutation and event owners.

## Required work

1. Inventory every `AppState` field, lock, owner, mutation path, query path, lifecycle, and
   cross-capability dependency.
2. Define capability resources for active show, Programmer, Playback, Highlight, output,
   configuration, fixtures, sessions, media, events, and replay state.
3. Expose commands and immutable projections; do not expose raw `Mutex`, `RwLock`, registries, or
   stores to adapters.
4. Move lifecycle/startup/shutdown ownership into each capability and keep only a small composition
   root that wires ports together.
5. Migrate one capability at a time with temporary accessors that can only shrink.
6. Add an architecture ratchet preventing new raw state/lock access and remove `desk_lock()`
   compatibility access once callers migrate.
7. Verify lock ordering, cancellation, shutdown, event backpressure, and no lock held across
   `.await`.

## Acceptance and verification

- HTTP, WebSocket, OSC, persistence, and background adapters depend on capability ports.
- One capability's tests can instantiate its resource without the whole server state bag.
- The composition root contains wiring, not business operations.
- Concurrency, deadlock-sensitive, startup/shutdown, API, OSC, event, architecture, and full
  workspace tests pass.
