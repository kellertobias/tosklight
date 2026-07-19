# State Ownership

Every state field belongs to exactly one lifetime. Before adding a field, record its owner,
persistence location, migration, reconnect, restart, Save As, and deletion behavior. If two rows
seem applicable, split the state into a portable definition and a runtime projection rather than
giving one field two owners.

## Lifetime matrix

| Lifetime | Current owner and storage | Persistence and migration | Reconnect | Process restart | Save As | Deletion |
| --- | --- | --- | --- | --- | --- | --- |
| **Portable show** | `light-show`; `.show` SQLite files opened by `ShowStore`. Includes fixtures, deduplicated profile revisions, Groups, Presets, Cuelists, Playbacks, pages, routes, layouts, patch data, and unknown objects/fields. | `crates/show/src/portable/` owns raw lossless documents and atomic revisions. `crates/application/src/show_compiler/` migrates and compiles candidates before installation. A typed edit must retain unowned JSON. | Rehydrate the active-show projection and resume scoped events after its snapshot cursor. Client caches are never authoritative. | The indexed active show is reopened, migrated, validated, compiled, and installed. Invalid data enters recovery without overwriting the source. | Copy the complete portable file, referenced fixture-profile revisions, and portable managed assets. Assign the copy its own show identity. Never copy desk users, screens, devices, sessions, or live runtime. | Object deletion is one validated active-show transaction and must reconcile dependent runtime. Deleting a non-active show removes its file and named revisions; the active show is protected from deletion. |
| **Desk installation** | `light-show::DeskStore`; `<data-dir>/desk.sqlite`, plus installation-owned fixture-library and configured local asset/cache paths. Includes users, client/control desks, screens, server/input/output configuration, show index, and active-show choice. | `crates/show/src/desk/migration.rs` migrates the desk schema. Settings with JSON bodies have their own typed decode/default validation. Never store this data inside a portable show. | A new authenticated session queries current desk projections. The client identity may regain its remembered control desk; reconnect does not copy desk state into the browser. | Reopen `desk.sqlite`, load validated configuration, resolve the active show, and reopen the fixture library. | Not copied. The destination show remains on the same installation unless explicitly exported and moved. | Use the owning desk operation: delete a screen, remove a client desk, delete a user, remove a show index entry, or reset one setting. Cascading removal must be explicit and must not delete portable shows merely because a screen or user is removed. |
| **Desk interaction** | Desk-scoped application/domain state. Includes shared unfinished command line and target, selection gesture context, current desk/page addressing, Shift/press state, and short interaction locks. `ProgrammerRegistry` currently owns command and selection contexts attached by desk. | Persist only an explicitly promised recovery projection. The current command-line/selection projection is checkpointed with the Programmer session; press phases, gesture-open flags, locks, and input ownership are not durable. Schema changes need a compatible checkpoint decode. | Surfaces attached to one desk share the authoritative context. Reconnect reads the desk/command projection; it does not replay local key presses. Sequence gaps require a snapshot. | Restore only checkpointed fields, reset partial physical gestures, held keys, locks, and transport ownership deterministically, and mark restored sessions disconnected. | Never copied. A copied show opens with the destination desk's interaction context. | Clear when its owning desk/context is removed or when the documented command/gesture reset runs. Closing one surface must not clear state still owned by another surface on the same desk. |
| **User Programmer** | `light-programmer::ProgrammerRegistry`; one semantic Programmer per user, addressable by attached sessions. Includes ordered selection expression, fixture/Group values, timing, modes, Preload, and mutation-only undo/redo. | Serialized as `ProgrammerState` in the `desk.sqlite` session checkpoint. `transient_values` and Highlight are explicitly skipped. Decode older checkpoints with defaults; never move Programmer content into the show until Record/Update creates a portable object. | A session for the same user reattaches to the user Programmer while keeping desk interaction context separate. Query an authoritative Programmer projection after a gap. | Restore checkpoints as disconnected, rebuild runtime ordering counters, and attach a newly authenticated session by user identity. | Never copied, including Preload and undo/redo. | Ordinary Clear removes the documented Programmer layers but not the user. Session/user retention policy owns checkpoint removal. Deleting a show must not silently delete a user's Programmer; show replacement must reconcile invalid references explicitly. |
| **Connection or session** | `light-server` session/auth maps and transport adapters. Includes live session/client identity, authentication, WebSocket/OSC subscriptions, delivery cursor, file-input ownership, and negotiated desktop/screen role. | Live connections, sockets, queues, and subscriptions are not persisted. `desk.sqlite` session rows currently retain a token-labelled Programmer recovery checkpoint, but startup does not reinstate the row as a live authenticated connection. | Authenticate or resume according to the session contract, rebind the desk interaction context, hydrate projections, then subscribe after the snapshot cursor. Only the primary frontend owns session creation/destruction; secondary screens borrow it. | All live connections end. Event-bus sequences, delivery queues, socket subscriptions, and input claims restart empty. | Never copied. | Closing a session releases live subscriptions, file input, preview Highlight, and connection ownership, then marks its Programmer disconnected. Logout, expiry, client removal, and retained checkpoint deletion are separate explicit operations. |
| **Transient runtime** | Playback, Engine, Highlight, Control, Output, media, schedulers, and adapter supervisors. Includes active transitions, Chaser/FOLLOW position, Highlight overlay, output health, frames, queues, sockets, in-flight imports, and task cancellation. | Runtime is not portable. Only named recovery projections are checkpointed in desk settings: current active Playback state and global Output runtime are examples. Highlight, output health, frames, queues, locks, timing samples, and in-flight work are never persisted. | Read immutable runtime projections and subscribe only to displayed identities. Do not infer state from missed events or browser-local optimistic values. | Restore documented checkpoints against the newly compiled show; otherwise start from a deterministic safe state. Recreate sockets, queues, clocks, and health. First output after restart must come from the coherent restored generation. | Never copied. Portable definitions may be copied, but running instances, phases, output health, and pending work are not. | Release on stop, owning-object deletion, show replacement, session/context teardown, or adapter shutdown as applicable. Deleting a definition must not leave an orphan runtime instance. |

## Required declaration for new state

Put this information beside the model or in the feature's engineering contract before adding
persistence:

```text
State:
Lifetime: portable show | desk installation | desk interaction |
          user Programmer | connection/session | transient runtime
Authoritative owner:
Query projection and revision:
Persistence location (or "none"):
Migration from older data:
Reconnect and sequence-gap repair:
Process-restart behavior:
Save As behavior:
Owning-object and owning-user deletion behavior:
```

For a portable mutation, also name the `ActiveShowService` or capability-specific transaction used
to validate, back up, persist, compile, install, reconcile, and publish the change. For a runtime
field, name the snapshot that repairs a missed event. “The frontend remembers it” and “the server
will refresh everything” are not ownership policies.

## Common splits

- A Cuelist definition is **portable show**; its running Cue, transition, and Chaser position are
  **transient runtime**.
- A fixture profile revision in the installation library is **desk installation**; the immutable
  profile revision referenced by a patched fixture is **portable show**.
- A Macro definition or future Dynamic definition would be **portable show**; supervised instances,
  waits, phases, and failures would be **transient runtime**. Neither feature is implemented.
- An external fixture binding referenced by a show is **portable show**; credentials and adapter
  configuration are normally **desk installation**; sockets, retry state, observed values, and
  health are **transient runtime**.
- Command-line text and key gesture state are **desk interaction** even though the current recovery
  checkpoint is serialized beside the **user Programmer**. Storage location does not change the
  semantic owner.

Unknown portable objects and fields must survive load, migration, Save As, revision creation,
export, and selective import. A known migration may canonicalize data it owns, but it must not
normalize away data owned by a newer or optional capability.
