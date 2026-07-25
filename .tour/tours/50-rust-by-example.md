---
slug: rust-by-example
title: "Rust and Tauri for TypeScript Developers"
components: [engine, backend]
order: 50
---

# Rust and Tauri for TypeScript Developers

This is the guided entrance, not the complete Rust manual. It assumes general programming and
TypeScript knowledge, but no Rust knowledge. The reusable explanations live under
`docs/engineering/rust-for-typescript/`; use the links below before following unfamiliar Rust code.

## Read in this order

1. [Rust syntax and mutability][language-basics]
   `let`, `mut`, `const`, `fn`, semicolons and `()`, structs, enums, `match`, attributes,
   macros, and the precise meaning of `unsafe`.
2. [Types, errors, and ownership][types-errors-ownership]
   Newtypes, `Option`, `Result`, `?`, references, borrowing, lifetimes, traits, generics, and
   typestate.
3. [Shared state, async, and serialization][shared-state-async-serde]
   `Arc`, locks, interior mutability, async tasks, cancellation, channels, serde, iterators, and
   feature flags.
4. [Tauri and the TypeScript boundary][tauri-process-edge]
   Tauri commands, `#[tauri::command]`, handler registration, windows, process ownership, and the
   typed `DesktopBridge`.

Each guide defines syntax before using it. If a code sample still assumes a Rust concept it has not
introduced, treat that as a documentation defect.

## Repository route after the language guides

Read the implementation in this order:

1. `crates/core/src/lib.rs` and `attributes.rs` — newtypes, enums, and serde.
2. `crates/show/src/error.rs` and `show_store.rs` — typed errors, borrowing, and revisioned writes.
3. `crates/application/src/action.rs` — typed commands and action context.
4. `crates/output/src/scheduler.rs` and `delivery/driver.rs` — async scheduling and cancellation.
5. `crates/application/src/event/bus.rs` — shared state, weak references, and channels.
6. `crates/engine/src/runtime_generation.rs`, `engine.rs`, and `lifecycle.rs` — coherent snapshots,
   `ArcSwap`, and typestate.
7. `apps/control-ui/src/platform/desktop/` and `apps/control-ui/src-tauri/src/` — the typed native
   bridge.

For the native application map, continue with the **Tauri Desktop Apps** component page.
Architecture rules remain in `docs/engineering/`; this tour links to them rather than duplicating
them.

## Working habits

```sh
cargo fmt                    # not standalone rustfmt
cargo clippy --workspace
cargo test --workspace
npm run test:unit
```

Operator-visible desktop behavior must ultimately be verified through `npm run open`.

[language-basics]: https://github.com/kellertobias/tosklight/blob/main/docs/engineering/rust-for-typescript/01-language-basics.md
[types-errors-ownership]: https://github.com/kellertobias/tosklight/blob/main/docs/engineering/rust-for-typescript/02-types-errors-ownership.md
[shared-state-async-serde]: https://github.com/kellertobias/tosklight/blob/main/docs/engineering/rust-for-typescript/03-shared-state-async-serde.md
[tauri-process-edge]: https://github.com/kellertobias/tosklight/blob/main/docs/engineering/rust-for-typescript/04-tauri-process-edge.md
