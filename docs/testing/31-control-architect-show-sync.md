# Control and Architect Show Sync

## Purpose

Prove that a show opened in the Architect from a desk stays in step with that desk without an
explicit Save: edits in either application appear in the other and survive a restart, independent
edits both survive, a same-field edit becomes a visible and recoverable conflict, an Architect that
was offline catches up exactly once, live control never travels, and nothing is ever written into
the wrong show or the wrong desk.

The protocol is described in `docs/engineering/show-sync.md`. Server steps can be driven through
the API (`POST /api/v2/show-sync/transactions`, and `/api/v2/events` subscribed with the
`show_sync` topic); Architect
steps use the Architect window or its local editing API.

## Binding a document to a desk show

1. Start the desk and open a show, `Tour`. Confirm `GET /api/v2/readiness` reports a
   `desk_identity` and that it is unchanged after restarting the desk.
2. In the Architect, open `Tour` through **Open → Load from ToskLight Control**. Confirm the
   document opens and the Architect's data directory holds `show-sync/index.json` and one
   `show-sync/<association>/binding.json` naming the desk identity and the show's UUID.
3. Confirm no `Tour.show.desk-source.json` file is written beside the document.
4. **Save As** the document to `Tour copy.show` and open the copy. Confirm the copy is not bound:
   nothing it does reaches the desk.

## Architect edits reach the desk

1. In the Architect, add an annotation on the CAD plan and move a lamp 0.5 m along X in one drag.
   Confirm the desk shows the moved lamp without any Save, and that `GET /api/v2/objects/cad_annotation`
   lists the annotation.
2. Confirm the desk's show revision advanced by exactly one for the drag, and that playback and
   DMX output did not pause and the desk did not reload its show.
3. Restart both applications. Confirm the annotation and the moved lamp are still there in both.

## Desk edits reach the Architect

1. On the desk, rename a patch layer and change a fixture's address. Confirm the Architect shows
   both without reloading the document.
2. On the desk, record a cue and run a playback. Confirm the Architect's document does not change
   and the sync feed carries no event for the playback or for programmer values, selection or
   Highlight.
3. Subscribe to `/api/v2/events` without the `show_sync` topic. Confirm the stream shows one event
   per desk commit and no sync events, exactly as before.
4. In the desk's file browser, replace the open show's file in the shows folder. Confirm the
   Architect is told at once to re-read the show.

## Independent and conflicting edits

1. Disconnect the Architect from the network. In the Architect, change layer `Truss`'s order to 4
   and rename it `Back Truss`. On the desk, rename the same layer `Front Truss`.
2. Reconnect. Confirm the layer's order is 4 on both sides, its name is `Front Truss` on both
   sides, and the Architect shows one conflict for the name offering **Keep Control's** and
   **Use mine**, with `Back Truss` kept as a recoverable draft.
3. Choose **Use mine**. Confirm both sides read `Back Truss` and the conflict is gone.
4. Bind a second Architect to the same show and edit a different field of the same annotation in
   each. Confirm both edits survive without either Architect waiting for the other.

## Offline, retries and restarts

1. With the Architect offline, make three edits and quit the Architect. Restart it while the desk
   is still unreachable. Confirm the edits are still in the document and the status reads Offline,
   never "Saved to Control".
2. Reconnect. Confirm each edit is applied on the desk once, in order.
3. Make an edit and drop the desk's reply (for example by stopping the network after the request
   leaves). Confirm the retry reports the edit as already applied and the desk's show revision
   advanced only once.
4. Restart the desk between an edit and its retry. Confirm the retry still applies once.
5. Save the document to the desk manually, then retry an edit the desk had already applied.
   Confirm the retry still reports it as already applied.
6. Hold a write transaction open on the desk's show file. Confirm an Architect edit is refused as
   retryable, nothing is written, and the retry after the file is released applies once.

## The wrong show or the wrong desk

1. On the desk, open a different show. Confirm the Architect reports **Show not active on
   Control**, keeps its edits, and writes nothing into the open show.
2. Reopen `Tour` on the desk. Confirm the held edits apply.
3. Point the binding at a different desk answering on the same address. Confirm every transaction
   is refused as a different desk and nothing is written.
4. Send a transaction writing a cue list, a playback or the show name. Confirm it is refused and
   nothing is written.

## Large changes and named revisions

1. Import an MVR with several hundred fixtures on the desk. Confirm the feed announces one gap
   instead of hundreds of bodies, and that the Architect re-reads the patch and catches up.
2. Save a named revision on the desk and open it. Confirm the revision copy has its own show UUID
   and that the bound Architect reports **Show not active on Control** instead of writing into it.

## Standalone mode

1. Create a new document in the Architect without a desk. Confirm it has no binding, edits behave
   exactly as before, and nothing is sent anywhere.
2. Damage `show-sync/index.json`. Confirm the bound document still opens, standalone, with a
   message saying why, and that the damaged file is left in place.
