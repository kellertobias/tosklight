# 09 — Desk UI playback actions onto the WebSocket; kill the HTTP retry re-send

## Context (api-rules §2 violations, verified 2026-07-23)

The desk UI's highest-traffic live controls (GO / flash / fader / master on the playback
pool, programmer keyboard flash/swap) go over **HTTP**:

- `apps/control-ui/src/features/playbackRuntime/actionWriter.ts` — `applyAction` POSTs to
  `/api/v2/shows/{show_id}/desks/{desk_id}/playback-actions`
  (URL built in `apps/control-ui/src/api/client/playback.ts:79-90`).
- Consumers: fader bank `components/control/playbackFaderBank/heldActions.ts:66-67`,
  keyboard flash `components/control/commandLine/keyboardFlashActions.ts:1,33`.
- **Worse:** `actionWriter.ts:298-314` (`applyWithRetry`) re-sends on failure — directly
  forbidden for live control ("a caller that sends GO twice meant GO twice"; the desk gets
  its safety from ordered WS frames, and after a reconnect it re-reads state, never replays).

A request_id-correlated WS action mechanism already exists and is in production use:
`apps/control-ui/src/api/client/runtime.ts:116-141` (`command()`, pending-map correlation,
5 s timeout) over the `/api/v2/events` socket (`runtime.ts:94-106`), exposed as
`transport.command` (`client/transport.ts:11-17`). Server side: `ws_dispatch.rs:134,238-331`
already threads request ids; `ws_preload_handlers.rs:220,248-304` shows the pattern for a
whole action family. The playback HTTP route stays for integrators (rule 2: every WS action
also exists as a plain HTTP URL) — this chunk moves the **desk UI** onto WS frames.

## Work

1. Add a WS action frame family for playback actions (mirror the wire shapes of
   `crates/wire/src/v2/playback.rs` used by `playback_v2.rs:31`), dispatched in
   `crates/server/src/runtime/ws_dispatch.rs` to the same application service the HTTP
   route calls (`crates/application/src/playback/service.rs` — replay window `:323-345`
   already exists; WS frames don't re-send, so replay applies only to the HTTP form).
2. Point `PlaybackRuntimeActionWriter.applyAction` at the WS frame; **delete
   `applyWithRetry`** — on failure/timeout, surface the error and re-read authoritative
   state (the store already refetches on repair).
3. Keep the HTTP route serving (integrator surface); bench/tests may keep using it.
4. Confirm reconnect behavior: no queued frames replay after reconnect (grep the writer/
   transport for any queue-flush on reopen).

## Definition of done

- Fader bank + keyboard flash/swap drive playback actions over WS frames.
- `applyWithRetry` is gone; no client code re-sends a live-control frame.
- PBK/playback and PLAYBACK-SELECT scenarios green.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e -- tests/29-playback-telemetry.spec.ts
npm run test:e2e   # full suite gate — watch PBK-005, PLAYBACK-SELECT-001, preload contracts
```

Manual: `npm run open`, hammer GO/flash/faders, pull the network briefly (kill server,
restart) and confirm no action replays on reconnect.

## Decisions

None — transport split is decided (api-rules §2/§4-transport). Chunks 10 and 11 repeat
this pattern for other action families; land this one first to establish it.
