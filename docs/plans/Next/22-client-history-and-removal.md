# Client History and Removal

## Status and scope

This is a future Desk Setup feature. Do not implement it as part of the current settings-screen UI pass.

Desk Setup currently lets an operator choose a known client configuration as the app's default screen. Over time, clients that are no longer used remain in that chooser. The desk needs enough client-presence information to distinguish a client that is connected now from a historical client, show when each historical client was last connected, and let an operator remove obsolete clients deliberately.

This data is desk-level installation state. It must not be stored in or exported with a portable show.

## Operator experience

The **Choose default screen** modal shall list known clients with:

- the client name and stable client identity;
- whether the client is currently connected;
- the date and time it was last connected;
- the screen configuration currently associated with it; and
- a **Remove client** action for clients that can be removed safely.

Connected clients should be grouped above disconnected clients. Within each group, sort by most recent connection first. The currently used client and current default screen must be clearly identified.

Removing a client requires a confirmation dialog that names the client and explains which desk-level client settings will be removed. The current client cannot remove itself. A client with an active session cannot be removed until it disconnects, unless a later design adds an explicit administrative disconnect workflow.

## Persistence and reconnect behavior

Persist a last-connected timestamp against the stable client identity. Update it when a client establishes a valid session, without creating a new client record for every reconnect. The timestamp and client inventory must survive a server restart.

Removal must clean up only state owned by that client. It must not delete portable shows, users, other clients, or installation-wide configuration. The implementation plan must inventory desk-scoped references before deciding whether screen layouts, per-client default-screen selection, and other client-owned settings are deleted or retained.

If a removed client reconnects later with the same local identifier, treat it as a newly registered client and assign the normal new-client defaults. Do not silently resurrect the removed client configuration.

## API and compatibility work

The current bootstrap desk list does not expose live connection state or last-connected timestamps. Add an authoritative server-side client summary rather than deriving presence from browser-local state. The API must distinguish:

- known client identity;
- current connection state;
- last-connected time;
- associated control desk/default-screen configuration; and
- whether removal is currently allowed.

Define migration behavior for existing desk records that have no timestamp. They should remain visible with an explicit unknown last-connected value until they connect again or are removed.

## Acceptance criteria

1. Two connected clients appear as connected and cannot be mistaken for historical entries.
2. Disconnecting a client records its last-connected time and the value survives a server restart.
3. Reconnecting the same client updates the existing record instead of adding a duplicate.
4. Existing records without timestamps remain usable and display **Last connected unknown**.
5. The current client and any actively connected client cannot be removed through the normal removal action.
6. Removing a disconnected client requires confirmation and removes it from the chooser without changing shows, users, or other clients.
7. A removed client that reconnects is registered with new-client defaults and does not recover its deleted client-owned configuration.
8. Client history and removal remain installation-level data and never appear in an exported show file.
