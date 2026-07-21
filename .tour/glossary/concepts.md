## One action one authority

An operator can set a fader from six places: the software UI, the keypad, the
[command line](glossary:command-line), attached OSC hardware, an HTTP request, or a future macro.
All six must produce **exactly one** [typed command](glossary:typed-command), processed by exactly
one [application service](glossary:application-service), producing exactly one
[domain event](glossary:domain-event).

Before the refactor, six surfaces each implemented the same orchestration. Every bug had to be fixed
six times, and they drifted.

## Typed command

A bounded, named request to change state — `ProgrammerCommand`, `PlaybackCommand`, `ShowCommand`,
`DeskCommand`, `OutputCommand`.

A new capability adds its own command family rather than extending a shared enum or adding a
string-plus-JSON message. Serialization belongs only in transport adapters.

Defined in `crates/application/src/action.rs`.

## Action context

Carried by every action. It answers *who, where, and against what*: desk, user, session,
**source surface**, correlation or request identity, and the applicable expected
[revision](glossary:revision).

Source surface drives feedback and audit. Correlation identity is how an optimistic frontend matches
a response and an event to the write it issued. Expected revision is how concurrent writers are
rejected rather than silently overwriting each other.

## Adapter

The thin layer between a transport and the application. It may parse and validate untrusted input,
authenticate, normalise addressing, and translate DTOs to commands and events to wire or OSC
feedback.

It may **not** implement business rules. Writing "if the fixture is unpatched then…" in an adapter
means the logic belongs in the service.

## Application service

A transport-independent use-case owner in `crates/application/`.

Services own their state and locks, exposing [commands](glossary:typed-command) and immutable
[projections](glossary:projection) — never their underlying mutexes or registries. They may depend
on domain crates, but never on `light-wire`, HTTP, WebSocket, Tauri, or a concrete database or
network adapter.

## Outcome

What a command returns. An explicit union, not a boolean, distinguishing **changed**, **no-change**,
**replay**, and **conflict**, plus the authoritative resulting projection and revision, and an event
sequence *only when an event was actually emitted*.

"No-change" is first class: it must not publish, must not persist, and must not fabricate an event.

## Dependency direction

Adapters depend on the application layer, which depends on domain crates. `light-wire` is a leaf
depending on nothing. Domain crates never depend on `light-application`, `light-wire`, or
`light-server`.

Enforced in CI by `tools/check-architecture.mjs` against `cargo metadata` — not by convention.

## State lifetimes

Six lifetimes exist, and every piece of state belongs to exactly one: **portable show**,
**desk installation**, **desk interaction**, **user Programmer**, **connection or session**, and
**transient runtime**.

Before adding a field you must answer seven questions: which lifetime, where persisted, migration
policy, reconnect behaviour, restart behaviour, Save As behaviour, deletion behaviour. If you cannot
answer one, you do not yet know where the field belongs.

Common mistakes: command-line text is desk interaction, not show; Programmer values are per-user,
not per-desk; [Highlight](glossary:highlight) and cue ambiguity are transient runtime.

## Projection

A read-only, **authoritative** view derived from domain state. Services hand out projections, never
their mutable internals.

UI and OSC feedback derive from projections, never from client-local approximations of what the
client thinks it just did.

## Snapshot

A complete [projection](glossary:projection) at a point in time, fetched over HTTP with a
[revision](glossary:revision) or cursor. A client hydrates from a snapshot, then follows events
incrementally.

Hydration is independent of WebSocket readiness, so a view can render correct data before the
socket connects.

## Domain event

A typed record of an externally observable state transition.

The bar is broad and includes **automatic** changes: [chaser](glossary:chaser) steps, FOLLOW and
timecode advances, transition completion, playback release, Programmer ownership changes,
[Highlight](glossary:highlight) movement, [output health](glossary:output-health) changes.

Every event carries a monotonic sequence, event time, source surface, correlation identity where
applicable, and enough stable identity to re-request the authoritative projection.

Events describe **meaningful state boundaries** — not every render sample, DMX frame, or intermediate
fade value.

## Event bus

Application-owned publication, in `crates/application/src/event/`.

The render engine must not know about WebSocket clients, frontend stores, or OSC serialization. Domain services publish typed events; [adapters](glossary:adapter) translate them.

Implementation notes: `Arc` plus `Weak` back-references so subscriptions do not keep the bus alive,
a `watch` channel whose `send_replace` coalesces notifications, and a `VecDeque` replay ring buffer.

## Subscription

Clients subscribe **explicitly and narrowly** by capability, object identity, desk, and event class.
Not subscribing is valid — a client may work purely from snapshots.

A view that is not mounted performs no snapshot request, opens no socket, and subscribes to no
selectors. Mounting the global provider alone performs no request.

Delivery supports filtering, coalescing, per-topic rate limits, bounded queues, and backpressure.
Safety, command outcomes, errors, and discrete transitions are **never dropped**; replaceable
telemetry may be collapsed to the newest value.

## Sequence gap

A hole in the monotonic event sequence, typically after a reconnect.

The client **repairs from an authoritative [snapshot](glossary:snapshot)** before resuming
incremental delivery. It does not guess and does not silently continue. Malformed events must not
poison a reconnect, and late responses arriving after an authority replacement are rejected.

## Optimistic update

The frontend mutation pattern: apply an overlay keyed by request identity, send the
[typed command](glossary:typed-command), reconcile against whichever arrives first — the HTTP
outcome or the WebSocket event — and on failure roll back that overlay only, repairing narrowly
rather than reloading everything.

Writer policy follows the gesture: continuous values retain only the newest pending value per
target; ordered gestures are barriers; selection is FIFO.

## Revision

A monotonic counter on stored objects, the portable show, and projections. It does three jobs:
concurrency control, cache coherence, and event ordering.

## Compare and swap

The write API is optimistic concurrency, not last-writer-wins:
`put_object(kind, id, body, expected: Revision)` returns the new revision or
`RevisionConflict { expected, current }`.

There is no force-write path in normal use. This is why [action context](glossary:action-context)
carries an expected revision and why frontend writers capture the revision at dialog-open time.

## Lossless bodies

Serde silently drops unknown fields, so an older desk deserializing a newer show into typed structs
and writing it back would destroy what it did not understand — an operator's work, gone.

The fix: raw object bodies are stored as JSON and preserved verbatim. A typed mutation computes a
**before/after delta** and applies it to the raw value. See
`crates/application/src/lossless_json.rs`.

## Transaction

A batch of writes and deletes applied atomically in one SQLite transaction.

One batch produces one candidate migration and validation pass, one backup, one atomic persistence
[revision](glossary:revision), one compile, one runtime replacement, one event. A fixture count must
never become a loop of generic show-object requests.

## Ordered lifecycle

The fixed sequence the active-show boundary runs for every mutation: decode candidate, migrate,
validate, backup, CAS persist, compile, prepare runtime, install runtime, reconcile adapters, audit,
publish event.

Two consequences: the render engine never writes persistence, and a router never writes SQLite.

## Prepared install

A [typestate](glossary:typestate) pattern. Preparing a runtime snapshot is fallible and side-effect
free; installing it **consumes** the prepared value and cannot fail.

This is why persistence can never get ahead of the engine and why a failed compile cannot leave a
half-installed show. See `crates/engine/src/lifecycle.rs`.

## Typestate

Encoding a state machine in the type system so invalid states cannot be constructed. Here, holding a
`PreparedEngineSnapshot` proves all fallible work is done; installing consumes it, so you cannot
install twice or install something unprepared.

## Replay

An idempotent retry. Requests carry a request ID; a retried request already applied returns a
**replay** outcome and does not repeat persistence, command history, or interaction side effects.

## Wire DTO

A versioned serialized data-transfer object crossing the process boundary, living only in
`crates/wire/src/v2/`. Not a domain type and not a frontend view model.

`light-wire` depends on no workspace crate, so the domain can never accidentally start speaking
transport.

## Generated contract

TypeScript definitions and JSON Schemas generated from the Rust DTOs and checked in:

```sh
cargo run -p light-wire --example generate-contracts
```

Never hand-edit `apps/control-ui/src/api/generated/light-wire.ts`. A test re-renders every artifact
in memory and asserts byte equality, so a stale file fails `cargo test`. Schemas are
**direction-aware**: requests generated for deserialize, responses and events for serialize.

## Transport boundary

The one place untrusted bytes become trusted types: decode and validate in
`apps/control-ui/src/api/`, then map to view models.

Only files under `src/api/` may import [wire DTOs](glossary:wire-dto); a component doing so fails
the build. Nothing under `src/api/` may import from `src/components/` or `src/windows/`. Frontend
view types are hand-owned, not aliases of wire DTOs.

## Compatibility adapter

A deliberate temporary shim kept behaviour-compatible while callers migrate — v1 route modules,
`ws_*` handlers, `ServerContext.tsx`, `features/server/`, `useServer()`.

Keep them working; put no new domain rules in them; do not copy their shape. A new capability
belongs in a feature-local store plus a validated API adapter.

Note that **OSC is not one of these.** Internal APIs and REST/WebSocket v1 may break; exact OSC
paths, aliases, feedback indices, and desk-sharing semantics may not.

## Engine snapshot

An immutable compiled view of the show — compiled fixtures, attributes, groups, cues, bindings,
routes — that a render retains for its complete lifetime.

Because it is immutable and shared by `Arc`, fixture projection, playback state, group resolution,
and output routing cannot be mixed across show revisions while a new show installs concurrently. A
render that started under revision 41 finishes under revision 41.

## Runtime generation

The unit swapped when a show is installed. Every field is an `Arc`, published through an `ArcSwap`
and read lock-free on the render path. Installing a new show swaps one pointer.

The corollary used constantly: **clone the `Arc`, not the data.**

## Contribution

A value offered by one source for one fixture-or-head and attribute at one instant — from the
Programmer, a playback, Preload, or future dynamics.

Stateful sources sample into an immutable `ContributionBatch` *outside* the deterministic render
core. The engine consumes values supplied for the current render instant; it does not own animation
state.

## Arbitration

Resolving competing [contributions](glossary:contribution) for the same address:
[HTP](glossary:htp), [LTP](glossary:ltp), or ownership; then transitions — fade, delay,
[MIB](glossary:mib), [masters](glossary:masters); then the transient
[Highlight](glossary:highlight) overlay. The result is resolved semantic fixture values.

## Fixture projection

Mapping resolved **semantic** values onto concrete outputs — DMX channels within the fixture's
patched [mode](glossary:mode), honouring fine bytes, splits, multipatch, and
[logical heads](glossary:logical-head).

The only place in the system where "Intensity at 50%" becomes a byte. Today it produces DMX
[frames](glossary:frame); the same stage is where future typed external-device intents emerge.

## Tick budget

What the timing-critical output loop must never do: per-tick full-show cloning, broad mutex
contention, JSON serialization, frontend projection work, fixture-library reads, persistence, or
blocking external-device and sound-to-light adapters.

Effects, dynamics, macros, timecode, and device adapters may **schedule or submit** contributions.
They may not **block** frame generation.

Hard acceptance floor: 32 fully packed [universes](glossary:universe) at 100 Hz. Target: 64 at
120 Hz. Low-power goal: 4–8 at 40 Hz on Pi-class hardware.

## Monotonic clock

Deterministic, injectable time for cue fades, chasers, [MIB](glossary:mib), macro timers, and
timecode scheduling — distinct from wall-clock metadata used for event timestamps and audit.

Scheduler deadlines use real `Instant`, so a manual test clock cannot distort
real-time I/O health measurement.
