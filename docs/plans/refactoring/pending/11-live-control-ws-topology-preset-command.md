# 11 — Remaining desk-UI live-control HTTP calls onto the WebSocket

## Context (api-rules §2, verified 2026-07-23)

After chunks 09–10, these desk-UI live-control surfaces still POST over HTTP:

- **Virtual/touch playback triggers (topology actions):**
  `apps/control-ui/src/api/PlaybackTopologyTransport.ts:51-58` →
  `POST /api/v2/shows/{show_id}/playback-topology/actions`
  (server: `playback_topology_http.rs:28`; replay `playback_topology/replay.rs:59`).
- **Preset recall:** `apps/control-ui/src/api/PresetRecallTransport.ts:37-41` →
  `POST /api/v2/shows/{show_id}/presets/recall`
  (server: `command_http/preset_recall_routes.rs:23`).
- **Command line replace:** `apps/control-ui/src/api/client/programming.ts:36-49` →
  `POST /api/v2/desks/{desk_id}/command-line` (wired at
  `api/ServerProgrammingProviders.tsx:254`).
- **Programmer selection actions (keys):** `programming.ts:51-68` →
  `POST /api/v2/desks/{desk_id}/programming-selection/actions`.
- **Programmer priority / speed-group / output-runtime actions:**
  `ProgrammerPriorityTransport`, `SpeedGroupRuntimeTransport`, `OutputRuntimeTransport`
  (all POST in `apps/control-ui/src/api/*Transport.ts`; server replay:
  `programming/service/priority_replay.rs`, `speed_group/replay.rs:87-133`,
  `output_runtime/service.rs:239`).

Note the WS `command()` path already carries several of these action families
(`programming.ts:77-153`: `group.select`, `selection.set`, `programmer.command_line`, …) —
part of this chunk is de-duplicating: one action family should have **one** desk-UI path.

## Work

1. Inventory which of the above already have a WS twin via `transport.command`; migrate the
   HTTP callers to it instead of adding new frames where a twin exists.
2. Add WS frames for the families without one (topology actions, preset recall, priority,
   speed-group, output-runtime), same pattern as chunk 09.
3. Remove the now-dead desk-UI HTTP call sites; HTTP routes stay for integrators.
4. No client re-send anywhere; failure → error + re-read.

## Definition of done

- Every rule-2 live-control example (select group, trigger preset, trigger virtual
  playback, touch-playback buttons, encoder values, programmer keys, GO/flash/fader) goes
  over the WS from the desk UI.
- Duplicated HTTP/WS client paths for the same action family are collapsed to one.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e   # full suite gate — preset, topology, speed-group, output scenarios
```

## Decisions

None. Do this after 09/10; it is deliberately the mop-up chunk — split it further at
execution time if any single family (likely topology) turns out large; put the split files
into `pending/` as `11a-…`, `11b-…` and note the split in the result.
