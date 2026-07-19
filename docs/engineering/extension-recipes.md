# Extension Recipes

These recipes extend the current architecture without reopening global ownership. They complement
[Architecture boundaries](architecture-boundaries.md) and the six-lifetime
[state matrix](state-ownership.md).

Dynamics, Macros, Timecode, and virtual external fixtures remain gated future plans. The existing
fake services and ports prove seams only. Do not create product schemas, commands, UI, or persisted
objects until the caution gate in the relevant file under `docs/plans/Later/` is removed and its
behavior is decision-complete.

## Add a typed capability slice

Use this sequence for a current, specified capability. Keep the change vertical: command, service,
wire, adapter, event, snapshot, store, view, and tests land together.

1. **Declare ownership.** Add the field's lifetime declaration using the template in
   [State ownership](state-ownership.md). Split portable definitions, desk configuration,
   interaction state, user Programmer content, connection state, and transient runtime into
   separate models where necessary.
2. **Choose the domain boundary.** Put stable semantic models and pure rules in the owning domain
   crate. A domain crate must not import `light-application`, `light-wire`, HTTP, WebSocket, Tauri,
   or a concrete store/driver.
3. **Add one bounded application service.** Create or extend a focused module under
   `crates/application/src/<capability>/`. Define typed commands, immutable projections, results,
   errors, and dependency-injected ports. Carry `ActionContext`; use `expected_revision` and
   `request_id` where the operation can conflict or retry. The service, not an adapter, owns
   ordering and idempotency.
4. **Use the correct persistence path.** Portable writes prepare a lossless
   `PortableShowTransaction` and pass through `ActiveShowService`. Desk-installation fields use a
   typed `DeskStore` operation and desk-schema migration. Runtime-only fields do not acquire a
   database merely for convenience.
5. **Publish a semantic event once.** Extend `crates/application/src/event/model.rs` with a bounded
   payload and stable `EventObject` route. Choose `Lossless` for discrete transitions, safety,
   errors, and outcomes; use `Replaceable` only for a current projection or telemetry. Publish
   after authoritative state changes and outside timing-critical domain locks.
6. **Define the wire DTO.** Add versioned request, outcome, error, projection, snapshot, and event
   DTOs under `crates/wire/src/v2/`. Register every checked-in TypeScript/schema artifact in
   `crates/wire/src/generation.rs`, then run:

   ```sh
   cargo run -p light-wire --example generate-contracts
   cargo test -p light-wire --no-default-features
   ```

7. **Write a thin server adapter.** Add a feature router under `crates/server/src/runtime/`, compose
   it in `crates/server/src/runtime/http_router.rs`, authenticate and normalize addressing, map DTOs
   to application types, invoke one service, and map the result back. Add the event conversion in
   `crates/server/src/runtime/event_transport/adapter.rs` or a focused submodule. No business rule
   or second state machine belongs here.
8. **Provide authoritative repair.** A subscribed projection needs a snapshot containing the full
   requested scope and an event cursor captured at a coherent boundary. Define stable subscription
   identities by capability and object, not by component name or URL. A gap is repaired by
   installing the snapshot first and acknowledging that exact cursor second.
9. **Build the frontend boundary.** Generated wire DTOs may be imported only below
   `apps/control-ui/src/api/`. Decode untrusted JSON there and map it to feature contracts. Under
   `apps/control-ui/src/features/<capability>/`, separate `contracts`, `store`, `session`,
   `transport`, and React view/hooks. Follow `features/showObjects/` and
   `features/playbackRuntime/` for scoped hydration and `features/patch/` for optimistic mutation.
10. **Make visibility own subscription.** A mounted view activates only the capability/object IDs it
    displays and releases them on unmount or selection change. Select a narrow immutable store
    projection so unrelated events do not rerender consumers. Do not add idle provider polling or a
    broad `refresh()` call.
11. **Reconcile optimistic work.** Keep pending overlays apart from authoritative state. Correlate
    command responses/events by request identity, event cursor, and object revision; ignore stale
    responses; roll back on failure; expose pending and actionable error state.
12. **Retire the old path.** Migrate production callers and public tests, retain deliberately named
    compatibility coverage, then remove the feature's v1/string-event/`useServer()` branch. Do not
    keep both orchestration paths as permanent alternatives.

Run `./test architecture` throughout. Finish with the focused commands from the
[test map](test-map.md) and the exact operator surfaces named by the acceptance contract.

## Add an event and view-scoped store to an existing capability

When commands already converge but clients still poll, the minimum safe slice is:

1. Define an immutable application projection and meaningful semantic transition.
2. Add an `EventObject` constructor with stable domain identity, such as `playback:{number}` or
   `objects:{show}:kind:{kind}:object:{id}`; add related routes when one event is relevant through
   more than one identity.
3. Publish one event from the owning service for manual and automatic origins alike.
4. Add the event payload and snapshot DTO to `light-wire` and translate it in the server event
   adapter.
5. Capture snapshot and cursor coherently. Never return “latest data” with an unrelated cursor.
6. At the frontend API boundary, reject malformed envelopes before advancing a cursor.
7. In the feature session, ref-count active view scopes, hydrate those scopes, subscribe after the
   snapshot cursor, queue events that overlap hydration, and perform snapshot-first gap repair.
8. In the store, reject wrong-show/wrong-desk events and older revisions or sequences. Publish a
   new snapshot only when selected state changed.
9. Add network and render tests proving that an inactive view has no subscription/poll, unrelated
   topics are ignored, a relevant event becomes visible without a broad GET, and a gap repairs.
10. Remove the replaced interval and event-triggered broad refresh from
    `apps/control-ui/src/features/server/`.

## Add a portable object to Selective Show Import

Do not create a feature-specific copy endpoint.

1. Define the object's stable key and every owned identity/reference location in
   `crates/application/src/selective_import/references/`. The current registry is the
   `RegisteredObjectKind` match in `references/mod.rs`; add a schema-specific descriptor in
   `references/descriptors.rs` or its own focused module.
2. List fixture-profile and managed-asset references explicitly. Never guess dependencies by
   scanning arbitrary field names.
3. Ensure duplicate identity rewriting covers object keys, scalar IDs, map keys, arrays, and nested
   references owned by the schema.
4. Add preview tests for dependency closure, identical objects, missing references, each conflict
   resolution, duplicate rewrite, unknown fields, source/target revision races, and atomic failure.
5. Add the kind to the catalog/modal presentation and targeted event/store reconciliation.

The generic prototype descriptors for the strings `dynamic`, `macro`, and `timecode` are not a
production schema contract. Replace them with exact descriptors only after those product schemas
are specified.

## Future Dynamic implementation seam

The gate in [`docs/plans/Later/02-dynamics.md`](../plans/Later/02-dynamics.md) currently forbids
implementation. Once it is explicitly made implementable, use these boundaries:

- Store the definition as a typed portable object with a schema-specific selective-import
  descriptor; do not add Dynamic fields to fixture, Group, Cue, or transport DTOs as raw JSON.
- Keep phase, pause, restart, hidden/suppressed, and per-Playback instance state in a supervised
  runtime service outside `light-engine` and the output scheduler.
- Resolve each fixture/logical-head and attribute lane independently. The runtime samples a finite
  immutable `ContributionBatch` using `crates/engine/src/contribution_batch.rs`; it does not write
  DMX or call a protocol driver.
- Submit Programmer, Preload, and Cue assignments through their existing semantic contribution
  boundaries. Static/fixed competition uses ordinary priority and ownership, not special cases in
  Groups, Cues, fixture projection, or output.
- Publish definition and runtime projections through a bounded Dynamic capability and subscribe
  only to the selected definition/instances. High-frequency phase display must be an explicit
  bounded telemetry stream or client interpolation, never one event per DMX sample.

The architecture proof is in `crates/engine/src/tests/contribution_batches.rs`: fake stateful,
two-attribute, and fixed sources use ordinary arbitration. It deliberately defines no production
Dynamic behavior.

## Future Macro implementation seam

The gate in [`docs/plans/Later/46-macros-and-scheduled-macros.md`](../plans/Later/46-macros-and-scheduled-macros.md)
currently forbids a Macro product. Once language, sandbox, capabilities, persistence, lifecycle,
interaction, scheduling, and acceptance behavior are specified:

- Implement the selected language behind `MacroRuntime` in
  `crates/application/src/macro_runtime/service.rs`; keep the language adapter outside domain and
  render crates.
- Expose only the capability-scoped `MacroHost`. Its backend authorizes queries, typed application
  actions, event waits, operator input, and audited HTTP. A runtime must not receive database,
  socket, process, filesystem, environment, or engine-lock access.
- Run instances through `MacroService` and `MacroTaskRunner` so invocation is supervised,
  cancellable, observable, and outside request/output threads. Preserve trusted source,
  correlation, and request identities on every host action.
- Persist definitions and schedules as separate portable objects with explicit dependencies and
  selective-import descriptors. Runtime instances and waits are transient unless a future spec
  defines a safe checkpoint.
- Use `crates/application/src/scheduling/` to keep monotonic waits distinct from wall-clock schedule
  metadata. Timezone, occurrence identity, skip/catch-up, and duplicate prevention remain product
  policy, not generic scheduler guesses.

The tests under `crates/application/src/macro_runtime/tests/` and `scheduling/tests.rs` are fake
extension proofs. They do not select a language or authorize a production host API.

## Future external-device adapter seam

The product behavior in [`docs/plans/Later/05-virtual-output-fixtures.md`](../plans/Later/05-virtual-output-fixtures.md)
is not yet implementable. After fixture bindings, desired/observed authority, credentials,
offline/retry behavior, and acceptance criteria are specified:

1. Model the portable fixture binding separately from desk-installation credentials/configuration
   and transient connection/health state.
2. Extend compiled fixture output with semantic `ExternalDeviceIntent` values; never translate an
   external fixture into pretend DMX slots.
3. Implement `ExternalDeviceAdapter` from `crates/output/src/external.rs` in a concrete adapter
   layer. Group immutable intents by adapter and revision, schedule application outside the render
   loop, and keep retry/backpressure from blocking DMX.
4. Keep desired desk state and `ExternalDeviceObservation` separate until the product spec chooses
   authority and conflict behavior. Observed feedback may update a scoped runtime projection, but
   must not silently overwrite Programmer or Cue data.
5. Publish health, failure, reconnect, and observed-state events with bounded queues and explicit
   replaceable/lossless policy. Shut adapters down through the process cancellation lifecycle.
6. Test with a fake adapter that DMX output is byte-identical, intents are routed only to the named
   adapter, retry/offline behavior is bounded, and observed values cannot feed back into desired
   state accidentally.

The fake in `crates/output/src/external.rs` proves only the desired/observed/DMX separation.
