# Rust Types, Errors, and Ownership for TypeScript Developers

Read [language basics](01-language-basics.md) first.

## Aliases and newtypes

`type Revision = u64;` creates another spelling for `u64`; the compiler does not distinguish them.
`struct FixtureId(Uuid);` creates a distinct type. ToskLight uses newtypes so a `CueListId` cannot
accidentally be passed where a `FixtureId` is required.

`#[derive(...)]` generates standard trait implementations.
`#[serde(transparent)]` serializes a one-field newtype like its inner UUID.

## Generic types, `Option`, and `Result`

Angle brackets supply type arguments. `Vec<f32>` is a growable list of 32-bit floats.
`Option<T>` represents `Some(T)` or `None`; Rust does not use unchecked `null` for ordinary optional
values:

```rust
pub fn normalized(&self) -> Option<f32> {
    match self {
        Self::Normalized(value) => Some(*value),
        _ => None,
    }
}
```

Recoverable failures use `Result<T, E>`: `Ok(T)` or `Err(E)`. The `?` operator returns an error
early and converts it through `From` when needed:

```rust
#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    RevisionConflict { expected: Revision, current: Revision },
}
```

These `thiserror` attributes generate the standard error behavior and conversion. They are
compile-time code generation, not exception handlers. ToskLight uses `assert!` for broken internal
invariants and `Result` for user, validation, and I/O failures.

## Ownership and borrowing

Every value has an owner. Assigning or passing a non-`Copy` value normally moves ownership instead
of silently sharing an object reference. Borrowing with `&T` reads without taking ownership;
borrowing with `&mut T` grants exclusive mutation.

This prevents use-after-free and unsynchronized aliasing at compile time. Use `.clone()` only when
an independent owned copy is intended. Cloning `Arc<T>` shares the same allocation instead.

## Lifetimes

A reference cannot outlive its source. The compiler usually infers this. A lifetime name describes
the relationship when a signature is ambiguous:

```rust
fn strip_prefix_word<'a>(value: &'a str, prefix: &str) -> Option<&'a str>
```

The returned slice may live no longer than `value`. `'a` is not a timer and does not keep anything
alive. `'_` asks the compiler to infer an unnamed lifetime.

`Cow<'a, T>` means “borrowed or owned.” ToskLight uses it only where a common path can borrow
existing data while another path must construct data.

## Traits, generics, and `dyn`

A trait is a behavior contract, roughly comparable to a TypeScript interface used for capabilities:

```rust
pub trait ApplicationClock: Debug + Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub type SharedClock = Arc<dyn ApplicationClock>;
```

`dyn ApplicationClock` is runtime dispatch through a trait object. A generic such as
`fn run<P: Ports>(ports: &P)` uses a concrete type selected at compile time. Associated types let
each trait implementation select related concrete types. `Send + Sync` states that the value can
cross and be shared across thread boundaries.

Use runtime `dyn` when implementations are selected dynamically. Use generics when the concrete
type is known and static dispatch is useful.

## Builders and typestate

A consuming builder takes `mut self`, changes it, and returns `Self`:

```rust
pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
    self.request_id = Some(id.into());
    self
}
```

Typestate represents a lifecycle stage as a distinct type. `PreparedEngineSnapshot` can only be
created by successful preparation and is consumed by installation. It cannot be installed twice or
constructed in an unprepared state. `#[must_use]` warns if code prepares one and ignores it.

This is a recurring Rust design idea: make an invalid transition impossible to express rather than
checking a boolean later.
