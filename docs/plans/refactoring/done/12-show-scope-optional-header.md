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

**Desk-id segments — DECIDED (maintainer, 2026-07-23, recorded in api-rules §6):** a
desk id stays in the path only where the desk is the **operand** (editing/reading the
desk object itself, e.g. `PUT /control-desks/{id}`). Everywhere the desk is merely
*context* — current-page playback actions (`…/desks/{desk_id}/playback-actions`), cue
deletion (`…/desks/{desk_id}/shows/{show_id}/cues/delete`), command-line and
programming-selection routes (`/api/v2/desks/{desk_id}/command-line`,
`…/programming-selection/actions`) — the segment is **removed** and replaced by an
optional desk-context header; absent header → the main/controlling desk (deterministic
default; usually there is exactly one desk). WS frames need no header — the session
carries the desk.

## Work

1. Define **both context headers** once with shared axum extractors and document them in
   `docs/engineering/api-rules.md` §6:
   - show guard (e.g. `X-Tosk-Show`): present → compare against the loaded show,
     mismatch → clear 409/412-style error; absent → no check;
   - desk context (e.g. `X-Tosk-Desk`): present → resolve that desk, unknown desk →
     clear 4xx; absent → the main/controlling desk (define the deterministic default:
     the single existing desk, else the designated main desk).
2. Re-register each route without the `{show_id}` segment **and without desk-context
   segments** (playback-actions, cues/delete, command-line, programming-selection);
   handlers take the loaded show from state and the desk from the header/default. One route family per commit inside the chunk; client transports
   (`PlaybackTopologyTransport`, `ProgrammerValuesTransport` URL builders,
   `client/programming.ts`, `PresetRecallTransport`, patch/show-patch transports), bench
   helpers (`apps/control-ui/e2e/bench`), and root tests migrate in the same commit.
3. Chunks 09–11 may have already moved desk traffic to WS for some of these — the HTTP
   forms still get de-scoped (integrator surface).
4. Send the header from the desk UI's transports (it has the show id from bootstrap) so the
   show-switch race stays covered.

## Definition of done

- No v2 route carries `{show_id}` as scope and no v2 route carries a desk segment where
  the desk is context; selective-import keeps only the source-show operand; both header
  extractors are tested (match, mismatch/unknown, absent → default).
- Desk UI sends the show-guard header on all edits and the desk header where it targets
  a specific desk.

## Verification

```sh
cargo test -p server
npm run test:unit
npm run test:e2e-api
npm run test:e2e   # full suite gate
```

## Decisions

Decided (see the desk-segment paragraph above and api-rules §6). No open decisions
remain in this chunk.

## Result

Split at execution time so the shared extractors, desk-context routes, show-action
routes, and patch/import routes can be reviewed and gated without one cross-cutting
mega-commit:

- `12a-context-headers-and-desk-routes.md`
- `12b-show-action-route-descope.md`
- `12c-patch-selective-import-route-descope.md`

No production behavior changed in this parent chunk; the three ordered child chunks
retain the complete definition of done above.
