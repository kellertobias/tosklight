---
slug: orientation
title: Orientation
order: 10
---

# Orientation

What ToskLight is, how the layers fit, and the rules that are enforced rather than suggested.

## Run it

```sh
./dev
```

Open `http://127.0.0.1:5000`. A new desk has one enabled `Operator` user.

Then do the operator loop the architecture exists to serve: select a fixture on the Stage or
Fixture Sheet, set an intensity, `RECORD` it into a group and then a cue, put the cuelist on a
playback, run it.

If those words are unfamiliar, read the glossary first.

## Layers

```
UI / Keyboard ┐
OSC/hardware  ├→ input adapters →  typed application services  →  domain crates
HTTP          │                            ↓         ↓
future Macros ┘                     show compiler   event bus
                                            ↓         ↓
                                     render engine → output → Art-Net / sACN
```

| Layer | Path | Constraint |
| --- | --- | --- |
| Adapters | `crates/server/`, UI transports | Parse, authenticate, translate. No business rules. |
| Application | `crates/application/` | Transport-independent use cases. Owns state, exposes commands and immutable projections. |
| Domain | `crates/{core,fixture,playback,programmer,output,control,show,media,mvr}` | No HTTP, WebSocket, SQLite, or Tauri. |
| Wire | `crates/wire/` | Leaf. Versioned DTOs only. |
| Frontend | `apps/control-ui/`, `apps/hardware-controls/` | Renders authoritative projections. |

Dependency direction is checked in CI by `tools/check-architecture.mjs`.

## One action, one authority

An operator can raise a fader from the software UI, the keypad, the command line, OSC hardware, an
HTTP request, or a future macro. All six produce one typed action, processed by one service,
publishing one semantic event.

Before the refactor, six surfaces each implemented the same orchestration. Every bug had to be
fixed six times, and they drifted.

See [one action, one authority](glossary:one-action-one-authority).

## Six state lifetimes

Portable show, desk installation, desk interaction, user Programmer, connection/session, transient
runtime.

Before adding a field, answer seven questions about it: lifetime, persistence location, migration
policy, reconnect behaviour, restart behaviour, Save As behaviour, deletion behaviour.

See [state lifetimes](glossary:state-lifetimes).

## Read AGENTS.md

`AGENTS.md` is short and binding. The parts newcomers miss:

- Honour the narrowest requested scope. If the request says edit planning Markdown, do not implement
  the feature.
- An unpatched fixture stays in the show. Only DMX output is suppressed.
- A stored empty group is not an absent group. A missing ID in a range is skipped.
- Programmer LTP and Playback HTP are distinct.
- Validate the exact interaction path the request describes. An adjacent click handler is not proof.
- Preserve unrelated changes in a dirty worktree.

## Both architectures are present

You will find two shapes for the same job.

| Converging on this | Being removed |
| --- | --- |
| `crates/application/src/<capability>/` | `crates/server/src/runtime/ws_*`, v1 routes |
| `apps/control-ui/src/features/<capability>/` | `api/ServerContext.tsx`, `features/server/` |
| typed command + typed event + narrow store | `useServer()`, broad bootstrap refresh, polling |

`useServer()` is a temporary facade scheduled for deletion. Adding a field to it moves backwards.

State: `docs/plans/refactoring-progress.md`. Target: `docs/plans/major-refactoring.md`.

## Verification

```sh
./test architecture
./test unit
./test e2e-api
./test e2e-ui
./test e2e tests/<focused-spec>.spec.ts
./test desktop-smoke
./build open
```

Use `cargo fmt`, not standalone `rustfmt`. Full reference:
`docs/engineering/build-and-test-commands.md`.

## Next

One Action End to End, then the component page for your area. Rust by Example if Rust is new.
