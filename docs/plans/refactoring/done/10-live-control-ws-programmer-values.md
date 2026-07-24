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
5. Adopt the refactoring-shaped seam required by
   [`Next/71`](../../Next/71-attribute-registry-and-activation-groups.md): one incoming
   programmer-value intent must be able to produce one atomic mutation containing values
   for multiple fixture/head attributes. The caller sends the initiating absolute-set or
   relative-step intent; it must not expand activation groups locally. The authoritative
   application service performs any expansion from one consistent current-value snapshot
   and commits the initiating value plus linked captures as one programmer revision and
   one Undo step.

   This chunk establishes the operation boundary and transport shape, but preserves
   current behavior: with no activation policy configured, an intent changes only its
   requested attribute. It does **not** implement the attribute registry, recommended
   groups, Desk Setup UI, persisted activation configuration, or migration from
   `Next/71`. Those remain feature work. Do not bake fixed Color, Position, or Media
   membership into the WS frame or client.

## Definition of done

- Encoder input, numeric-pad value entry, and range spreads (chunk 03's shape) reach the
  server as WS frames from the desk UI.
- No retry/replay of value frames client-side.
- Absolute-set and relative-step intents share a server-authoritative mutation boundary
  capable of atomically returning/applying more than one attribute value without the
  transport knowing why those values were linked.
- Current unconfigured behavior is pinned by tests: one initiating intent changes only
  the requested attribute and creates one Undo step. Application-service coverage also
  proves that an injected multi-attribute expansion is committed and undone atomically,
  without implementing the `Next/71` configuration feature.

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
touch model now lives in
`docs/plans/Next/00-programmer-relative-encoders-and-fade-time-scope.md`: discrete
±1/±10 step taps, center tap opens the
set-value modal, and hold-drag is a **rate-based** continuous change (displacement
controls speed, not position). Shape the WS value frames for this from the start:

- a **relative step** op (attribute, signed delta) for the tap zones and hardware
  encoder ticks;
- absolute set stays for the modal/command paths;
- for hold-drag, prefer streaming small relative-step frames at the current rate over a
  server-side start/stop-rate op — decide against the implementation, but do not ship a
  frame shape that only carries absolute values.

The encoder *UI* rework itself stays feature work in `Next/00`; this chunk only ensures
the transport contract doesn't have to be reworked when it lands.

**Activation groups are the same kind of transport-boundary concern.** Implement their
atomic programmer-mutation seam in this chunk because programmer value actions are
already being consolidated here. Implementing their operator-visible behavior or
desk-local configuration here would mix a substantial new feature and persistence
migration into the transport refactor, so that remains in `Next/71`.

## Result

- Added correlated `programmer.values.action` frames to the authenticated live WebSocket
  and moved normal desk Programmer value writes to that single-send route. The HTTP
  endpoint remains available to integrators; Preload remains distinct for chunk 11.
- Added server-authoritative absolute-set and signed relative-step intents. Hardware
  encoder ticks now send relative deltas, while numeric and range entry send absolute
  values or spreads without client-side expansion.
- The application service plans each intent from one frozen value environment, including
  fixture defaults where no live contribution exists, and commits the resulting mutation
  vector as one revision and one Undo step. Production activation links remain empty;
  application tests prove an injected multi-attribute link expands, replays, and undoes
  atomically.
- Verified `cargo test -p light-server` (435 passed, 1 ignored plus 14 benchmark tests),
  `npm run test:unit` (276 files, 1996 tests), `npm run test:e2e-api` (86 passed,
  1 skipped), the focused hardware-encoder suite (4 passed), and `npm run test:e2e`
  (285 passed, 11 skipped).
