# Rust Shared State, Async, and Serde for TypeScript Developers

Read [language basics](01-language-basics.md) and
[types, errors, and ownership](02-types-errors-ownership.md) first.

## `Arc` and interior mutability

`Arc<T>` is a thread-safe reference-counted pointer. Cloning it increments a counter and shares the
same allocation; it does not clone `T`.

Immutable bindings and shared references normally prevent mutation. A synchronization type can
provide controlled **interior mutability**:

| Type | ToskLight use |
| --- | --- |
| `Mutex<T>` | one reader or writer at a time |
| `RwLock<T>` | multiple readers or one writer |
| atomics | lock-free scalar state |
| `ArcSwap<T>` | atomically replace one shared snapshot |

`Engine` publishes a complete `RuntimeGeneration` through `ArcSwap`. A render retains one
generation, so it cannot mix fixture data from one show revision with routes from another.

Interior mutability is not a loophole in the borrow checker. The wrapper enforces access at runtime,
and its guard defines how long the protected access lasts.

## Async functions and futures

`async fn` returns a future. `.await` yields while that future is pending so the executor can run
other work; it does not create an operating-system thread per call.

ToskLight async loops receive a `CancellationToken` and have an owning lifecycle. Shutdown is
cooperative and explicit. Lock guards should not normally be held across `.await`, because another
task may need that lock before the awaited operation can finish.

`#[async_trait]` is a procedural macro from the `async-trait` library. It rewrites compatible async
trait methods and boxes their futures. It is compile-time transformation, not a runtime decorator.

## Closures and generic futures

Closures use vertical bars for parameters:

```rust
items.map(|item| item.name)
```

`move |item| ...` moves captured values into the closure. Scheduler functions are generic over a
closure and the future it produces, allowing tests to supply a fake tick without sockets.

## Channels and task communication

ToskLight uses channel types according to delivery semantics:

- `watch` stores the latest value and coalesces intermediate updates.
- `broadcast` delivers each message to each active subscriber.
- bounded queues apply backpressure rather than growing forever.
- `Weak<T>` observes shared state without keeping it alive and avoids reference cycles.

Choose a channel from the behavior the operator needs, not merely because it is async.

## Serde attributes

Serde generates Rust-to-wire serialization:

```rust
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AttributeValue {
    Normalized(f32),
    Discrete(String),
}
```

This produces a discriminated JSON shape such as
`{"kind":"normalized","value":0.5}`. Common field attributes include:

- `#[serde(default)]` to accept an absent field using its default.
- `skip_serializing_if = "Option::is_none"` to preserve absence instead of writing `null`.
- `deny_unknown_fields` to reject unexpected request fields.

Hand-written `Deserialize` implementations accept legacy show shapes. ToskLight also retains raw
JSON and applies typed before/after changes to it so an older desk does not destroy unknown fields
written by a newer version.

## Iterators and conditional compilation

Rust iterators such as `map`, `filter_map`, `flat_map`, `zip`, and `fold` are lazy until consumed
and generally compile to loops without intermediate allocations.

Cargo feature flags conditionally include dependencies. Rust attributes such as
`#[cfg(target_os = "macos")]` conditionally compile code for a target. Neither is a runtime
feature toggle.

Visibility narrows access: `pub` is public, `pub(crate)` is crate-wide, and `pub(super)` is visible
to the parent module. Keep the narrowest visibility that supports the intended boundary.
