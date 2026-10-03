# Control ↔ Architect show synchronization

TL-543. An Architect document opened from a desk stays in step with that desk's show without an
explicit Save: Architect edits reach Control as typed sync transactions, and every Control commit
reaches the bound Architect on the sync feed. Control is authoritative; its show revision is the
cursor both sides agree on.

This document is the server-side contract (chunks 1–4). The Architect sync client, its journal and
mirror, the status chip and the conflict UI are later chunks.

## Owner decisions

| | Decision |
|---|---|
| D1 | Per-field compare-and-set applies **only** to the sync transaction route. Desk UI, OSC and object intents stay last-write-wins (api-rules §7 exception, below). |
| D2 | One multi-object route, `POST /api/v2/show-sync/transactions`, so each CAD gesture is atomic (api-rules §3 exception, below). |
| D3 | A partial conflict commits every non-conflicting field; only the conflicting fields await resolution. |
| D4 | Exactly-once request identities live in the portable show table `sync_applied_requests`, written in the same SQLite transaction as the edit. Part of the base schema; no schema-version bump. |
| D5 | When the bound show is not active on Control, Architect holds its edits Offline ("Show not active on Control"). The server never writes a show that is not active. |
| D6 | The `*.show.desk-source.json` sidecars are dropped, without migration, now that the binding store exists. Manual "Save to desk" stays until the end-to-end chunk passes. |
| D7 | This document and `docs/testing/31-control-architect-show-sync.md` precede the Playwright work. |
| D8 | Several Architects may bind the same show. Field compare-and-set and per-association request identities handle it; there is no lock. |

## Kind matrix

What an Architect may write through a sync transaction, what Control does with it, and whether a
change recompiles the desk's runtime. "Incremental" means the change is committed through the
active-show unit of work and every compiled projection is shared with the live runtime
(`synchronized_architect_kinds_share_every_compiled_projection` characterizes this).

| Kind | Synchronized | Architect writes | Control writes | Control runtime effect of a change |
|---|---|---|---|---|
| `patched_fixture` | Yes, as patch operations | Patch service (`PlanningDocument::patch_fixtures`) | Patch service, MVR and selective import | **Recompiles** the patch (`fixtures` dirty), reconciles media heads |
| `patch_layer` | Yes | `put_object` / `delete_object` | Object intent `POST /api/v2/patch/layers/{id}/update`, MVR, selective import | Incremental; validated as the typed `PatchLayer` family, published as `show_objects_changed` |
| `rig_attachment` | Yes | CAD (`cad.rs`, `cad/history.rs`) | Never | Incremental; not read at runtime |
| `cad_annotation`, `cad_drawing_tree`, `cad_underlay`, `cad_venue_groups` | Yes | CAD | Never | Incremental; not read at runtime |
| `fixture_note`, `fixture_visibility` | Yes | Session / CAD | Never | Incremental; not read at runtime |
| `media_server`, `media_source`, `media_surface`, `media_projector`, `media_fallback_asset` | Yes | Media intents (`mutate_objects_atomically`) | Selective import only | Incremental; Control's media runtime reads patched fixtures, not these objects |
| `visualizer_input` | Yes | `save_live_dmx_inputs` | Never | Incremental; read only by the Viz desk provider |
| `venue` | Yes (legacy standalone records) | Not written any more; read by the desk provider | Never | Incremental |
| `media_layout_request` | **No** — Architect-only: the replay ledger of the Architect's own media layout requests, meaningless on a desk | Media intents | Never | — |
| `stage_layout` | **No** — desk-only | Never (read only) | Stage layout intents | Recompiles `stage_layouts` |
| `route` | **No** — desk-only | Never (read only) | Output-route service | Recompiles `routes`, may terminate a route |
| Cue lists, playbacks, playback pages, presets, groups, dynamics, macros, schedules, timecode, PSN, user layouts, attribute configuration, control mappings | **No** — desk-only | Never | Desk services | Recompile their projections |
| Metadata `previs.*`, `architect.*` | Yes, as `set_metadata` | `save_paperwork_metadata` | Never (only `description`) | None |
| Metadata `name`, `show_id`, revision keys, `description` | **No** — library state | Rename | Library | — |

`truss` is not an object kind: trusses are visual-only patched fixtures, synchronized as patch
operations. The allowlist is `light_application::show_sync::SHOW_SYNC_OBJECT_KINDS`; anything
else is refused with `400 invalid`, never silently dropped.

## Identity and binding

- **Desk identity.** `settings.desk_identity` in the desk database, seeded once by the desk
  migration (`DeskStore::desk_identity`). It is published in `GET /api/v2/readiness` as
  `desk_identity` and in the DNS-SD record as the `desk_id` TXT key, next to `show_id`, the active
  show's UUID. Readers ignore TXT keys they do not know, so older and newer peers read each other.
- **Binding.** Architect keeps `SyncBinding {association_id, desk_identity, show_id, base_urls,
  desk_name, acknowledged_show_revision, created_at_millis}` at
  `<app data>/show-sync/<association_id>/binding.json`, located through
  `<app data>/show-sync/index.json`, which maps the working document's canonical path to its
  association. The binding is installation state: a copy of the file has none. A damaged index or
  binding opens the document standalone and reports why; it never deletes the damaged file.
- A transaction may name its desk in `origin.desk_identity`; a different installation answering on
  the same address refuses it with `409 desk_mismatch`.

## The transaction route

`POST /api/v2/show-sync/transactions` (`crates/light/adapters/headless/src/runtime/show_sync_http.rs`),
body `ShowSyncTransactionRequest` (`crates/light/contracts/wire/src/v2/show_sync.rs`), accepting
unknown properties through `TolerantJson`.

Operations: `update_object` (field edits), `create_object`, `delete_object` (with
`base_object_revision`), `update_patch_fixture` (field edits against the fixture's
`PatchFixtureInput` document), `create_patch_fixture`, `remove_patch_fixture` (with
`base_fixture_revision`), `retain_profile_revision` (a profile revision the desk's library lacks),
and `set_metadata`.

### Field compare-and-set

A field is an RFC 6901 pointer into the object body, with the value the client last saw (`base`)
and the value it wants. `null` and an absent field are the same state.

| Current value | Result |
|---|---|
| equals the wanted value | unchanged (a retry or a convergent edit) |
| equals `base` | applied |
| anything else | conflict: the field keeps the current value, and the outcome reports `base`, `mine`, `theirs` and the object revision |

Object-level rules: creating an identity that holds identical content is unchanged and holding
different content is an `object_exists` conflict; deleting an object whose revision moved past the
base is an `object_modified` conflict; editing a deleted object is an `object_deleted` conflict.
Arrays are best sent as one field holding the whole array.

Patch fixture fields are resolved against the stored fixture projected as `PatchFixtureInput`, then
committed through the patch capability's own planning, profile resolution, validation and scope
check, so a synchronized patch edit is exactly a desk patch edit.

### Commit

`ActiveShowService::synchronize` (`crates/light/src/show_sync`) runs inside the ordered active-show
lifecycle: under the one mutation gate it looks up the request identity, resolves every field
against the open document, stages object, patch and metadata writes into **one**
`PortableShowTransaction`, compiles it (incrementally when no patch changed), records the request
identity in the same transaction, and commits through `ServerActiveShowUnitOfWork`. Patch changes
publish `show_patch_changed` and typed families such as `patch_layer` publish
`show_objects_changed`, so desk surfaces update as for a desk edit. Profile planning runs inside
the gate here, unlike the desk patch route; a sync gesture rarely resolves a new profile.

### Outcome and errors

`200` with `ShowSyncTransactionOutcome {status: accepted | conflicted, replayed, show_revision,
patch_revision, applied, metadata, conflicts, event_sequence}`. A conflict is a normal outcome,
not an error: the non-conflicting fields have committed.

| Status | `kind` | Meaning | Client action |
|---|---|---|---|
| 400 | `invalid` | Desk-only kind, metadata key outside `previs.`/`architect.`, malformed pointer, empty or oversized transaction, body that fails its typed family | Reject the journal entry, show Error |
| 409 | `show_not_active` | The bound show is not active (`active_show_id` names the active one) | Hold edits Offline |
| 409 | `desk_mismatch` | A different desk installation | Error |
| 409 | `request_reused` | The identity was used for different content | Error (client bug) |
| 503 | `unavailable` (`retryable: true`) | Store busy, or the file moved under the unit | Retry the **same** request |

### Exactly once

The request identity `(association_id, request_id)`, a SHA-256 signature of the request content and
the stored outcome are inserted into `sync_applied_requests` in the commit's SQLite transaction. A
retry — from any session, after a lost reply or a Control restart — finds the row inside the gate
and returns the stored outcome with `replayed: true`. A transaction that changes nothing (every
field unchanged or conflicting) records no row; re-sending it re-resolves to the same no-op.
Retention is the newest 10 000 identities per show. A manual whole-file "Save to desk"
(`update_document`) carries the desk's rows into the replacement file, keeping the desk's row
where both files hold one, so a retry after a manual save still applies once. The table is created by the base schema for
new files and lazily by the first sync write for older files, so schema version 9 files still
open in older builds.

## The sync feed

The feed travels on the existing `/api/v2/events` stream as the opt-in `show_sync` topic: a client
subscribes with `filter.topics: ["show_sync"]` (usually together with
`capabilities: ["show"]`). Events of an opt-in topic match only subscriptions that name it, so a
subscription that does not opt in receives exactly the stream it received before
(`EventFilter::topics`, `ApplicationEvent::opt_in_topic`). The feed is also **published only while
an opted-in subscription is alive** (`EventBus::has_subscriber_for`), so a desk with no bound
Architect keeps one event — and one sequence number — per commit. A client that was not subscribed
for a while is not misled: the show revision is the cursor, and `previous_show_revision` or a gap
tells it to re-read snapshots (`show_sync_feed.rs`).

- **`show_sync_committed`** is published from `ServerActiveShowUnitOfWork::commit`, the seam every
  active-show commit passes: desk UI, OSC, object intents, patch, MVR and selective import, undo,
  and sync transactions. It carries `show_revision`, `previous_show_revision`, `patch_revision`,
  the sync `request_id`/`association_id` when a transaction produced it (echo suppression), the
  synchronized objects with bodies up to 32 KiB each and 256 KiB per event (larger bodies are
  announced with `body_omitted` and read by snapshot), synchronized metadata, profile revisions as
  references, and a count of desk-only changes.
- **`show_sync_gap`** replaces a commit that changed more than 256 synchronized objects or would
  withhold more than 32 bodies (`bulk_commit`); announces show open and rollback
  (`show_replaced`); and announces renames, re-uploads, description edits and overwrites of the
  active show (`out_of_band_write`). A commit that finds the show revision moved since the last
  announcement publishes the missing `out_of_band_write` gap first.
- The durable cursor is the show revision, not the event sequence, which restarts with the desk.
  A client whose mirror revision differs from `previous_show_revision`, or that receives a gap or
  a transport `gap`, re-reads its synchronized kinds (`GET /api/v2/objects/{kind}`,
  `GET /api/v2/patch`) before trusting its mirror.
- Live control — selection, programmer values, playback, Highlight, previews — never commits a
  show transaction and so never appears on the feed.

### Writers outside the unit of work

Audited for chunk 4; each either publishes a gap or cannot touch the active show:

| Path | Active show? | Feed |
|---|---|---|
| Show open, rollback, revision open, `update_document` on the active show (whole-file replace) | Yes | `show_replaced` gap from the `show_opened` / `show_rolled_back` notifications |
| Upload over the active show's name, rename, description, overwrite | Yes | `out_of_band_write` gap |
| Test-bench generic object put/delete | Only with `--test-bench` | Watermark gap at the next commit |
| Cue thumbnails, schedule occurrences | Yes, but not portable revisions | None needed |
| MVR apply, store-preload into a library show, revision copies, startup, defaults | Never the active show | None needed |
| File manager copy/move/rename/delete over the active show file | Yes — the shows root is an ordinary file root | `show_replaced` gap immediately (file identity compared around every file operation) |

## API-rule exceptions

- **§3 (per-object intent routes).** The sync route carries several objects because one CAD
  gesture — a rig attachment with its fixtures, a drawing with its tree — must commit atomically
  or not at all. It is the only multi-object write route and is used only by a bound Architect.
- **§7 (last-write-wins).** The sync route compares each field with the client's base instead of
  overwriting, because an Architect may have been offline for hours and a silent overwrite would
  lose a desk operator's work. Desk UI, OSC and object intents keep last-write-wins; a desk edit
  always lands, and an Architect edit to the same field becomes that Architect's conflict.

## Profiles

A profile revision carried by `retain_profile_revision` is stored in the show, not in Control's
fixture library: the portable show owns the profile revisions its fixtures reference, exactly as a
show loaded from a file does.

## Known limitations

- The file manager can replace or delete the active show's file without passing through
  activation; the runtime keeps running the old content until the show is reopened. The sync feed
  announces it, but the missing guard is a separate, pre-existing issue.
- Metadata conflicts compare whole values; an empty string reads as absent.
