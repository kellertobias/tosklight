# Media Pane Live Data and Workflows

## Status

**Specification only.** This chunk records the live-data implementation of the Light Desk Media pane for connected media servers. It does not implement the dummy UI chunk, Media Server rebuild, protocol replacement, media fixture generation, ToskLight-specific connection page, renderer behavior, library administration, text-overlay services, persistence migrations, help changes, or executable tests.

This chunk follows [Media Pane Dummy UI](01-media-pane-dummy-ui.md) and the [Media Server Application Behavior](02-media-server-application-behavior.md) chunk. It replaces dummy data with actual patched fixtures, advertised server data, previews, and programmer workflows.

## Goal

Add **Media** as a full built-in and as a configurable Desktop pane when the show contains at least one patched media-server master with logical layer heads and a configured supported media connection such as CITP/MSEX.

The window provides fast visual layer selection and programming from a touch desk:

- select a patched media-server master;
- monitor its composite/program output;
- monitor and select its individual layers;
- browse folders and files with thumbnails;
- program the selected layer; and
- reach less-frequent geometry, playback, Shaper, mask, and effect controls without displacing the primary media browser.

The existing **Media Servers** subview in Show Patch remains the endpoint/setup surface. The new **Media** window is the operating surface.

## Availability and persistence

The built-in and pane picker offer Media only when at least one patched media fixture has:

- a physical master with a configured supported connection;
- at least one logical media-layer head; and
- protocol capabilities sufficient for the requested surface.

Current support is CITP. “CITP or similar” is a capability boundary for future providers, not a claim that another protocol already exists.

A saved Media pane is not deleted when a server temporarily disconnects or a show changes. It displays a clear offline, missing-patch, or unsupported-capability state, retains its desk-local pane configuration, and offers the normal path back to Show Patch setup. Reconnecting restores the configured server and layer when their stable identities still exist.

## Server, output, and layer presentation

The server selector lists patched media-server master fixtures, not arbitrary IP endpoints. Logical layers inherit their master's endpoint.

The main surface shows:

- the selected server's **Program output** or composite preview when advertised;
- each advertised layer preview and status;
- folder and file selection with cached thumbnails; and
- loading, failed-source, stale-preview, and offline feedback without burning error cards into the actual program preview.

The provider must advertise preview-source and layer mappings. The UI must not assume that preview source `0`, layer number, fixture sub-ID, and CITP source ID are interchangeable.

Clicking a layer preview selects that exact logical head through the desk's authoritative shared selection. It does not maintain a competing Media-only selection. External programmer, DMX, OSC, or playback changes reconcile into the visible layer values and ownership feedback.

## Touch folder/file transaction

Touch browsing separates looking through the library from changing live output.

1. The window begins on the layer's live folder and file.
2. Touching another folder changes only local browsing state and fetches that folder's file list and thumbnails.
3. Browsing, backing out, or cancelling does not change programmer values, DMX, or the selected source.
4. Touching a file commits the browsed folder and chosen file together as one authoritative programmer mutation and one Undo step.
5. The engine must never expose an intermediate state containing the new folder with the old file.

Switching server or layer cancels or isolates the old draft explicitly. A disconnect, stale library revision, missing file, or rejected programmer write cannot leave half of the pair applied.

Software and physical encoder operation remains immediate. Turning a Folder, File, Mask Folder, Mask File, or other Media encoder changes its live value as the encoder moves. The staged transaction is specifically the touch-library workflow, not a global delay applied to all controls.

The default attribute-activation configuration links Folder with File and Mask Folder with Mask File as described in [Attribute Registry, Activation Groups, and Indexed Presets](../Next/71-attribute-registry-and-activation-groups.md). The atomic touch commit still requires one grouped server operation; frontend sequencing of two independent requests is not sufficient.

## Layer controls

Folder and file browsing remain the primary central content. Less-frequent controls use a touch-appropriate sidebar, drawer, or comparable secondary region. They may include:

- playback and speed;
- position, scale, rotation, tint, opacity/dimmer, and audio;
- Shapers and other visual manipulation;
- masks; and
- advertised effect slots.

Controls are generated from fixture/profile and connection capabilities. An unsupported feature is absent or clearly unavailable; the UI must not invent four named effects merely because a protocol exposes four anonymous values.

Mask selection reuses the staged folder/file workflow. Provide a visible **Media / Mask** selector whenever masks are supported. A long press on a thumbnail may be an optional shortcut to select it as a mask, but it must not be the only discoverable path. When masks are unsupported, both paths are unavailable with a useful explanation.

## ToskLight Media Server extension

When the selected server advertises the native ToskLight Media Server integration, a separate title-bar action may open the [Media Pane ToskLight Server Page](04-media-pane-tosklight-server-page.md). The generic live-data pane only advertises and routes to that capability. Native generated-source, text-overlay, and connection-management behavior belongs to that later chunk.

## Non-goals

This window does not upload, rename, delete, transcode, or reindex media-library assets. It does not implement the Media renderer, resolve the open mask/effect semantics in the Media Core plan, generate a media fixture profile, or replace Show Patch endpoint configuration.

## Acceptance coverage

1. Media is offered only for a patched media-server master with logical layers and a configured supported connection.
2. Temporary offline or missing capability states preserve an existing pane and explain why it cannot operate.
3. The server selector addresses physical master fixtures and layer previews address their exact logical heads.
4. Program/composite and per-layer previews use advertised mappings and expose honest stale/loading/failure state.
5. Clicking a layer uses the authoritative shared fixture/head selection.
6. Touching folder B while A/file X is live fetches B without changing output.
7. Selecting B/file Y commits exactly B and Y in one mutation, one Undo step, and no observable B/file X intermediate state.
8. Cancelling, changing layer, disconnecting, or losing a library revision cannot partially apply a touch draft.
9. Encoder changes remain immediate, including Folder/File and Mask Folder/Mask File.
10. Media and Mask browsing are explicit and capability-gated; long press is optional rather than mandatory.
11. Secondary controls remain subordinate to the media browser and reflect advertised attributes/functions.
12. ToskLight Media Server-only controls appear behind a separate capability-advertised title-bar action that routes to the dedicated ToskLight Server page.
13. Persisted pane state survives restart, show change, disconnect, and reconnect without inventing another server or layer.
