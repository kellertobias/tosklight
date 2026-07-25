# Media Pane Dummy UI

## Status

**Specification only.** This chunk records the first Media pane implementation step: build the Light Desk Media pane UI against deterministic dummy data. It does not implement Media Server protocols, media fixtures, programmer mutations, real previews, CITP, native Media Core requests, persistence migrations, help changes, or executable tests.

## Goal

Build **Media** as a full built-in and configurable Desktop pane using dummy data so the operator workflow, layout, touch targets, and visual hierarchy can be reviewed before the Media Server and live-data integration exist.

The dummy pane must look and behave like the intended operator surface, but every data source is local deterministic mock data. It must not pretend to control output.

## UI Scope

The dummy pane should show:

- a server selector populated by mock patched media-server masters;
- a program/composite preview placeholder;
- layer preview tiles with status and selection affordances;
- folder and file browsing with mock thumbnails;
- a visible local browsing draft separate from the live folder/file;
- Media and Mask browsing selectors where the mock capability says masks exist;
- secondary controls for playback, speed, position, scale, rotation, tint, opacity/dimmer, audio, Shapers, masks, and effects;
- offline, stale-preview, failed-source, missing-patch, and unsupported-capability states; and
- pane settings for the selected mock server/layer where useful.

The pane should be usable as a full built-in and as a configurable Desktop pane. It should use the shared ToskLight UI primitives where available, but remain application-owned where it needs Light Desk state adapters later.

## Dummy-Data Rules

Dummy data must be deterministic and visibly labeled in development/test surfaces so it cannot be mistaken for real server state. It may include representative servers, layers, folders, files, thumbnails, masks, and capability flags.

The dummy UI must not:

- open network connections;
- mutate Programmer values;
- patch fixtures;
- write Media pane persistence beyond ordinary pane layout/settings required for the mock surface;
- claim real preview freshness; or
- call generic CITP or native ToskLight Media Core endpoints.

## Acceptance Coverage

1. Media is available as a full built-in and configurable pane in the dummy-data mode.
2. The pane renders server, program output, layer previews, library folders/files, masks, and secondary controls from deterministic data.
3. Browsing another folder changes only local draft state and does not claim live output changed.
4. Offline, stale, failed, missing-patch, and unsupported states are visible without replacing the actual preview area with misleading output.
5. Layer and file interactions expose the intended touch workflow but do not mutate real selection, Programmer state, or output.
6. The dummy pane can be visually reviewed without a running Media Server.
