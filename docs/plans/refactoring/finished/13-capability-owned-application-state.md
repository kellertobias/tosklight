# Capability-owned Application State

## Goal

Replace the large raw-lock `AppState` container with capability-owned runtime resources and narrow
service/query ports. This is the deliberate wholesale completion of the state-ownership refactor,
performed capability by capability rather than as one untestable rewrite.

Estimated effort: 1–2 Codex days after plans 06 and 07 establish mutation and event owners.

## Queue dependency

Doing. Plans 06, 07a, 07b, and the Storybook lane are complete, so the required active-show,
Highlight, event, and frontend ownership boundaries are available.

## Required work

1. Inventory every `AppState` field, lock, owner, mutation path, query path, lifecycle, and
   cross-capability dependency.
2. Define capability resources for active show, Programmer, Playback, Highlight, output,
   configuration, fixtures, sessions, media, events, and replay state.
3. Expose commands and immutable projections; do not expose raw `Mutex`, `RwLock`, registries, or
   stores to adapters.
4. Move lifecycle/startup/shutdown ownership into each capability and keep only a small composition
   root that wires ports together.
5. Migrate one capability at a time with temporary accessors that can only shrink.
6. Add an architecture ratchet preventing new raw state/lock access and remove `desk_lock()`
   compatibility access once callers migrate.
7. Verify lock ordering, cancellation, shutdown, event backpressure, and no lock held across
   `.await`.

## Acceptance and verification

- HTTP, WebSocket, OSC, persistence, and background adapters depend on capability ports.
- One capability's tests can instantiate its resource without the whole server state bag.
- The composition root contains wiring, not business operations.
- Concurrency, deadlock-sensitive, startup/shutdown, API, OSC, event, architecture, and full
  workspace tests pass.

## Ownership inventory

The former flat `AppState` fields are assigned as follows. The field lists are the complete
pre-refactor inventory, including test-only state.

| Capability | Former fields now owned | Command/query boundary | Lifecycle and dependencies |
|---|---|---|---|
| Installation | `desk`, `fixture_library`, `data_dir`, `configuration`, `desk_token` | Typed desk, user, screen, playback-layout, setting, token, fixture-definition, and fixture-profile commands/projections | Opened during startup; supplies immutable installation projections to every runtime capability. |
| Sessions | `sessions`, `session_clients`, `file_input_contexts` | Session authentication/binding, deterministic desk resolution, and file-input ownership commands | Created at startup; disconnect removes client and file-input ownership before Programmer/Highlight cleanup. |
| Programming | `programmers`, `programming`, `command_history` | Programmer commands/projections and desk-operation execution; no registry or lock escapes | Depends on sessions, active-show mutation ports, Highlight, playback, and events. Desk operations serialize per desk without holding a lock across `.await`. |
| Playback | `playback_service`, `playback_topology`, `playback_telemetry` | Playback commands, topology projections, and telemetry sampling | Depends on active-show data, Programmer capture, output timing, and events. |
| Highlight | `highlight`, `highlight_service`, `patch_preview_highlights` | Application-service commands, immutable state, output fixture projection, and patch-preview commands | Cleared with show/session lifecycle; depends on Programming selection and active-show fixture data. |
| Output | `output_runtime_service`, `speed_group_service`, `engine`, `output_health`, `output_rate`, `output_control`, `timecode_router`, `network_output`, `output_sequences`, `manual_clock`, `test_clock_lock`, `speed_groups`, `sound_capture_owners`, and the four persistence test controls | Output runtime, render options, DMX overrides/frames, health, timecode, route sequences, speed-group, sound-capture, and bounded test-clock commands/projections | Output task ownership is supervised and awaited at shutdown; depends on active-show, Playback, Highlight, Installation, and Integrations. |
| Active show | `activation_lock`, `active_show`, `active_show_document`, `active_show_backup_checkpoint`, `active_show_error`, `active_show_service`, `show_patch`, `selective_show_import`, `mvr_imports`, and four test lifecycle pauses | Opaque mutation permit, current-show/document projections, cache/checkpoint commands, patch/import services, staged MVR commands, and capability-owned portable-show repository | Owns the ordered in-memory/WAL/revision/undo/event/backup boundary and preserves portable/legacy show behavior. |
| Events | `application_events`, `audit_events`, `event_revision` | Publish, subscribe, replay, audit, and revision operations | Subscription backpressure stays in `EventBus`; shutdown is coordinated by Lifecycle. |
| Integrations | `matter_bridge`, `matter_transport`, `osc_subscribers`, `osc_cue_record_suppression`, `osc_feedback`, `osc_feedback_capture` | Matter and OSC registration, state, suppression, feedback, and capture commands/projections | Long-lived Matter work is owned by the runtime capability supervisor and cancelled/awaited at shutdown. |
| Media | `media_cache`, `media_status` | Thumbnail lookup/store/invalidation and media-server status commands/projections | Cache/status lifetime follows the process and fixture invalidation. |
| Replay | `show_library_replay`, `fixture_library_replay`, `show_object_replay`, `show_object_intent_replay`, `preset_generation_replay`, `screen_configuration_replay`, `control_desk_configuration_replay`, `desk_management_replay`, `stage_layout_replay`, `virtual_playback_zones_replay` | Per-operation async lookup/insert ports preserving request fingerprints and replay metadata | Independent cache locks avoid cross-route blocking; no replay lock is held across an awaited mutation. |
| Lifecycle | `shutdown` | Cancellation projection and shutdown request | Runtime supervisors own cancellation roots and join handles, propagate task errors/panics, and are awaited on every server exit path. |

`AppState` is now only the twelve-resource composition root. Startup code may construct concrete
adapters, but operational HTTP, WebSocket, OSC, persistence, and background code consumes the
capability commands and projections above.

## Result

### Changes

- Replaced the 69-field raw-lock `AppState` with twelve capability resources covering
  installation, sessions, programming, playback, Highlight, output, active show, events,
  integrations, media, replay, and lifecycle.
- Made capability internals private and migrated HTTP, WebSocket, OSC, persistence, and background
  adapters to named commands and immutable projections. Removed the `desk_lock()` compatibility
  escape and retained per-desk serialization through the Programming capability.
- Moved the active-show repository behind the active-show capability while preserving its portable
  show, WAL, revision, migration, backup, and selective-import behavior.
- Added supervised ownership for output, control-input, Matter, and timed-pulse tasks, including
  cancellation, join/error propagation, and a bounded timed-task queue with admission rollback.
- Added `tools/capability-state-boundaries.mjs` and its nine-test architecture ratchet. The current
  debt inventory is empty for raw `AppState` fields, adapter access, public capability APIs,
  resource escapes, and unsupervised task ownership.

### Verification

- `node --test tools/capability-state-boundaries.test.mjs` — 9 passed.
- `node tools/capability-state-boundaries.mjs` — passed with zero recorded debt.
- `cargo fmt -p light-headless-runtime -p light-application -- --check` — passed.
- `cargo clippy -p light-headless-runtime --all-targets -- -W clippy::await_holding_lock` — passed;
  existing non-blocking warnings remain.
- `cargo test -p light-headless-runtime` — 502 passed, 1 ignored.
- `npm run test:e2e-api` — 21 passed.
- `cargo test --workspace` — passed across the complete Rust workspace.
- `npm run test:unit` — the Plan 13 architecture violation is resolved; the gate still reports four
  unrelated concurrent frontend CSS ownership duplicates in `hardware-dense.css`,
  `playback-colors.css`, and `window-kit.css`.
- `git diff --check` — passed.

### Limitations

- This refactor deliberately changes ownership and dependency boundaries, not HTTP/WebSocket/OSC
  contracts, persisted show schemas, or operator behavior.
- Existing Rust warnings and the four unrelated frontend CSS ownership failures were not changed
  or staged with this plan.

### Commit

- `refactor(runtime): own application state by capability` (this commit)
