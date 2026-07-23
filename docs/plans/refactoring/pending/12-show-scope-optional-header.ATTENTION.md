# 12 — Replace show-scoped v2 route segments with the optional show-guard header

## Context (api-rules §6, verified 2026-07-23)

Rule: no show-scoped routes; the show guard is an **optional request header** (desk sends
it → server verifies; absent → no check). v2 routes still carrying `{show_id}` as a path
scope (registration file:line):

| File | Route |
|---|---|
| `runtime/playback_v2.rs:31` | `/api/v2/shows/{show_id}/desks/{desk_id}/playback-actions` |
| `runtime/playback_topology_http.rs:28` | `/api/v2/shows/{show_id}/playback-topology/actions` |
| `runtime/virtual_playback_zones_http.rs:25,29` | (handled in chunk 07) |
| `runtime/programming_update_http.rs:28,32,36` | `/api/v2/shows/{show_id}/programming-update/{preview,targets,actions}` |
| `runtime/show_patch_http.rs:19,21` | `/api/v2/shows/{show_id}/patch[,/fixtures]` |
| `command_http/group_management_routes.rs:25` | `/api/v2/shows/{show_id}/groups/manage` |
| `command_http/group_recording_routes.rs:25` | `/api/v2/shows/{show_id}/groups/record` |
| `command_http/cue_recording_routes.rs:21` | `/api/v2/shows/{show_id}/cues/record` |
| `command_http/cue_transfer_routes.rs:22` | `/api/v2/shows/{show_id}/cues/transfer` |
| `command_http/cue_deletion_routes.rs:23` | `/api/v2/desks/{desk_id}/shows/{show_id}/cues/delete` |
| `command_http/preset_recall_routes.rs:23` | `/api/v2/shows/{show_id}/presets/recall` |
| `command_http/preset_recording_routes.rs:26` | `/api/v2/shows/{show_id}/presets/record` |

Legitimate (show id as **operand**, keep): `runtime/selective_import_http.rs:27,31,35` —
`{source_show_id}` is the operand; **however `{target_show_id}` is a scope** and should
become the loaded show + optional header like the rest. All routers assemble in
`runtime/http_router.rs:8-26`.

Desk-id segments: keep only where genuinely per-desk (playback actions target a desk's
control surface — likely legit; cue deletion's desk segment — verify what it scopes; drop
if it's only authentication, per the exclusion-zone precedent).

## Work

1. Define the header once (e.g. `X-Tosk-Show`) with a shared axum extractor: present →
   compare against the loaded show, mismatch → 409/412-style clear error; absent → no check.
   Document it in `docs/engineering/api-rules.md` §6.
2. Re-register each route without the `{show_id}` segment; handlers take the loaded show
   from state. One route family per commit inside the chunk; client transports
   (`PlaybackTopologyTransport`, `ProgrammerValuesTransport` URL builders,
   `client/programming.ts`, `PresetRecallTransport`, patch/show-patch transports), bench
   helpers (`apps/control-ui/e2e/bench`), and root tests migrate in the same commit.
3. Chunks 09–11 may have already moved desk traffic to WS for some of these — the HTTP
   forms still get de-scoped (integrator surface).
4. Send the header from the desk UI's transports (it has the show id from bootstrap) so the
   show-switch race stays covered.

## Definition of done

- No v2 route carries `{show_id}` as scope; selective-import keeps only the source-show
  operand; the header extractor is tested (match, mismatch, absent).
- Desk UI sends the guard header on all edits.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e-api
npm run test:e2e   # full suite gate
```

## Decisions

**DECISION NEEDED (small):** confirm the desk-id segment disposition for
`playback-actions` (keep: per-desk surface?) and `cues/delete` (drop?). Propose in the
result note if the maintainer hasn't answered by execution time — do not guess silently.
