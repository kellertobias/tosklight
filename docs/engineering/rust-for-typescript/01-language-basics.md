# Rust Language Basics for TypeScript Developers

This guide assumes general programming and TypeScript knowledge, but no Rust knowledge. It defines
the syntax used throughout ToskLight before relying on it.

## Bindings and mutation

`let` introduces a local binding:

```rust
let universe = 1;
let address: u16 = 512;
let mut sequence = 0;
sequence += 1;
```

Rust bindings are immutable unless `mut` is explicit. This differs from TypeScript because
TypeScript `const` prevents rebinding but normally leaves the referenced object mutable:

```ts
const address = { universe: 1, channel: 42 };
address = { universe: 2, channel: 1 }; // error
address.channel = 100;                 // allowed
```

An immutable Rust binding prevents both operations through that binding:

```rust
let address = Address { universe: 1, channel: 42 };
address = Address { universe: 2, channel: 1 }; // error
address.channel = 100;                         // error
```

`let mut` grants both reassignment and mutation, but it does not weaken typing or memory safety:

```rust
let mut address = Address { universe: 1, channel: 42 };
address.channel = 100;
address = Address { universe: 2, channel: 1 };
```

The replacement must still be an `Address`. Ownership and borrowing still decide whether any other
access is permitted.

- `&Address` is a shared reference: multiple readers, normally no mutation.
- `&mut Address` is an exclusive mutable reference: Rust prevents competing access while it is
  borrowed.

This is stronger than TypeScript `readonly`, which is a type-level view and does not freeze the
JavaScript object. Rust also has controlled **interior mutability** through types such as `Mutex`
and `RwLock`; the shared-state guide explains it.

Shadowing creates a new binding and is not mutation:

```rust
let input = "512";
let input = input.len(); // a new binding, now usize
```

Rust `const` is a compile-time constant with a required type, not the equivalent of ordinary
TypeScript `const`:

```rust
const DMX_SLOTS: usize = 512;
```

Most TypeScript locals declared with `const` become immutable Rust `let` bindings.

## Functions, semicolons, and `()`

`fn` declares a function. Parameter types follow their names and `->` introduces the return type:

```rust
fn next_sequence(current: u8) -> u8 {
    current.wrapping_add(1)
}
```

The final expression without a semicolon becomes the block's value:

```rust
let brightness = {
    let base = 40;
    base + 2
};
// brightness is 42

let nothing = {
    let base = 40;
    base + 2;
};
// nothing is ()
```

With the semicolon, `base + 2;` is a completed statement whose result is discarded. With no final
expression left, the block evaluates to `()`. This is pronounced “unit.” It is one real value,
written as empty parentheses, used when there is no meaningful result.

The same rule controls implicit function returns:

```rust
fn next(current: u8) -> u8 {
    current.wrapping_add(1)
}

fn broken(current: u8) -> u8 {
    current.wrapping_add(1);
    // Error: the body is (), but the signature promises u8.
}

fn announce(current: u8) {
    println!("Sequence: {current}");
}
// No declared return type means -> ().
```

`return value;` returns early. Its semicolon is required syntax and does not discard `value`.
`pub fn` makes a function visible outside its module. Rust items are private by default.

## Structs and methods

A `struct` defines a nominal data type, not merely an object shape:

```rust
struct Address {
    universe: u16,
    channel: u16,
}

let address = Address { universe: 1, channel: 42 };
```

A tuple struct such as `struct FixtureId(Uuid);` is a checked wrapper with positional fields. A unit
struct such as `struct SystemClock;` has no fields.

An `impl` block attaches functions and methods:

```rust
impl Address {
    fn is_first_universe(&self) -> bool {
        self.universe == 1
    }
}
```

`Self` means `Address` in this block. A method receiving `self`, `&self`, or `&mut self` consumes,
borrows, or exclusively mutably borrows the value.

## Enums and exhaustive `match`

A Rust enum is a discriminated union whose variants may carry different data:

```rust
enum Level {
    Off,
    Normalized(f32),
}

fn number(level: Level) -> f32 {
    match level {
        Level::Off => 0.0,
        Level::Normalized(value) => value,
    }
}
```

This is closer to a TypeScript discriminated union than a numeric enum. Every possible variant must
be handled. `_` is a catch-all pattern; `=>` separates a pattern from its result expression.
`::` accesses a module or type item, while `.` accesses a field or calls a method.

## Attributes: `#[...]` and `#![...]`

The hash-and-square-bracket syntax is a Rust **attribute**. “Decorator” is a useful comparison, but
attributes are compile-time metadata or macro inputs, not runtime wrappers.

- `#[something]` applies to the item or field immediately after it.
- `#![something]` applies to the containing item, commonly the entire crate.
- `#[derive(Clone, Debug)]` generates implementations of named traits.
- `#[serde(...)]`, `#[error(...)]`, and `#[tauri::command]` configure library procedural macros.
- `#[cfg(target_os = "macos")]` conditionally includes code at compile time.

Each attribute has its own meaning; do not treat all attributes as interchangeable decorators.

## Macros and `!`

`vec![...]`, `matches!(...)`, and `id!(FixtureId)` invoke macros. Macros expand Rust syntax at
compile time and can generate declarations. `macro_rules! id { ... }` defines a declarative macro;
`$name:ident` captures an identifier. ToskLight uses macros sparingly.

## What `unsafe` means

`unsafe` does not disable Rust's type system. It marks operations whose memory-safety requirements
the compiler cannot prove, such as dereferencing a raw pointer or calling an unsafe function. The
programmer must uphold documented invariants; normal type and borrowing checks still apply.

ToskLight sets `unsafe_code = "forbid"` at workspace level, and most crate roots repeat
`#![forbid(unsafe_code)]`. Rust code using the `unsafe` keyword is rejected. English messages such
as “unsafe archive path” are unrelated to this language feature.
