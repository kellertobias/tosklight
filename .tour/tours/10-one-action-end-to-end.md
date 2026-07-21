---
slug: one-action-end-to-end
title: One Action, End to End
components: [backend, engine, control-ui, programmer]
order: 20
---

# One Action, End to End

Follow `GROUP 1 AT 50 ENTER` from a keypress to a DMX byte and back to the screen. Everything else
in the system is a variation on this path.

## 1. The keypress

`apps/control-ui/src/components/control/` — numeric pad and command bar.

The keypad model is shared: `apps/shared/programmerKeypad.ts` defines the `SoftwareKey` union and
`numericPadLayout`, used by the control UI, the hardware app, and the Playwright bench.

The UI does not interpret `GROUP 1 AT 50`. It emits logical keys.

## 2. The optimistic write

`apps/control-ui/src/features/programmingInteraction/commandLineWriter.ts`

A latest-wins writer gives immediate feedback, bounds slow writes to one in flight plus the newest
pending value, and waits for accepted writes before ENTER. The feedback is an overlay; authority
comes from the server.

## 3. The transport

```
POST /api/v2/desks/{desk_id}/command-line/keys      one logical key, press or release phase
POST /api/v2/desks/{desk_id}/command-line/execute   ENTER
```

DTOs come from `apps/control-ui/src/api/generated/light-wire.ts`, which is generated and checked in.

## 4. The adapter

`crates/server/src/command_http/`

Parses, authenticates, resolves the desk, builds an `ActionContext` (desk, user, session, source
surface, request ID, expected revision), and hands a typed command to the application.

No command grammar, no LTP rule, no group resolution live here.

## 5. The application boundary

`crates/application/src/programming/`

One authenticated, ordered boundary. It serializes typed commands per desk, so a UI keypress, an OSC
tap, and an HTTP request cannot interleave into an incoherent command line.

The contract is in `crates/application/src/action.rs`:

```rust
pub trait ApplicationCommand: Send + 'static {
    type Value: Send + 'static;
    const FAMILY: CommandFamily;
}
```

Each command declares its own result type. There is no process-wide command enum.

## 6. The domain

`crates/programmer/src/command_line/` parses the accumulated keys.
`crates/programmer/src/groups.rs` expands Group 1 to its ordered logical heads.
`crates/programmer/src/values.rs` applies the value under LTP.

None of this knows a WebSocket exists.

## 7. The outcome

`ActionOutcome<T>` carries the value, the authoritative revision, and an event sequence only if an
event was emitted (`crates/application/src/action.rs`).

A repeated On, a same-value master write, or a zero-time crossfade endpoint returns no-change:
nothing published, nothing persisted.

## 8. The event

`crates/application/src/event/bus.rs`

One semantic transition, one typed event, carrying a monotonic sequence, event time, source surface,
correlation identity, and enough identity to re-request the projection. The bus does not know about
WebSockets; adapters translate.

## 9. Into the engine

`crates/engine/src/` — the values become a contribution. Arbitration resolves it against playback
contributions (HTP/LTP/ownership), transitions apply fade, delay, MIB, and masters, and the
Highlight overlay sits on top. The result is resolved semantic fixture values.

## 10. To the wire

Fixture projection maps semantic values onto DMX channels — mode, fine bytes, splits, multipatch,
logical heads.

`crates/server/src/runtime/output_scheduler.rs` ticks: render, leave the domain locks, publish
automatic transitions, send encoded routes. Publishing after releasing the locks keeps a slow
subscriber from stalling a frame.

## 11. Back to the screen

`crates/server/src/runtime/event_transport/adapter.rs` turns the typed event into a wire message on
`/api/v2/events`.

`apps/control-ui/src/api/*Transport.ts` decodes and validates it. The feature store reconciles it
against the overlay from step 2, handling either arrival order, because the HTTP outcome and the
WebSocket event race. If the sequence had a gap, the store repairs from a snapshot.

## Same command, three surfaces

`tests/support/operator/programmer.ts` drives the same command through:

```ts
{ via: "command-line", api }   // HTTP
{ via: "software", page }      // DOM clicks
{ via: "osc", api, hardware }  // UDP OSC with true/false phases
```

Steps 5 to 11 are identical for all three. Only 1 to 4 differ. `pairedScenario` in
`apps/control-ui/e2e/bench/pairedScenario.ts` keeps that true.

## Exercises

1. Find where `GROUP` becomes `[GRP]`, and where `[GRP][GRP]` becomes DEGRP.
2. Send the same value twice. Confirm the second returns no-change and publishes nothing.
3. Kill the WebSocket mid-gesture. Find the gap detection and the snapshot repair.
4. Find something in `crates/server/src/runtime/ws_*` doing this the old way, and note the
   difference.
