---
slug: rust-by-example
title: Rust by Example
components: [engine, backend]
order: 50
---

# Rust by Example

The Rust this codebase uses, from the code it runs. Assumes you can read `let`, `fn`, `struct`, and
`match`. Skips features not used here.

Two workspace facts first: Rust 2024 edition with `resolver = "3"`, and `unsafe_code = "forbid"` at
the workspace level plus `#![forbid(unsafe_code)]` in most `lib.rs` files. There is no `unsafe`
anywhere.

## 1. Newtypes and a declarative macro

`crates/core/src/lib.rs:17-46`

```rust
pub type Revision = u64;
pub type Universe = u16;
pub type DmxAddress = u16;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }
    };
}

id!(UserId); id!(SessionId); id!(ShowId);
id!(FixtureId); id!(CueListId); id!(ProgrammerId);
```

`FixtureId` and `CueListId` are both `Uuid` at runtime, but the compiler rejects passing one where
the other is expected. In a system with a dozen kinds of identity that removes a class of bug.

Three details:

- `#[serde(transparent)]` makes the JSON a bare UUID string, so the newtype costs nothing on the
  wire.
- `Default` generates a fresh UUID rather than a zero value.
- This is the only `macro_rules!` in the workspace. Macros hurt readability; this one generates six
  types from nine lines.

The `type` aliases above it read better than `u64` but are not type-safe. Know which you picked.

## 2. Enums carrying data, and serde tagging

`crates/core/src/attributes.rs:273-294`

```rust
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AttributeValue {
    Normalized(f32),
    Spread(Vec<f32>),
    Discrete(String),
    ColorXyz(Xyz),
    RawDmx(u8),
    RawDmxExact(u32),
}
```

A Rust enum is a tagged union: each variant carries its own data. `match` on it is exhaustive, so
adding a seventh variant breaks every `match` that does not handle it. That is how a new attribute
kind cannot be silently ignored.

Two tagging styles are used:

- **Adjacently tagged** here: `{"kind":"normalized","value":0.5}`, needed because `Normalized(f32)`
  is a newtype variant with no field names.
- **Internally tagged** across `crates/wire`, e.g. `crates/wire/src/v2/events.rs:36-54` — struct
  variants giving a flat `{"type":"subscribe", ...}`, which the TypeScript side reads as a
  discriminated union.

Many wire DTOs add `deny_unknown_fields` (`crates/wire/src/v2/programming.rs:119`), so an unexpected
field is a rejected request rather than a silently ignored one.

**Try it:** add a variant locally and run `cargo check`. The error list is the exhaustiveness
guarantee.

## 3. Option and its combinators

`crates/core/src/attributes.rs:287-294`

```rust
pub fn normalized(&self) -> Option<f32> {
    match self {
        Self::Normalized(value) => Some(*value),
        _ => None,
    }
}
```

Rust has no `null`. Absence is `Option<T>` and the compiler makes you handle it.

| Combinator | Site | Meaning |
| --- | --- | --- |
| `.and_then(...)` | `crates/fixture/src/library/package_io.rs:65` | chain another fallible step |
| `.map_or(0, ...)` | `crates/fixture/src/library/package_io.rs:25` | transform, or a default |
| `.unwrap_or_else(...)` | `crates/fixture/src/patch.rs:24` | lazily compute a fallback |
| `.ok_or_else(...)` | `crates/fixture/src/patch_validation.rs:291` | turn `None` into a domain error |

`ok_or_else` is where absence becomes failure. The `_or_else` variants take a closure, so the
fallback is not computed when unused.

`*value` dereferences: `self` is `&self`, so `value` is `&f32`, and `f32` is `Copy`.

## 4. Errors: Result, `?`, thiserror

`crates/show/src/error.rs:5-34` shows three patterns in one file:

```rust
#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),

    RevisionConflict { expected: Revision, current: Revision },
    // ...
}
```

Rust has no exceptions. Fallible functions return `Result<T, E>` and `?` propagates early.

- `#[from]` generates `impl From<rusqlite::Error> for StoreError`, which is what lets `?` convert
  across a library boundary. `?` is calling `From::from`.
- `#[error(transparent)]` forwards the inner message unchanged.
- Struct variants carry data. `RevisionConflict { expected, current }` is why optimistic concurrency
  works: the caller gets the numbers, not a string.

Same shape at `crates/mvr/src/lib.rs:26-35` and `crates/core/src/lib.rs:48-58`.

### Conversions between layers

Layer-crossing conversions are hand-written `From` impls, not derived:

- `crates/application/src/timeline/mod.rs:66,77,88` — three error types into one `TimelineError`
- `crates/application/src/macro_runtime/model.rs:282` — `ActionError` to `MacroError`
- `crates/application/src/programming/lifecycle_projection.rs:86,94` — domain to projection DTO

Some convert from a reference: `impl From<&MacroWaitRequest> for MacroWaitState`
(`crates/application/src/macro_runtime/model.rs:123`) only reads, so it borrows.

### assert! vs Result

`crates/application/src/event/bus.rs:41,80-83` uses `assert!` for invariants that indicate a bug,
while user and I/O errors return `Result`. Panicking is correct when the alternative is continuing
with a broken invariant.

## 5. Ownership, borrowing, lifetimes

### The minimal lifetime

`crates/programmer/src/command_line/helpers.rs:1`

```rust
fn strip_prefix_word<'a>(value: &'a str, prefix: &str) -> Option<&'a str>
```

The returned slice lives as long as `value`. Only `value` is annotated, because only `value` can be
the source of the return; `prefix` is read and discarded. Lifetimes annotate relationships between
inputs and outputs, not how long a variable lives.

### Borrowed structs

`crates/show/src/show_store.rs:14-25`

```rust
pub struct AtomicObjectWrite<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub body: &'a serde_json::Value,
    pub expected: Revision,
}
```

Consumed as `&[AtomicObjectWrite<'_>]` at `:155-158`, so the write path allocates nothing. `'_` is
the anonymous lifetime: there is one, infer it.

### Cow

`crates/application/src/programming/values_action.rs:66-70`

```rust
pub fn mutations(&self) -> Cow<'_, [ProgrammingValueMutation]> {
    match self {
        Self::Batch { mutations } => Cow::Borrowed(mutations),
        Self::SetFixture { .. } => Cow::Owned(vec![/* ... */]),
    }
}
```

The `Batch` variant already holds a `Vec` and hands back a slice; single-mutation variants must
synthesize one. `Cow` serves both without forcing an allocation on the common path.

Two `Cow` sites exist in the workspace. It is not a default choice.

### impl Trait in argument position

`open(path: impl AsRef<Path>)` (`crates/show/src/show_store.rs:28`), `impl Into<String>` for error
constructors (`crates/fixture/src/patch_validation.rs:327`), `impl IntoIterator<Item = &'a
PortablePatchedFixtureRecord>` (`crates/fixture/src/portable_patch/compiler.rs:138`). Caller
convenience, static dispatch.

## 6. Traits: dyn, generics, associated types

### Trait objects

`crates/core/src/clock.rs:16-20`

```rust
pub trait ApplicationClock: Debug + Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
pub type SharedClock = Arc<dyn ApplicationClock>;
```

Two impls: `SystemClock` (unit struct) and `ManualClock`, which uses `RwLock<DateTime<Utc>>` to
mutate through `&self` (`:43-51`). Tests inject `ManualClock` and control time. `Send + Sync` make
it shareable across threads.

The doc comment at `:14-15` notes that scheduler deadlines use real `Instant` so a test clock cannot
distort real-time I/O health measurement — an example of scoping an abstraction.

More `dyn`: `crates/application/src/macro_runtime/host.rs:85-98`, and the boxed closure alias
`pub type MacroTask = Box<dyn FnOnce() + Send + 'static>;`
(`crates/application/src/macro_runtime/service.rs:20`).

### Associated types

`crates/application/src/active_show/ports.rs:33-57`

```rust
pub trait ActiveShowPorts: Send + Sync {
    type UnitOfWork: ActiveShowUnitOfWork;
    type PreparedRuntime;

    fn authorize_mutation(&self) -> Result<(), ActionError> { Ok(()) }

    fn run_active_show_lifecycle<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ActionError>,
    ) -> Result<T, ActionError>;
}
```

Each implementor picks its own concrete types, resolved at compile time — no vtable, no boxing.

Also shows default trait methods, a generic method taking a closure, and supertraits:
`ShowPatchPorts: ActiveShowPorts` (`crates/application/src/show_patch/ports.rs:7`), and the
marker-only `ProgrammingGroupActiveShowPorts: ActiveShowPorts {}`
(`crates/application/src/programming/group_recording.rs:178`).

Consumers are generic over the port: `impl<P: SelectiveShowImportPorts> Planner<'_, P>`
(`crates/application/src/selective_import/plan/conflicts.rs:9`).

**Choosing:** `dyn` when the implementation set is open or chosen at runtime and the call is not
hot. Generics when the type is known at compile time and you want it inlined.

### Associated consts

`crates/application/src/action.rs:79-83` requires a constant, not just methods:

```rust
pub trait ApplicationCommand: Send + 'static {
    type Value: Send + 'static;
    const FAMILY: CommandFamily;
}
```

The doc comment states the intent: concrete commands implement this instead of joining one
process-wide command enum.

## 7. Consuming builders and const fn

`crates/application/src/action.rs:67-75, 108-116`

```rust
pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
    self.request_id = Some(id.into());
    self
}

pub const fn with_expected_revision(mut self, revision: u64) -> Self {
    self.expected_revision = Some(revision);
    self
}
```

Take `mut self` by value, mutate, return `Self`. It chains and needs no separate builder struct;
ownership makes it safe.

One is `const fn` and one is not: `with_request_id` allocates through `Into<String>`, so it cannot
run at compile time.

At `:149`, `matches!` as a boolean expression with an or-pattern:

```rust
retryable: matches!(kind, ActionErrorKind::Busy | ActionErrorKind::Unavailable)
```

## 8. Typestate

`crates/engine/src/lifecycle.rs:12-21`

```rust
/// Preparing a snapshot is side-effect free. Installing it consumes this value and cannot fail,
/// which lets callers complete fallible work before committing an authoritative show mutation.
#[must_use = "a prepared snapshot must be installed to affect the live engine"]
pub struct PreparedEngineSnapshot {
    snapshot: EngineSnapshot,
    runtime: PreparedRuntime,
}
```

`prepare_snapshot(...) -> Result<PreparedEngineSnapshot, _>` is fallible with no side effects.
`install_prepared_snapshot(prepared) -> ()` takes it by value and cannot fail.

The type encodes that all fallible work is done. You cannot install something unprepared, because
there is no other way to construct one, and you cannot install twice, because installing consumes
it. That is why persistence cannot get ahead of the engine and a failed compile cannot leave a
half-installed show — see the comment at `:59-62`.

`#[must_use]` makes forgetting to install a warning. The workspace has three, all this pattern:
here, `crates/engine/src/playback_batch.rs:37`, and `crates/engine/src/contribution_batch.rs:173`.

## 9. Arc, ArcSwap, interior mutability

`crates/engine/src/runtime_generation.rs:12-26`

```rust
/// A render retains this value for its complete lifetime, so fixture projection, Playback state,
/// Group resolution, and output routing cannot be mixed across show revisions while a new show is
/// installed concurrently.
pub(crate) struct RuntimeGeneration {
    snapshot: Arc<EngineSnapshot>,
    playback: Arc<RwLock<PlaybackEngine>>,
    groups: Arc<HashMap<String, GroupDefinition>>,
    routes: Arc<[OutputRoute]>,
    // ...
}
```

`Arc<T>` is a thread-safe reference-counted pointer: cloning copies a pointer and bumps a counter,
not the data. Clone the `Arc`, not the data.

`Arc<[OutputRoute]>` is a shared slice rather than `Arc<Vec<_>>` — one less pointer hop, built with
`Arc::from(...)`.

The generation is published through `ArcSwap<RuntimeGeneration>` (`crates/engine/src/engine.rs:19,43`)
and read lock-free on the render path (`:90-94`). Installing a show swaps one pointer, so a render
that started under revision 41 finishes under revision 41.

### Interior mutability

`crates/engine/src/engine.rs:18-36` — every mutating method on `Engine` takes `&self`:

| Type | Used for |
| --- | --- |
| `ArcSwap<_>` | whole-generation swap, lock-free reads |
| `AtomicU64` / `AtomicBool` | individual scalars |
| `parking_lot::Mutex<_>` | maps needing exclusive edit |
| `parking_lot::RwLock<_>` | read-heavy maps |

Two details: `speed_groups_bpm: [AtomicU64; 5]` (`:25`) stores f64 bit patterns
(`120.0_f64.to_bits()` at `:54`) because there is no `AtomicF64`, explained at `:23-24`; and
`std::array::from_fn(|_| AtomicBool::new(false))` (`:60`) builds a fixed-size array of non-`Copy`
values, which `[AtomicBool::new(false); 8]` cannot do.

`crates/programmer/src/registry.rs:10-46` is the counterpart: `#[derive(Clone)]` over 14
`Arc<RwLock<…>>` fields, so cloning shares state. `ReentrantMutex` at `:38-44` carries a written
justification.

### Two lock families

`std::sync` locks poison on panic and are used with `.expect("… poisoned")` in cold paths
(`crates/core/src/clock.rs:44,48,56`). `parking_lot` locks do not poison and are faster, used in hot
paths.

## 10. Async

### Generic over the future

`crates/output/src/scheduler.rs:16-26`

```rust
pub async fn run_scheduler<F, Fut>(
    rate_hz: u16,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    tick: F,
)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
```

Being generic over `F` and its returned future lets tests drive the real scheduler with a fake tick:
no sockets, no timing flakiness.

Also here: `CancellationToken` for cooperative shutdown (`:38`), `AtomicU16` for live rate changes
(`:39`), `tokio::time::sleep_until` with `Instant::from_std` (`:73`).

The `&Mutex<OutputHealth>` parameters at `:49,61,70` pass the lock rather than a guard, and lock
scopes are narrow around `.await` points. Holding a non-async lock across an `.await` is a common
way to deadlock async Rust.

### async_trait and default methods

`crates/output/src/delivery/driver.rs:10-17`

```rust
#[async_trait]
pub trait OutputDriver: Send + Sync {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()>;

    async fn terminate(&self, universe: Universe, sequence: u8) -> io::Result<()> {
        self.send(universe, sequence, &[0; DMX_SLOTS]).await
    }
}
```

`async fn` in traits historically needed `#[async_trait]`, which boxes the future. The default
`terminate` gives implementors blackout. `&[0; DMX_SLOTS]` coerces to `&DmxFrame`.

At `:27`, a match guard inside `matches!`:

```rust
if matches!(destination.ip(), IpAddr::V4(address) if address.is_broadcast())
```

### Channels

`crates/application/src/event/bus.rs`

```rust
#[derive(Clone)]
pub struct EventBus { inner: Arc<EventBusInner> }
struct EventBusInner { state: Mutex<EventBusState>, changed: watch::Sender<u64> }
```

| Line | Notice |
| --- | --- |
| `:42` | `watch::channel(0)` with the initial receiver dropped |
| `:67` | `send_replace` coalesces, so slow subscribers see the latest sequence, not a backlog |
| `:92` | `Arc::downgrade(&self.inner)` — subscriptions hold a `Weak`, avoiding a reference cycle |
| `:100` | `Arc::clone(&event)` spelled out, the convention for making refcount bumps visible |
| `:34` | `VecDeque<Arc<EventEnvelope>>` as a replay ring buffer |

Contrast `watch` (latest value, coalescing) with `broadcast`
(`crates/server/src/runtime/state.rs:29`), where every subscriber gets every message.

## 11. Serde

### Field attributes

`crates/core/src/attributes.rs:303-323`:

- `#[serde(default)]` for a field added later, so older documents still deserialize.
- `#[serde(default, skip_serializing_if = "Option::is_none")]` for optional fields, so
  absent stays absent rather than round-tripping as `null`.

The doc comments explain why `Option` is used rather than a sentinel: absent and present-but-zero
mean different things to an operator.

### Custom Deserialize for migration

Hand-written impls accept multiple legacy JSON shapes:
`crates/fixture/src/profile/channel_model.rs:99-113`, `crates/fixture/src/patch_model.rs:100-126`,
and `crates/fixture/src/portable_patch/codec.rs:32-45` with a matching custom `Serialize` at
`:25-27`.

This is where you meet the `'de` lifetime and the `D: Deserializer<'de>` bound. `'de` is the
lifetime of the data being deserialized from, which is what allows zero-copy deserialization of
borrowed strings.

### Lossless JSON

`crates/application/src/lossless_json.rs:1-26`

```rust
pub fn merge_typed<T: Serialize>(
    stored: &Value, before: &T, after: &T,
) -> serde_json::Result<Value>
```

Serde drops unknown fields. An older desk deserializing a newer show into typed structs and writing
it back would destroy the fields it did not understand — an operator's work, gone.

The fix computes a typed before/after delta and applies it to the raw stored JSON: typed ergonomics
inside, lossless storage outside.

## 12. Iterators and the rest

### Iterator pipelines

- Nested `flat_map` and `filter_map` with `move` closures —
  `crates/fixture/src/definition.rs:140-142`. `move` is required because the closures outlive the
  loop.
- `zip` and `fold` — `crates/fixture/src/encoding.rs:95`, `crates/engine/src/engine.rs:85-87`.
- `filter_map(Result::ok)` — `crates/fixture/src/library/package_io.rs:49`, discarding errors by
  function reference.
- Array `.map()`, the inherent method rather than the iterator one —
  `crates/engine/src/engine.rs:78-84`.

Iterators are lazy and compile to roughly the loop you would have written.

### Feature flags

`crates/control/Cargo.toml`:

```toml
default = ["native-midi"]
native-midi = ["dep:midir"]
```

`dep:` makes the optional dependency non-implicit. Propagated through `crates/engine` and
`crates/server`; consumers import with `default-features = false`. Usage:
`crates/control/src/midi.rs:1,5,8,23,29,72`.

The reason: portable Linux binaries omit native USB-MIDI because it depends on the target machine's
ALSA library. See also target-conditional dependencies —
`[target.'cfg(unix)'.dependencies] xattr` in `crates/server/Cargo.toml`.

### Const generics

`crates/application/src/programming/group_active_show_tests.rs:313`:

```rust
fn new<const N: usize>(objects: [(&str, Value); N]) -> Self
```

Takes a fixed-size array of any length by value, so call sites need no `vec![]`.

### Visibility

`pub(crate)` (`crates/engine/src/engine.rs:19-35`), `pub(super)`
(`crates/server/src/runtime/state.rs:5-43`), and `pub(crate) use` re-export
(`crates/show/src/lib.rs:33`).

`crates/fixture/src/lib.rs` uses blanket `pub use module::*;`, which is convenient but hides the
public surface. Not a default to copy.

### Boxing to shrink an enum

`crates/wire/src/v2/events.rs:60`:

```rust
Event { event: Box<EventEnvelope> }
```

An enum is sized by its largest variant, so boxing the big one keeps the others cheap to move.

## Reading order

1. `crates/core/src/lib.rs` — macro, newtypes
2. `crates/core/src/attributes.rs` — enums, serde
3. `crates/core/src/clock.rs` — first `dyn` trait, interior mutability
4. `crates/show/src/error.rs` — thiserror
5. `crates/show/src/show_store.rs` — lifetimes, compare-and-swap
6. `crates/application/src/action.rs` — typed commands, builders
7. `crates/output/src/scheduler.rs` and `delivery/driver.rs` — async
8. `crates/application/src/event/bus.rs` — Arc, Weak, watch
9. `crates/engine/src/runtime_generation.rs` and `engine.rs` — Arc snapshots, ArcSwap
10. `crates/engine/src/lifecycle.rs` — typestate
11. `crates/application/src/active_show/ports.rs` — associated-type generics

## Working habits

```sh
cargo fmt                    # not standalone rustfmt
cargo clippy --workspace
cargo test --workspace
./test unit
```

Files ≤400 lines and functions ≤20 are goals; ≤1200 and ≤150 are hard limits checked by
`tools/check-source-size.mjs` against an empty ratchet baseline, so any new violation fails
immediately. Split by responsibility, abstraction level, ownership, and test boundary rather than to
satisfy a number.
