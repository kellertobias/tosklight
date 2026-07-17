# Completed TODOs

This file contains TODO areas moved out of `TODO.md` after checking the current repo state.
Primary evidence is `docs/todo-completion-audit.md`, backed by the referenced code and tests.

## Moved on 2026-07-12

### Group masters

- Group masters are implemented as a first-class group-to-fader relationship.
- Group master scaling is applied to normalized intensity before encoding.
- Multiple group masters use HTP for affected fixture intensity.
- Fixture sheet and playback/group faders expose group master limiting.

Evidence: `docs/todo-completion-audit.md` lists engine tests for 50/75% arbitration, unassigned groups, sibling-head isolation, and final masters.

### CITP media server patching

- Direct-control fixture capability and parent-owned CITP endpoints are implemented.
- Multi-head media layers inherit the parent endpoint.
- Thumbnail and live preview retrieval are implemented with bounded caches and offline/error state.
- Setup UI supports media endpoint configuration and preview refresh.

Evidence: `docs/todo-completion-audit.md` and `docs/citp-media-servers.md`.

### Empty groups and template shows

- Empty groups are valid and retain portable programming.
- Group-scoped values are stored independently from resolved fixture values.
- Empty group intent becomes active when fixtures are later assigned.
- Unsupported attributes are preserved and surfaced as warnings.

Evidence: `docs/todo-completion-audit.md` and `docs/group-programming.md`.

### Copy group programming to newly added fixtures

- Group programmer, preload, presets, cues, and dynamics resolve live group membership.
- Adding fixtures to a programmed group makes the existing intent apply to new members.
- Membership changes are undoable through show object revision history.
- Replacement and detach flows are implemented for derived/static membership behavior.

Evidence: `docs/todo-completion-audit.md` lists active-cue expansion, show undo, and selection refresh tests.

### Selection macros

- Odd, even, and every-Nth selection macros are implemented over ordered group membership.
- Derived groups retain source relationship, rule, ordering, offset, and N.
- Recursive cycle rejection is implemented.
- Selection expressions distinguish live, derived, frozen, and static membership.

Evidence: `docs/todo-completion-audit.md`; `crates/programmer/src/lib.rs`; `crates/server/src/main.rs`.

### Frozen groups

- Repeated group command syntax creates a frozen/static membership selection.
- Long-press group UI exposes live versus frozen selection.
- Frozen metadata is persisted and refresh is explicit.
- Missing/unpatched frozen members remain identifiable with non-blocking warning state.

Evidence: `docs/todo-completion-audit.md`; group UI and server grammar tests.

### Alignment

- Align left, right, center, and out are implemented over ordered normalized values.
- Alignment handles pan wrapping, inversion, physical metadata, fixture limits, and unsupported/discrete rejection.
- Alignment creates programmer values that can be stored downstream.

Evidence: `docs/todo-completion-audit.md`; server wrap/order tests and fixture encoder tests.

### Preload-mode storage

- Pending and active preload values are separated.
- Store can target arbitrary presets and cues while remaining in preload mode.
- Active-preload fallback is implemented when Store is pressed with an empty pending programmer.
- Preload clear only clears pending preload data and leaves active preload/stored show data intact.

Evidence: `docs/todo-completion-audit.md`; `docs/group-programming.md`; `PreloadStoreModal`; server preload storage tests.

### Older UI follow-up

- Context-sensitive `Special Dialog` control is implemented.
- Full-height 20-key numpad with Store, Set, and Groups is implemented.
- Programmer/playbacks mode toggle uses one dynamic left icon.
- Show/setup control, clock sizing, and removal of Save desk were implemented for the prior dock design.
- Built-ins/desks transition animation is implemented.
- Duplicate nested window titles were removed.
- Stage view selection gestures are implemented.
- Pool windows use responsive sizing.
- `Preload Scene Release` is adjacent to `Preload`.

Evidence: `docs/todo-completion-audit.md`; `StageWindow`; `NumericPad`; `CommandLineBar`; `LeftDock`; React and Playwright tests referenced by the audit.

## Control UI, operator workflow, and group-master pass — 2026-07-12

### Programmer, patch, and control layout

- Programmer numpad cells are equal-sized 20-key touch targets; `GRUP` emits the singular `GROUP` command.
- Show Patch no longer has edit mode. Arming `Set` enables direct fixture name, patch address, Location X/Y/Z, and Rotation X/Y/Z editing.
- Patch keeps Intensity identify visible, including previous/next lamp cycling.
- Patch uses one compact header and fills the available fixture pane space.
- Programmer configuration always exposes six encoder slots with placeholders; the programmer fade and cue fade use full available height.
- Playback pages sit beside the fades and five narrow speed groups occupy the right edge.
- Programmer/playback labels are `Prog.` and `Play B K`; the control split remains stable across modes.

### Pools, cells, desks, and stage

- Groups are available as a regular pool; the built-in Groups dock entry was removed.
- Presets, Stage, Fixture Sheet, and group shortcuts expose Groups controls consistently.
- Preset, group, shortcut, channel, and empty cells use the same 142×94 touch geometry and remain visible/clickable.
- Empty group cells are inert unless Store is armed; Store can create an empty group with no selection.
- Empty desk areas open the window picker; pane titles drag by whole grid tiles; pane settings use touch-friendly custom selects and no fullscreen action.
- Long-press desk settings support rename, icon selection, and confirmed deletion with the shared button styling.
- Stage 2D reflects live fixture defaults and resolved values, including color, pan/tilt glyphs, and intensity; 3D has audience-facing defaults and blue selected outlines.
- Follow Preload is available in Stage views; Fixture Sheet preload mode shows current and pending values.

### Store, live input, and show state

- Store is available on software control surfaces when hardware does not provide it; short press arms, long press opens settings, and the armed state is visible.
- Escape cancels Store; Clear cancels Store and clears the programmer.
- Store supports groups, family-filtered presets, playback cue creation/appending, active-cue merging, and merge/overwrite prompts.
- Preset storage uses the backend snake-case store-mode contract and stores only the selected family, with All preserving all active values.
- Direct live input is absolute and revision-free, with one-second per-input locks shared by all sessions of the same user.
- Show dirty state is driven by persisted show-object changes only, shown by a yellow dot; playback actions and live fader/master changes do not mark it dirty.

### Operator identity, diagnostics, and group masters

- The default `Operator` user is created and selected for new devices; Change User supports login as an existing user or creating a new one, with programmers attached to users across sessions.
- The dock identity/time/show control opens the show menu; Debug displays server events and can simulate hardware and errors.
- Group masters scale normalized intensity using the HTP maximum across assigned groups.
- Group Flash is transient: output uses `max(flash, fader)` while held and the fader value is never changed.

Evidence and release verification: [docs/todo-completion-audit.md](docs/todo-completion-audit.md), 80 Rust tests, 28 frontend tests, 7 Playwright scenarios, strict Clippy, formatting/type checks, production build, desktop check, and the 64-universe benchmark (`PASS`).

## Virtual Playback exclusion zones — 2026-07-16

- Shift-selection and named zone creation are inert configuration gestures in the real Virtual Playbacks pane.
- Zone names, ordered lists, memberships, and retained hidden grid cells are editable in Pane Settings and persist by show, control desk, and surface.
- One serialized server action path enforces overlapping-zone union semantics across UI, F-key shortcuts, REST, OSC, and restart normalization.
- Multiple sessions on one desk share the desk's zone behavior; another desk for the same user remains independent while programmer values remain user-shared.
- Automatic full-override release remains independent of mutual exclusion.

Evidence: [VPB-007](docs/testing/06-preload-modes-and-virtual-playbacks.md), `tests/06-preload-modes-and-virtual-playbacks.spec.ts`, focused UI unit coverage, and the completed [Feature 17 contract](docs/planned%20features/17-virtual-playback-exclusion-zones.DONE.md).

## Manual-review software corrections — 2026-07-17

- Saved workspace arrangements use Desktop terminology while physical desk identity, aliases, sessions, and OSC routing remain unchanged.
- Cues, File Manager, Text Editor, Help, DMX, Stage, fixture browsers, Desk Setup, diagnostics, output routes, standard file fields, MIB editing, and safe show recovery implement the reviewed operator contracts.
- Persisted legacy pane layouts receive deterministic defaults; deterministic help screenshots and manual keycap spacing were refreshed.

Evidence: [MANUAL-019](docs/testing/10-desk-lock-and-operator-ui.md), [File Manager and Text Editor](docs/testing/09-file-manager-and-text-editor.md), `tests/19-manual-review-software-corrections.spec.ts`, focused UI unit coverage, refreshed help screenshots, the rebuilt PDF manual, and the completed [Feature 19 contract](docs/planned%20features/19-manual-review-software-corrections.DONE.md).
