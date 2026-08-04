---
slug: orientation
title: Orientation
order: 10
---

# Orientation

What ToskLight is, how the layers fit, and the rules that are enforced rather than suggested.

## Run it

```sh
npm run dev
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
| Adapters | `crates/light/adapters/headless/`, UI transports | Parse, authenticate, translate. No business rules. |
| Application | `crates/light/` | Transport-independent use cases. Owns state, exposes commands and immutable projections. |
| Domain | `crates/{core,fixture,playback,programmer,output,control,show,media,mvr}` | No HTTP, WebSocket, SQLite, or Tauri. |
| Wire | `crates/light/contracts/wire/` | Leaf. Versioned DTOs only. |
| Frontend | `apps/light-desktop/`, `apps/light-hardware-controls/` | Renders authoritative projections. |

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

## The current extension shape

One vertical capability owns its application service, wire DTOs, server adapter, semantic events,
frontend projection, and focused tests. No served `/api/v1` route or production `useServer()`
consumer remains. The contract-only `api/ServerContext.ts` exists for legacy test mocks and is not
an extension point.

The summary is `REFACTORING-SUMMARY.md`; developer architecture lives in this Code Safari and
agent-facing operational contracts are under `docs/engineering/`.

## Verification

```sh
npm run test:architecture
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
npm run test:desktop-smoke
npm run open
```

Use `cargo fmt`, not standalone `rustfmt`. Full reference:
`docs/engineering/build-and-test-commands.md`.

## Next

One Value: From Desk Input to DMX and Back, then the component page for your area. Rust and Tauri
for TypeScript Developers if Rust is new.
