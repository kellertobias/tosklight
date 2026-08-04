---
slug: rust-by-example
title: "Rust and Tauri for TypeScript Developers"
components: [engine, backend]
order: 50
---

# Rust and Tauri for TypeScript Developers

This is the guided repository entrance, not a replacement for the language manual. It assumes
general programming and TypeScript knowledge, but no Rust knowledge. Read the official
[Rust Book](https://doc.rust-lang.org/book/) for language concepts before following unfamiliar
Rust code here.

## Repository route after the language guides

Read the implementation in this order:

1. `crates/shared/core/src/lib.rs` and `attributes.rs` — newtypes, enums, and serde.
2. `crates/shared/show/src/error.rs` and `show_store.rs` — typed errors, borrowing, and revisioned writes.
3. `crates/light/src/action.rs` — typed commands and action context.
4. `crates/light/domain/output/src/scheduler.rs` and `delivery/driver.rs` — async scheduling and cancellation.
5. `crates/light/src/event/bus.rs` — shared state, weak references, and channels.
6. `crates/light/domain/engine/src/runtime_generation.rs`, `engine.rs`, and `lifecycle.rs` — coherent snapshots,
   `ArcSwap`, and typestate.
7. `apps/light-desktop/src/platform/desktop/` and `apps/light-desktop/src-tauri/src/` — the typed native
   bridge.

For the native application map, continue with the **Tauri Desktop Apps** component page.
Architecture rules remain in this Code Safari; the tour links to them rather than duplicating
them.

## Working habits

```sh
cargo fmt                    # not standalone rustfmt
cargo clippy --workspace
cargo test --workspace
npm run test:unit
```

Operator-visible desktop behavior must ultimately be verified through `npm run open`.
