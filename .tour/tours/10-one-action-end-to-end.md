---
slug: one-action-end-to-end
title: "One Value: From Desk Input to DMX and Back"
components: [backend, engine, control-ui, programmer]
order: 20
---

# One Value: From Desk Input to DMX and Back

Follow `GROUP 1 AT 50 ENTER` from software or OSC keys to semantic fixture values, a DMX byte, and
authoritative feedback. Operator truth is in `docs/help/30-Programmer/01-command-line.md` and
`docs/help/30-Programmer/02-selecting-and-setting-values.md`; CROSS-001, OSC-002, and PROG-002 in
the root acceptance suite protect the path.

## 1. The keypress

`apps/light-desktop/src/components/control/` — numeric pad and command bar.

The keypad model is shared: `packages/light-controls/src/programmerKeypad.ts` defines the `SoftwareKey` union and
`numericPadLayout`, used by the control UI, the hardware app, and the Playwright bench.

The UI does not interpret `GROUP 1 AT 50`. It emits logical keys.

## 2. The optimistic write

`apps/light-desktop/src/features/programmingInteraction/commandLineWriter.ts`

A latest-wins writer gives immediate feedback, bounds slow writes to one in flight plus the newest
pending value, and waits for accepted writes before ENTER. The feedback is an overlay; authority
comes from the server.

## 3. The transport

```
WebSocket /api/v2/events, programmer.command_line.*  production desk UI
POST /api/v2/command-line/keys                       HTTP/integrator key twin
POST /api/v2/command-line/execute                    HTTP/integrator ENTER twin
```

The established desk WebSocket already owns ordering and desk/session context. HTTP uses the
optional `X-Tosk-Desk` context header. OSC sends the frozen
`/light/<desk>/programmer/<action>` address with true/false key phases. DTOs come from
`apps/light-desktop/src/api/generated/light-wire.ts`, which is generated and checked in.

## 4. The adapter

`crates/light/adapters/headless/src/command_http/`

Parses, authenticates, resolves the desk, builds an `ActionContext` (desk, user, session, source
surface, request ID, expected revision), and hands a typed command to the application.

No command grammar, no LTP rule, no group resolution live here.

## 5. The application boundary

`crates/light/src/programming/`

One authenticated, ordered boundary. It serializes typed commands per desk, so a UI keypress, an OSC
tap, and an HTTP request cannot interleave into an incoherent command line.

The contract is in `crates/light/src/action.rs`:

```rust
pub trait ApplicationCommand: Send + 'static {
    type Value: Send + 'static;
    const FAMILY: CommandFamily;
}
```

Each command declares its own result type. There is no process-wide command enum.

## 6. Selection, Groups, and Programmer LTP

`crates/light/domain/programmer/src/command_line/` parses the accumulated keys.
`crates/light/domain/programmer/src/groups.rs` expands Group 1 to its ordered logical heads.
`crates/light/domain/programmer/src/values.rs` applies the value under LTP.

None of this knows a WebSocket exists.

A missing Group ID inside a range is skipped; an explicitly stored empty Group remains distinct
from an absent Group. An unpatched fixture stays in the ordered selection and Programmer, but later
receives no physical output binding.

## 7. The outcome

`ActionOutcome<T>` carries the value, the authoritative revision, and an event sequence only if an
event was emitted (`crates/light/src/action.rs`).

A repeated On, a same-value master write, or a zero-time crossfade endpoint returns no-change:
nothing published, nothing persisted.

## 8. The event

`crates/light/src/event/bus.rs`

One semantic transition, one typed event, carrying a monotonic sequence, event time, source surface,
correlation identity, and enough identity to re-request the projection. The bus does not know about
WebSockets; adapters translate.

## 9. Into the engine

`crates/light/domain/engine/src/` — the values become a contribution. Arbitration resolves it against playback
contributions (HTP/LTP/ownership), transitions apply fade, delay, MIB, and masters, and the
Highlight overlay sits on top. The result is resolved semantic fixture values.

## 10. To the wire

Fixture projection maps semantic values onto DMX channels — mode, fine bytes, splits, multipatch,
logical heads.

`crates/light/adapters/headless/src/runtime/output_scheduler.rs` ticks: render, leave the domain locks, publish
automatic transitions, send encoded routes. Publishing after releasing the locks keeps a slow
subscriber from stalling a frame.

## 11. Back to the screen

`crates/light/adapters/headless/src/runtime/event_transport/adapter.rs` turns the typed event into a wire message on
`/api/v2/events`.

`apps/light-desktop/src/api/*Transport.ts` decodes and validates it. The feature store reconciles it
against the overlay from step 2, handling either arrival order, because the HTTP outcome and the
WebSocket event race. If the sequence had a gap, the store repairs from a snapshot.

A revision conflict repairs from authority and lets the deliberate action reapply. A replay returns
the stored outcome without duplicating the value, history, or event. Compact and widened JSON
spellings of the same Rust `f32` are compared by their `f32` value, so wire formatting cannot cause
a false repair.

## Same command, three surfaces

`tests/support/operator/programmer.ts` drives the same command through:

```ts
{ via: "command-line", api }   // HTTP
{ via: "software", page }      // DOM clicks
{ via: "osc", api, hardware }  // UDP OSC with true/false phases
```

Steps 5 to 11 are identical for all three. Only 1 to 4 differ. `pairedScenario` in
`tests/bench/core/pairedScenario.ts` keeps that true.

## Exercises

1. Find where `GROUP` becomes `[GRP]`, and where `[GRP][GRP]` becomes DEGRP.
2. Send the same value twice. Confirm the second returns no-change and publishes nothing.
3. Kill the WebSocket mid-gesture. Find the gap detection and the snapshot repair.
4. Follow one 16-bit Pan value through the fixture profile's coarse/fine encoding without running
   the desk.
