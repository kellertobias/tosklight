# API rules

Maintainer decisions, 2026-07-23. These rules are binding for every new or reworked HTTP/WS
route and for every client call site. When touching an existing route that violates them,
bring it into compliance in the same chunk or record precisely why not.

Customer-facing protocol documentation covers **OSC only** (`docs/help/50-Protocols/`). The
HTTP and WebSocket APIs are internal application transports plus an integrator convenience;
they are not published documentation surfaces for now.

## 1 — Classify every write

Ask: **does it change playback or fixture live state, the selection, or what the command
line shows?**

- **Yes → it is a live-control action** (rule 2). Examples: selecting a group, triggering a
  preset, triggering a virtual playback, any button on a touch playback, entering encoder
  values on fixtures, pressing the virtual hardware / touch programmer keys, programmer
  keyboard shortcuts, GO/flash/fader on a playback.
- **No → it is an object-intent update** (rule 3). Examples: window settings, desk
  settings, a fixture's DMX address, name, or stage position — everything that edits stored
  configuration rather than live state.

Reads load whole-object state (snapshots); volatile state is pushed (events/telemetry), not
polled.

## 2 — Live-control actions

- The **desk UI sends live-control actions as frames on its established WebSocket** — the
  connection is already up, frames arrive in order, and there is no per-request handshake.
- **A WebSocket action frame is never re-sent.** At the protocol level a frame is either
  delivered in order or the connection is dead; the only duplication risk is client code
  replaying queued frames after a reconnect — that is forbidden. After a reconnect the
  client re-reads authoritative state instead of re-sending.
- **Every WebSocket action also exists as a plain HTTP action URL** so a microcontroller
  with minimal storage can trigger it:
  - **GET** when the action needs no payload (e.g. `GET …/cuelists/1/go`). These GETs have
    side effects by design; the server must answer `Cache-Control: no-store`, and such URLs
    are only reachable on the trusted lighting network with the desk token.
  - **POST** when the action carries content (e.g. posting a fader position).
- The HTTP action form is fire-and-forget for integrators: no request-identity machinery.
  A caller that sends GO twice meant GO twice.

## 3 — Object-intent updates

- Intent style, not whole-object overwrite: **`POST …/{object}/{id}/update`** with a typed
  body carrying **only the fields being changed**. Pressing GO must never rewrite a
  playback object; renaming a fixture must not resubmit its patch.
- **Editing requests must never be accidentally redone.** Timeouts, dropped responses, and
  client retries are absorbed by request identity: an edit carries a client-generated
  request id, the server keeps a replay window, and a resend returns the stored outcome of
  the first execution instead of executing again. This machinery belongs to *edits*; live
  control gets its safety from the WebSocket rule above.

## 4 — Show logic lives on the server

**Anything that changes fixture state, playback state, or show data is computed by the
server.** The UI owns only UI logic: interaction behavior (a Shift-modified press meaning
something else, gestures, focus), and everything display-related (layout, formatting,
which pane shows what). The UI must not have opinions about how the server resolves show
state.

In particular, spread and fan-out semantics are server-side: editing a value across a
selection (patch addresses, a `-100 THRU 100` spread, group-wide changes, multi-fixture
stage-position moves) is **one request** carrying the selection/group members and the
spread parameters; the server computes the per-fixture results. The UI never precomputes
per-fixture values.

## 5 — Typing

- Every body validates against the typed wire contract (`crates/wire`, generated client
  types). A mismatch returns a clear 4xx error naming the field — the server never crashes
  on bad input.
- **Additional/unknown properties are accepted, and logged server-side** — never rejected.
  (Existing `deny_unknown_fields` wire types are brought into compliance as they are
  touched.)

## 6 — Scoping

- **No show-scoped routes.** The desk operates on the one loaded show. Cross-show routes
  exist only for the library: listing shows and loading objects from another show — there
  the show id is the operand, not a scope.
- The show guard is an **optional request header**: when the desk sends it, the server
  verifies the loaded show and rejects a mismatch (the show-switch race); when absent —
  the automation case — no check happens.
- A desk id appears in a route only when the data is genuinely per-desk. Virtual-playback
  exclusion zones are **show-level** (the storage already keys by show); the desk segment
  in the current route is an authentication artifact and is dropped whenever that route is
  next touched.

## 7 — Undo and concurrency (maintainer, 2026-07-23)

- **Undo is a programmer action, scoped to a desk.** It undoes that desk's programmer
  changes. The only way an undo affects show-global data is when the undone step was a
  recording (e.g. a cue or preset stored from that desk's programmer) — then the undo
  removes what that recording created. There is no generic show-wide undo surface.
- **Concurrent users are last-write-wins by design.** Two users writing the same object
  (e.g. the same preset) is either intentional or operationally harmless; the desk does
  not arbitrate between users. Object/show revision guards (`If-Match`, revision checks)
  exist to protect a client from acting on **stale state it didn't know changed** — they
  are not inter-user locks, and a revision conflict must never permanently block a
  deliberate operator action; the surface re-reads and reapplies.
- **Concurrent additions must not collide.** Operations that create new entries (two
  users storing a cue on the same cuelist) are server-assigned: the server picks the next
  cue number/slot at execution time, so both stores succeed as two new cues. Intent-shaped
  writes (rule 3) make this natural — the client never sends "the whole new cuelist".

## 8 — Persistence cadence (maintainer, 2026-07-23)

- The active show is authoritative **in memory** on the server; the `.show` file is
  flushed on a configurable autosave interval (default 30 s, desk configuration), not on
  every mutation. Losing the last interval on power loss is accepted (WAL +
  `synchronous=NORMAL` never guaranteed hard durability per-commit anyway).
- Flush immediately at hard boundaries regardless of interval: show switch/close, named
  revision save, upload/overwrite, shutdown, and after an idle gap. Automatic backups are
  taken per flush, not per mutation.
- Events, revisions, replay windows, and undo operate at **mutation time** in memory —
  persistence cadence must not change any client-observable ordering.
