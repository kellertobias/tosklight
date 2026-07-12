# TODO

> Status: implemented and audited. Every behavior and decision below has production code and
> automated evidence mapped in `docs/todo-completion-audit.md`, including CITP protocol fixtures,
> stage touch gestures, session isolation, rendered DMX output, and responsive browser paths.

## Group masters

Add group masters as a first-class relationship between a group and a fader/playback control.

### Behavior

- A group assigned to a fader has a master level from 0–100%.
- The group master scales the intensity/dimmer values produced for every fixture currently in that group:

  `effective dimmer = fixture/group value × group master`

- The master must scale the normalized intensity before it is converted to the fixture's native dimmer range.
- A fixture can belong to multiple groups with masters.
- When multiple group masters affect the same fixture, intensity uses HTP: the highest resulting intensity wins.
- A fixture that is active in any contributing group remains on, even if another contributing group is at zero.
- Group-master scaling should apply consistently to programmer output, presets, dynamics, sequences/cues, and playback output.
- Changing group membership or a group-master level must immediately recompute affected fixture output.

### Decisions

- Group masters affect intensity only. Any future attribute master uses a distinct typed relationship.
- Programmer/playback priorities and HTP/LTP resolve first; group scaling follows while normalized;
  grand master and blackout follow group scaling; fixture encoding is last.
- The fixture sheet uses an amber `Limited by group master` warning and group/playback faders expose
  the exact master percentage.
- A group master is a scaling stage, never a normal fixture-level HTP contribution.

## CITP media server patching

> Implemented: direct-control capability and parent-owned endpoints, inherited multi-head layers,
> MSEX thumbnail/preview retrieval, bounded caches, authenticated REST endpoints, offline state,
> Setup UI, protocol-fixture tests, and lifecycle behavior are documented in
> `docs/citp-media-servers.md`.

Support patching a CITP media server as the master fixture of a multi-head fixture setup.

### Behavior

- When a fixture is patched as a media server or another fixture type that supports direct IP control, allow an IP address to be set in the patch.
- Use the patched IP address as the target for CITP communication.
- Treat the media server master as the parent fixture for its layers.
- Represent the layers as multi-heads of the same master fixture rather than as independent fixtures.
- Use CITP to retrieve the configured thumbnails from the media server.
- Use CITP to retrieve a live output preview so the operator can see what the media server is currently outputting.
- Preserve the master/layer relationship in the patch model so edits to the master remain authoritative for all heads.

### Decisions

- A fixture profile must explicitly list the supported direct-control protocol; CITP is currently the
  only supported type.
- The endpoint is stored per patched physical fixture in the portable show.
- Thumbnail refresh is explicit; an enabled live preview polls once per second. Separate bounded
  fixture-scoped LRU caches retain 512 thumbnails and 32 previews.
- Offline or malformed responses retain a visible error and stable empty/last preview without
  affecting lighting output.
- Multi-head layers inherit the physical master endpoint and cannot override it.

## Empty groups and template shows

Allow groups to exist without assigned fixtures and remain fully programmable.

### Behavior

- An empty group is valid and can be selected, named, assigned presets, used in dynamics, and used in sequences/cues.
- Empty groups should display a clear non-blocking warning such as `Group is empty`.
- All group attributes remain active while the group is empty, so users can build a template show before fixtures are patched or assigned.
- Presets stored against an empty group retain their attribute values and group association without requiring fixture instances.
- Sequences/cues and dynamics programmed for an empty group retain their intent and become effective when fixtures are later assigned.
- Adding fixtures to the group should make the existing group programming apply to those fixtures without requiring the show to be rebuilt.

### Group-relative programming

Group programming must be represented independently from a snapshot of the group's current fixture IDs. This is required for template shows and for reliable membership changes.

- Store group-scoped attribute values/preset references in addition to any resolved fixture values.
- Define how fixture capabilities are handled when a newly assigned fixture does not support an attribute used by the group.
- Show unsupported attributes as warnings while preserving the portable group programming.
- Define behavior when a group changes from empty to populated while a cue, dynamic, or playback is active.

## Copy group programming to newly added fixtures

When a fixture is added to a group, bring it into the group's existing programming.

### Behavior

- If the fixture is already a member, adding it again is a no-op and must not duplicate values or references.
- For a newly added fixture, copy/apply all applicable group presets to that fixture, including color, position, intensity, beam, focus, and other supported attributes.
- Preserve the distinction between fixture-independent group intent and resolved fixture values.
- If the group is selected for programming dynamics or cues, update all downstream references so the new fixture participates as a member of that group.
- Existing cues, sequences, and dynamics that reference the group must resolve against the current group membership rather than a permanently frozen fixture list, unless explicitly stored as a fixture snapshot.
- Newly added fixtures should receive the same group-relative preset values, not values accidentally derived from another fixture's physical calibration or absolute range.

### Safety and lifecycle

- Make membership changes undoable.
- Show a preview or confirmation when adding a fixture would apply a large amount of existing programming.
- Removing a fixture stops live group resolution; it does not delete fixture-scoped values.
- Manually overridden fixture values survive later group membership changes.

## Selection macros

Add selection operations that derive a selection from the ordered members of a group.

### Required operations

- Select odd members.
- Select even members.
- Select every Nth member.
- Support a clear definition of the starting offset/index for every-Nth selection.
- Store a group from the current selection.

### Dynamic derived groups

A group created from a selection macro should retain its source relationship:

- Example: `Group A → odd → Store Group 2` creates a derived group containing the odd members of Group A.
- If Group A changes, Group 2 must be recalculated automatically.
- Preserve the source group, selection rule, ordering, offset, and N value rather than storing only the currently resolved fixture IDs.
- Define behavior for chained derived groups and cycles; cycles must be rejected or safely detected.
- Manual membership editing is disallowed until the operator explicitly detaches the derived group.
- Membership order must be deterministic and visible to the user because odd/even and alignment depend on it.

### Selection model requirements

- Selection cannot be represented only as an unordered set; retain an ordered fixture sequence for group-relative operations.
- Distinguish a live/derived selection from a static selection.
- Keep selection macros session-scoped unless explicitly stored as a show object.
- Make the current selection rule visible and reversible.

## Frozen groups

Support an explicit way to use a group's current members as a fixed snapshot instead of maintaining a live reference to the group.

### Terminology

Use **frozen group** in the product and documentation. The existing group remains the live source; the frozen group stores the fixture membership that existed at the moment it was created.

### Command-line syntax

- Typing the group reference twice in the command line means “resolve this group now and use its actual members.”
- The grammar is:

  - `Group A` → reference the live group; membership follows future group changes.
  - `Group A Group A` → reference a frozen snapshot of Group A's current members.

- The parser must distinguish this from accidental duplicate group references and provide a clear command-line explanation/error when the syntax is invalid.
- A frozen snapshot may remain a temporary session selection or be stored as a persistent named group.
- Define how nested, derived, or already frozen groups behave when referenced twice.

### UI interaction

- Long-pressing a group in the group UI opens a context menu.
- The context menu includes an action such as `Select frozen group`.
- The action selects the group's current members as a static fixture selection and visibly indicates that the selection is frozen.
- A normal tap continues to select/reference the live group.
- The context menu should also make the difference between `Select group` and `Select frozen group` explicit.

### Lifecycle and display

- A frozen group must retain the exact fixture membership and ordering captured at creation time.
- Later additions, removals, or reorderings in the source group must not change an existing frozen group.
- Show the source group and capture revision/time where useful, for example `Front Truss · frozen at revision 12`.
- Provide a way to refresh/re-freeze the snapshot deliberately; do not update it silently.
- Frozen groups support both temporary selections and named stored groups.
- Missing, deleted, or unpatched fixtures in a frozen group should remain identifiable and produce a non-blocking warning.
- Frozen groups must be compatible with selection macros and alignment, but their membership must remain static while those operations run.
- Frozen references freeze membership only. Masters remain relationships of live named groups and are
  not copied into the frozen selection.

## Alignment

Add alignment operations for an individual attribute across the ordered current selection.

### Required modes

- Align left.
- Align right.
- Align center.
- Align out.

### Behavior

- Alignment distributes the selected attribute's values across the ordered selection.
- It must work for attributes such as pan and tilt where interpolation is meaningful.
- Define the endpoint values, center value, and whether `align out` mirrors values away from the center.
- Define how alignment handles selections with one, two, or an odd number of fixtures.
- Alignment uses the ordered current selection: group order for live/frozen groups and gesture/order
  insertion order for static selections.
- Preserve attribute limits, invert flags, physical ranges, and wrap-around behavior for pan.
- Handle fixtures with different attribute ranges through normalized values before converting to fixture-native values.
- Unsupported or discrete attributes should be rejected or handled with an explicit non-interpolating rule.
- Alignment should create programmer values that can be stored in presets, cues, sequences, or dynamics.

## Suggested implementation boundaries

These features should be modeled separately even though they interact:

1. **Group model:** ordered membership, empty-group validity, derived-group metadata, and group-scoped programming.
2. **Selection model:** ordered selections plus selection expressions such as odd/even/every-Nth.
3. **Resolution model:** group-relative programming expanded to current fixtures, then group-master scaling, then final HTP/LTP arbitration.
4. **Persistence model:** stable references for groups, presets, cues, dynamics, derived selections, and their revisions.
5. **UI model:** empty-group warnings, derived-group indicators, group-master controls, selection-macro controls, and alignment controls.

## Acceptance scenarios

- A fader at 50% limits a group's fixture at 80% to 40% output.
- A fixture in two mastered groups receives the higher resulting intensity.
- A completely empty group can store color and position presets and be used in a sequence.
- Adding fixtures to that empty group makes the stored programming effective automatically.
- Adding two fixtures to a populated group applies the group's existing presets and downstream group-based cues/dynamics to both.
- An odd-derived group updates when its source group gains or loses fixtures.
- A live group reference follows membership changes, while a frozen group reference does not.
- Repeating a group reference in the command line produces the documented frozen-membership behavior.
- Long-pressing a group exposes `Select frozen group`, and the resulting selection remains unchanged when the source group changes.
- Every-Nth selection is repeatable and uses documented ordering/offset semantics.

## Preload-mode storage

Allow the current preloaded scene to be stored directly into an arbitrary cue or preset while remaining in preload mode.

### Behavior

- In preload mode, `Store` can target any cue, not only the currently selected or next cue.
- In preload mode, `Store` can also save the current preloaded scene as a preset.
- Stored cues and presets must capture the preloaded scene values, including the selected/group-relative scope and applicable timings or attribute data.
- Storage should use the normal revision/conflict and overwrite/merge safeguards for the target cue or preset.
- The UI should make the target explicit before committing: target cue number/name or preset pool/slot/name.

### Preload clear semantics

- `Clear` in preload mode clears the pending/preloaded scene state.
- Preload clear must not reset, release, or otherwise modify an already active preload scene/playback.
- If an active preload scene and a newly edited pending preload scene are both visible, the UI must distinguish them clearly so `Clear` cannot be mistaken for stopping the active scene.
- Clearing a pending preload must not alter stored cues, stored presets, live output, or the active playback state.
- Repeated Clear is idempotent. With no pending content it leaves active/stored state untouched and
  the command line remains visibly empty.

### Acceptance scenarios

- Build a scene in preload mode and store it into an arbitrary existing cue without executing that cue.
- Build a scene in preload mode and store it as a new or existing preset without leaving preload mode.
- With an active preload scene running, edit a different pending preload scene, then use `Clear`; only the pending scene is cleared and the active scene continues unchanged.
- After clearing pending preload data, stored cues and presets remain intact and live output is unaffected.

## UI follow-up

These changes describe the intended UI state before the related features are implemented.

### Context-sensitive special dialogs

- Replace the always-present `Special Dialogs` control with a single `Special Dialog` button.
- Show the button only when the currently selected control screen has a special dialog.
- Required screens with special dialogs:
  - Color
  - Position
  - Beam
  - Control
  - Dynamics
- The button should open the special dialog for the currently active screen/family.
- Do not show an inactive or misleading button on screens without a special dialog.

### Control-section layout and numpad

- Make the right side of the control section always occupy the full available height.
- Ensure the numpad stretches to fill that right-side height, including on smaller and taller layouts.
- Add `Store` to the numpad.
- Add `Set` to the numpad.
- Add `Groups` to the numpad.
- Preserve clear touch targets and avoid causing the numpad to overflow or become vertically compressed.

### Programmer/playbacks mode icon

- Keep the current programmer/playbacks behavior and text unchanged.
- Remove the right-side icon from the mode toggle.
- Keep one icon on the left.
- Change that left icon based on the currently active mode: programmer or playbacks.

### Show/setup area

- Replace the separate show/setup presentation with one button at the bottom of the dock.
- Show `Show` when no show is loaded.
- Show the loaded show name when a show is active.
- Remove the operator name from this area.
- The button remains the entry point for show/setup actions.

### Clock and dock space

- Display the clock primarily as hours and minutes.
- Show seconds compactly at/in the colon between hours and minutes to save horizontal space.
- Scale the clock to the available width.
- Match the clock sizing behavior to the show/setup button itself, rather than sizing based on the button's text.
- Remove the `Save desk` button entirely; it should not occupy dock space or compete visually with the show/setup control.

### Built-ins/desks transition

- Add a small animation when switching between Built-ins and Desks.
- The complete transition must not take longer than 500 ms.
- Keep the animation subtle enough that it does not slow down frequent workspace switching.

### Window titles

- Remove duplicated nested titles.
- For example, avoid showing both `Color & Position Presets` and an inner `Presets` title when they represent the same window.
- Each window should have one authoritative title, with nested content using tabs, filters, or controls rather than repeating the title.
- Verify this across desks and built-in windows, not only the presets window.

### Stage-view fixture selection

Support selecting fixtures directly in the stage view:

- Click a lamp/fixture to select it.
- Shift-click to select a group/range of lamps according to the defined stage ordering.
- Control-click to add or remove individual fixtures without replacing the current selection.
- Drag to select fixtures in a marquee.
- Control-drag to add fixtures from a marquee to the current selection.
- Preserve the existing selection when using additive selection gestures.
- Make selected fixtures visibly distinct and keep stage selection synchronized with the fixture sheet, groups, and programmer selection.
- Shift-click selects the contiguous range from the last selection anchor in visible stage order.

### Pool-window sizing

- All pool windows must use the actual width available in their containing pane.
- Avoid fixed-width cards or layouts that leave unusable blank space when the pane is wider.
- Reflow pool contents responsively while preserving readable cards and usable touch targets.

### Preload controls

- Place `Preload Scene Release` directly next to the `Preload` button.
- Keep the two controls visually associated while retaining a clear distinction between activating/preloading a scene and releasing it.

### UI acceptance scenarios

- A color screen shows `Special Dialog`; a fixture or unrelated screen does not.
- The numpad occupies the full right-side control height and exposes Store, Set, and Groups.
- Switching between programmer and playbacks changes only the left icon and retains the existing text and behavior.
- The bottom dock shows one show/setup button, with no operator name and no Save desk button.
- The clock fits the available width and presents seconds compactly without making the dock wider.
- Built-ins/desks switching animates in 500 ms or less.
- A window never displays two redundant titles for the same content.
- Stage click, shift-click, control-click, drag, and control-drag produce the documented selection behavior.
- Pool windows expand to the pane width, and Preload Scene Release is immediately adjacent to Preload.
- Aligning pan left/right/center/out produces deterministic values and respects fixture limits and pan inversion.
