# 10 — Encoder / programmer-values actions onto the WebSocket

## Context (api-rules §2, verified 2026-07-23)

Entering encoder values on fixtures is live control but goes over HTTP:
`apps/control-ui/src/api/ProgrammerValuesTransport.ts:92-94` POSTs to
`/api/v2/users/{user_id}/programmer-values/actions` (URL `:170`).

Server WS support partially exists already:
`crates/server/src/runtime/ws_programmer_handlers.rs:28-37` accepts programmer value frames
incl. `AttributeValue::Spread`. Wire: `crates/wire/src/v2/programming.rs`.
Replay windows for the HTTP form: `crates/application/src/programming/service/values_replay.rs`
(+ `preload_values_replay.rs`).

Follows the pattern established by chunk 09 — read that chunk's result note first.

## Work

1. Extend the WS dispatch to cover the full programmer-values action set the HTTP route
   accepts (`crates/server/src/command_http/values_routes.rs` / `values_wire.rs`), reusing
   the same application service.
2. Point `ProgrammerValuesTransport` (or its port,
   `ParameterValuesMutationPort`) at WS frames for desk-UI use; no re-send on failure —
   surface error + re-read.
3. Preload values (`preload_values_wire.rs`) ride along if the shapes are shared; otherwise
   note them for chunk 11.
4. HTTP route stays for integrators.

## Definition of done

- Encoder input, numeric-pad value entry, and range spreads (chunk 03's shape) reach the
  server as WS frames from the desk UI.
- No retry/replay of value frames client-side.

## Verification

```sh
cargo test -p server
npm run test:unit           # ProgrammerValuesTransport tests will need updating
npm run test:e2e-api
npm run test:e2e            # full suite gate — encoder + programmer scenarios
```

Manual: `npm run open`, spin encoders rapidly on a multi-selection; values track smoothly,
no dropped/duplicated steps.

## Decisions

None blocking. Sequence after 09 (pattern), 09b (direct encoder type removed — less
surface to migrate) and 03/03b (spread shape) to avoid double-touching the wire.

**Direction confirmed (maintainer, 2026-07-23): encoders are relative.** The decided
touch model lives in `docs/plans/Later/54-software-encoder-relative-controls.md`
("Decided touch interaction model"): discrete ±1/±10 step taps, center tap opens the
set-value modal, and hold-drag is a **rate-based** continuous change (displacement
controls speed, not position). Shape the WS value frames for this from the start:

- a **relative step** op (attribute, signed delta) for the tap zones and hardware
  encoder ticks;
- absolute set stays for the modal/command paths;
- for hold-drag, prefer streaming small relative-step frames at the current rate over a
  server-side start/stop-rate op — decide against the implementation, but do not ship a
  frame shape that only carries absolute values.

The encoder *UI* rework itself stays feature work (Later/54 / Next/65 — 65 also still
has its undecided command-line-fade question); this chunk only ensures the transport
contract doesn't have to be reworked when it lands.
