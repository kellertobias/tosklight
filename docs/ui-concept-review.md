# Lighting UI concept review

## Design direction

The supplied mockups establish a useful console-like model: a large workspace, a persistent programming/playback dock, and a small navigation rail. The revised design keeps that model while making it responsive and aligning it with the server's session, programmer, playback, fixture-head, and diagnostics domains.

## Backend-to-UI mapping

| UI concept | Server domain | Notes |
| --- | --- | --- |
| Selected fixtures and groups | Session programmer | Never shared implicitly between sessions. |
| Command line and undo/redo | Session programmer | Persisted across disconnect and daemon restart. |
| Blind, preview, highlight | Session programmer modes | Always visible near the command line. |
| Attribute encoders | Typed fixture attributes | Generated from selected fixtures and capability intersection. |
| Raw channels | DMX diagnostic inspector | Explicit override mode with warning treatment. |
| Preset pools | Revisioned show objects | Display partial fixture coverage and store mode. |
| Sequence/executor tiles | Playback engine | Show current/next cue, fade progress, priority, and state. |
| Stage view fixture heads | Physical fixture plus logical heads | Parent/shared attributes are visually distinct. |
| Output/network indicators | Diagnostics and output health | Persistent system bar with drill-down. |
| WebMIDI/native controller | Typed client command adapter | Same command envelope as touch and keyboard input. |

## Application shell

1. A persistent system bar contains show, session, live/blind state, timecode source and lock, output health, connection state, grand master, and blackout.
2. A workspace rail switches among Stage, Fixtures, Presets, Sequences, Patch, and Setup. It collapses to icons on narrow screens and becomes a bottom tab bar in portrait.
3. The center workspace is the selected view. Views may open a contextual inspector without replacing the programmer dock.
4. A dock contains the session command line and either attribute encoders or playback executors. It supports expanded, compact, and collapsed modes.

## Responsive profiles

### Full touch

For roughly 7–13 inch landscape displays without detected hardware, use large on-screen encoders, executor keys, numeric entry, and command controls. The dock occupies approximately 34–42% of height and can be resized.

### Hardware assisted

When a mapped controller is active, collapse physical encoder and executor controls while retaining labels, current values, page mapping, feedback, and command line. The workspace receives the reclaimed height. This is an explicit user-selectable mode; connecting hardware must not unexpectedly rearrange a live desk.

### Compact portrait

Use a bottom workspace tab bar, one main view, and a bottom sheet for programmer or playback controls. Show four encoders at a time with horizontal paging. Keep blackout, connection, and live/blind state visible.

### Multi-display

Each window is a named surface with its own layout, stored per user in the portable show. A surface can be workspace-only, programmer-only, playback-only, or monitoring-only. Session ownership remains explicit.

## Interaction and safety rules

- Minimum primary touch target: 48 CSS pixels; dense table rows may be 40 pixels when selection is not safety critical.
- All controls support touch, pointer, keyboard, and controller activation without hover-only behavior.
- Cyan indicates selection, amber indicates programmer or pending changes, green indicates healthy/running output, blue indicates informational state, and red is reserved for blackout, faults, destructive actions, or hazardous-device warnings.
- Raw DMX, hazardous fixtures, remote programmer clearing, show activation, and output changes require visually explicit modes or confirmation.
- Changes rejected by object revision conflicts open a comparison/merge surface and never silently overwrite another session.
- Loss of WebSocket events triggers a scoped REST refresh while preserving the local command line and making reconnect state visible.

## Suggested implementation shape

Use one TypeScript/React application for browser and Tauri. Keep transport, WebMIDI, native MIDI/HID, persistence, and window-management behind adapters. React components consume a shared typed client store built from REST bootstrap snapshots plus ordered WebSocket deltas. Layout profiles are data, not hard-coded device branches, and are persisted as show-specific per-user objects.
