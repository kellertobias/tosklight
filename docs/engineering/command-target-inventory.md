# Command target inventory

This inventory describes the command-target behavior implemented by the desk. It is an engineering
baseline for a shared Record/Set/Copy/Move/Delete target-selection system; it is not a claim that the
current surfaces already have uniform gestures or styling.

The implementation is authoritative where operator Help, tests, and code disagree. The operator
grammar remains documented in
[Command Line Reference](../help/10-Desk/20-Programmer-and-Cues/01-command-line.md).

## Shared routing that exists today

- Software SET is routed through the priority-based control-surface registry in
  `apps/light-desktop/src/features/controlSurfaceInteraction/registry.ts`. A constrained Cue editor
  can own SET at priority 300, pane-specific legacy SET handlers use priority 100, and the desk-wide
  SET state machine uses priority 50.
- The desk-wide state machine in `setInteraction.ts` supports bare SET, a selected Group source, a
  Playback target, Group Master assignment, Group settings on Enter, Clear, and Cancel. It carries
  exact desk, show, surface, object, page, and revision identities.
- Record, Update, Set, Copy, Move, and Delete target styling is reusable across pool cards. Every
  active state carries a literal workflow badge, so color is not the only marker.
- Copy, Move, and Delete pointer phases are enabled only for existing authoritative mutations.
  Presets expose occupied sources and empty same-family destinations; Groups expose Delete; File
  Manager retains its local source/destination namespace. Unsupported combinations remain full-entry
  only and receive no outline.
- Computer-keyboard, software-keypad, and OSC keypad input share the command grammar. That parity
  does not automatically make a later OSC Playback press a command target; Playback interception is
  a separate route.

## SET plus pointer target inventory

“Full entry” means completing the operation through the command line or a dedicated editor without
using the target-selection pointer gesture. “Right-click” means the browser/Tauri context-menu
gesture, not a touchscreen hold.

| Surface and target | SET plus click/touch | Full-entry or direct equivalent | Right-click or hold | Target indication while armed | Normal action that is overridden |
| --- | --- | --- | --- | --- | --- |
| Group Pool card | Bare SET plus an occupied Group chooses that exact Group revision as a Group Master source. The next Playback target assigns it. | `SET GROUP <number> ENTER` opens Group settings. A pending Group source plus Enter opens the same settings. | Right-click and 600 ms hold open Group settings; they do **not** choose a Group Master source. | Every occupied source receives the complete cyan SET outline and literal badge. | The card's normal live Group selection. Double-click frozen selection remains a separate direct gesture. |
| Embedded Group shortcut | Same whole-card Group Master source behavior as the Group Pool. | Same `SET GROUP <number> ENTER` form. | Right-click opens Group settings. No hold shortcut is implemented here. | Every occupied source receives the complete cyan SET outline and literal badge. | Live Group selection. |
| Touch Playback card | Bare SET plus any point inside the whole card, including nested button/fader content, opens configuration for that exact page and slot. A pending Group source assigns a Group Master. | `SET <page>.<playback> ENTER` opens Playback Configuration. A complete assignment command can assign a Cuelist or Dynamic. | Right-click anywhere on the card opens the same Playback Configuration. Shift plus the first Playback button also opens it. | Cyan full-card configuration overlay. A selected Cuelist source uses the cyan assignment overlay. | Playback button action, fader action, and card selection are suppressed before the target is handled. |
| Hardware-layout Playback card in the desktop UI | Bare SET plus any point in the complete card opens configuration; a pending Group source assigns. Nested hardware-style buttons, labels, and fader belong to the card target. | Same complete Playback commands as the touch card. | Right-click resolves the exact card and opens configuration. | Cyan complete-card outline and literal **SET TARGET** badge. | Card selection and every nested control action are suppressed for the complete gesture. |
| Virtual Playback cell | Bare SET plus the whole cell opens configuration for the exact virtual Playback. A pending Group source assigns a Group Master. | The Playback Configuration modal is the complete editor. Typed current-page/explicit-page forms do not use the virtual number as a page slot address. | Right-click opens Playback Configuration. | Cyan complete-cell outline with a literal **SET TARGET** workflow prefix. | The cell's configured action, including held Flash/Swap behavior. |
| Cuelist Pool card | SET plus an occupied Cuelist selects it as the source for the next Playback assignment. Empty slots reject the source choice until recorded. | `SET <Cuelist-number> AT <page>.<playback> ENTER` assigns; `SET <Cuelist-number> ENTER` configures the Cuelist. | A 650 ms hold opens Cuelist settings. Right-click only suppresses the native menu and has no equivalent action. | Only the chosen source card receives the cyan `set-target` outline. Playback destinations then show the assignment overlay. | Opening the occupied Cuelist's Cue view. |
| Preset Pool card | SET plus the whole occupied or empty slot opens its desk-local title, icon, and button-color editor. | No command-line form edits this desk-local presentation. | No context-menu or hold equivalent is implemented. | Every eligible Preset card receives the cyan `set-target` outline until one is chosen. | Preset recall; an empty slot's normal inert action is also replaced. |
| Dynamics Pool tile | SET plus the whole populated tile writes `SET DYNAMIC <number>` as the source for a later Playback assignment. | `SET DYNAMIC <number> PLAYBACK <number>` completes the assignment. | Shift-click, hold, or right-click opens the Dynamic editor; those gestures do **not** choose an assignment source. | Populated tiles receive the complete SET outline and literal badge. | Toggling the Dynamic on the current selection. |
| Show Patch split/address | Select a split and press SET, or press SET and touch a split/address control, to open Fixture Address for the exact physical split. | The Fixture Address screen provides the complete `universe.address` number block and 512-slot map. | No right-click equivalent for address. | Every eligible split/address button receives its own SET outline and literal badge. | Without SET, touching a split selects its fixture/split for ordinary programming selection. |
| Show Patch nested fixture fields | SET plus Name, Fixture/Mode, Masters, Pan/Tilt, MIB, light source, Location X/Y/Z, Rotation X/Y/Z, or Layer targets that exact nested value. The available Multi-patch targets remain instance-specific. | Each target opens its own complete modal/editor. | Only Location and Rotation axes implement right-click direct entry. Other nested fields have no context-menu equivalent. | Every eligible nested button receives its own SET outline and literal badge; the fixture row is not outlined. | The row's normal selection path is bypassed when the nested SET editor opens. |
| Constrained selected-Cue sidebar value | When the full Cue form cannot fit, SET arms only the compact fallback. Touching Title, In/Out Delay/Fade, Trigger, Link Cue, or Trigger/Link time opens that exact editor. | The full-height Cue sidebar exposes the same fields directly without SET. | No right-click equivalent. | Every eligible nested button becomes cyan/active; the whole Cue row or sidebar is not the target. | Before SET, the fallback value buttons are intentionally inert. The local priority-300 owner overrides global SET handling. |
| File Manager instance and entry | Bare SET means Rename. The first pointer inside one File Manager claims that whole instance; a following entry click chooses the one rename source. This is surface ownership followed by source selection, not SET plus one terminal card target. | The File Manager Edit menu starts the same Rename operation. Enter completes it and Escape cancels it. | No entry context menu is implemented. | Once the instance owns the operation, every eligible complete entry receives the SET outline and literal badge. | The ownership click is consumed instead of opening/selecting an entry. During the operation, entry clicks select the source instead of opening files or folders. |

## Record/Set/Copy/Move/Delete coverage by target family

The following table distinguishes a working target gesture from a complete typed command. A
complete typed command is parity for the stored result, but not proof of pointer or attached-control
target parity.

| Target family | Record | Set | Copy | Move | Delete |
| --- | --- | --- | --- | --- | --- |
| Group | Whole pool card/shortcut. Empty and stored-empty Groups record directly; populated Groups open the record-mode choice. | Whole occupied card chooses a Group Master source; Enter opens settings. | No card workflow or typed Group copy command. | No card workflow or typed Group move command. | Bare Delete outlines every occupied card; a touch executes `DELETE GROUP <number>` and preserves dependency rejection. |
| Preset | Whole card; empty records directly and populated opens the record-mode choice. | Whole card opens presentation settings. | Bare Copy outlines occupied sources, then only empty same-family destinations. | Bare Move uses the same source/destination phases and removes the successful source. | Bare Delete outlines occupied complete cards. |
| Cuelist/Cue | Cuelist card creates/appends; Playback card records to its assigned Cuelist. A specific Cue requires a complete address. | Every populated Cuelist card is a whole-card assignment source while Set is armed; compact Cue values are exact nested Set targets. | Bare Copy outlines whole Cue rows, records a complete source address, and outlines rows as destinations after AT. | Bare Move uses the same complete-address source/destination row flow. | Bare Delete outlines whole Cue rows and executes the complete Cue address; there is still no separate Delete button. |
| Physical/touch Playback | The whole card and its nested controls are captured as one Record target. | Whole touch and hardware-layout card, including every nested button, label, and fader; exact page/slot identity. | No Playback-card Copy target. | No Playback-card Move target. | No Playback-card Delete target; unassignment is an explicit Playback Configuration action. |
| Virtual Playback | No Record-to-cell workflow. | Whole cell configures or receives a Group Master. | No cell Copy target. | No cell Move target. | No cell Delete target; unassignment is explicit configuration. |
| Dynamic | Record and Update are rejected before the normal tile action. | Whole tile chooses an assignment source. | Copy is available inside the Dynamic editor, not as Copy plus tile. | Move is available inside the Dynamic editor, not as Move plus tile. | Exact `DELETE` plus an occupied Dynamic tile deletes it. This is a local exception, not generic command targeting. |
| Show Patch | Patch programming is not a Record-card workflow. | Nested fixture/split/value targets. | No command target. | No command target. | Patch's local Delete control plus a row opens Delete/Unpatch/Abort confirmation. |
| Compact Cue value | No nested Record target. | Nested value target only. | No nested Copy target. | No nested Move target. | No nested Delete target. |
| File Manager | Not a Record target. | Bare SET is the instance-owned Rename workflow. | Bare Copy selects file sources and uses the current directory as destination. | Bare Move selects file sources and uses the current directory as destination. | Bare Delete selects file sources and opens File Manager confirmation. |

## Current target indication

| Command | Current visible treatment | Known omissions |
| --- | --- | --- |
| Record | Standard red complete-entry outline and literal badge on every recordable Preset, Cuelist, Group, and Playback target. | Virtual Playbacks cannot be Record targets. Cue-row integration is deferred while TL-91 owns the Cuelist table. |
| Set | Standard cyan complete-entry treatment on Pools and File Manager, purpose-built complete-card Playback/Virtual Playback overlays, exact nested Patch/Cue-value controls, and populated Cuelist cards. | Pane titles and Cue rows have no Set mutation and remain ordinary navigation. |
| Copy | Literal complete-entry treatment on Preset sources/destinations, File Manager sources, and complete Cue rows. | Whole Cuelists and Playbacks have no Copy target. |
| Move | Literal complete-entry treatment on Preset sources/destinations, File Manager sources, and complete Cue rows. | Whole Cuelists and Playbacks have no Move target. |
| Delete | Literal complete-entry treatment on occupied Group, Preset, Dynamic, File Manager, complete Cue rows, and Pane titles. | Playback unassignment and Patch deletion retain their explicit configuration/local modes; the attached-encoder release remains a hardware-only exception. |

Update is outside the five-command scope, but its existing amber `update-target` treatment and
whole-card interception are the closest reusable precedent. Record red, Update amber, and SET cyan
must remain distinct.

## Keyboard, OSC, and attached hardware

### Shared command entry

- Software keypad, computer-keyboard shortcuts, and
  `/light/{desk}/programmer/{key}` OSC addresses enter the same Set, Record, Copy, Move, Delete,
  number, AT, Group, Cue, and Enter grammar.
- Consequently all documented **complete** command forms can be entered from attached keypad/OSC
  controls. This is command-line parity only.
- Computer F1-F8 normally operate the first eight current-page Playbacks. While bare SET or a Group
  source is pending, they become exact Playback targets instead.

### Attached Playback controls

- Bare Record is intercepted authoritatively for current-page, explicit-page, and Virtual Playback
  OSC addresses. Touching a Playback button, label, or fader records to that Playback and suppresses
  both the normal action and matching release/continuous samples.
- Update has the same target-first precedence for eligible Playback controls.
- Bare SET is intercepted by backend OSC Playback handling for current-page and explicit-page
  Playback controls. It suppresses the complete physical gesture and publishes the exact target to
  the shared desk configuration flow. Pending Group-source assignment uses the same target-first
  boundary.
- Copy, Move, and Delete are also not intercepted by physical Playback controls. Their complete
  command forms remain available through the keypad.

### Reserved navigation

- Computer Page Up/Page Down always changes the authoritative Playback page; it is never a command
  target.
- F9-F13 remain Speed Groups A-E. F1-F8 change role only for the explicit SET/Group-pending override
  above.
- Software and attached Shift-number shortcuts remain window/desktop navigation. They are consumed
  before ordinary target entry.
- Attached Page Up/Page Down, the NAV encoder, MENU, PROG/PLAYBACK, and ESCAPE remain navigation or
  desk actions rather than targetable cards.
- A selected Cuelist's Pool/card number and a page's navigation controls are not interchangeable:
  Cuelist source selection must not consume Page Up/Down or page-picker actions.

## Adjacent local command owners

These surfaces use command keys but are not evidence of a shared show-object target system:

- File Manager claims Rename/Set, Copy, Move, and Delete at surface level. Entry clicks select
  sources, while Copy/Move destinations are the current directory. Once claimed, complete entries
  carry the literal active-command outline in addition to ordinary source selection.
- Exact Delete makes a Pane title remove that pane after confirmation. The pane body is not a target.
- Exact Delete plus an attached encoder press releases that encoder's scoped Programmer value. There
  is no matching software encoder-click target.

## Requirements for shared targeting work

1. Represent the active operation, target phase, source identity, destination identity, originating
   desk/show/surface, and revision explicitly. Do not infer a destination from selection or the
   currently visible page after the target was chosen.
2. Run command interception before every conflicting normal click, button, fader, hold, or release
   action. Suppress the complete physical gesture lifetime, not only its initial press.
3. Outline the actual hit target: the complete card for pool/playback operations and the exact
   nested button/cell for Patch or Cue-value operations. Do not outline a surrounding grid when only
   one nested value will be changed.
4. Keep Record, Set, Copy, Move, and Delete visually distinct and non-color-only. Reuse the PoolCard
   workflow badge pattern, with literal command names.
5. Define context-menu equivalence per terminal action. Right-click may synthesize SET plus the same
   target only when that sequence terminates in the same action. Group and Dynamic source selection
   currently differs from their right-click settings/editor action and must not be silently treated
   as equivalent.
6. Preserve all reserved page, view, desk, and navigation presses. Only an explicitly documented
   target-selection override may replace a normal Playback or pool-card action.
7. Prove software mouse, touch, computer keyboard, OSC, and attached hardware separately. Calling a
   frontend state-machine method with source `hardware` is not attached-hardware ingress proof.
8. Preserve full-entry commands as the unambiguous path for specific Cue, Copy, Move, and Delete
   addresses even if pointer source/destination selection is later added.

## Implementation and test anchors

- Target routing and SET state: `apps/light-desktop/src/features/controlSurfaceInteraction/`
- Active pane SET ownership: `apps/light-desktop/src/state/AppContext.tsx`
- Record/Set/Update pool states: `apps/ui-library/src/pools/PoolCard.tsx` and
  `apps/ui-library/src/styles/operator-surfaces.css`
- Group targets: `apps/light-desktop/src/windows/groupsWindow/GroupPoolGrid.tsx` and
  `apps/light-desktop/src/components/shared/GroupStrip.tsx`
- Preset targets: `apps/light-desktop/src/windows/PresetsWindow.tsx`
- Cuelist targets: `apps/light-desktop/src/windows/cuelistWindow/CuelistPool.tsx`
- Playback precedence: `apps/light-desktop/src/components/control/playbackFaderBank/slotActions.ts`
- Virtual Playback targets: `apps/light-desktop/src/components/control/virtualPlayback/`
- Patch nested SET targets: `apps/light-desktop/src/components/setup/fixturePatch/`
- Compact Cue targets: `apps/light-desktop/src/windows/cuelistWindow/CueProperties.tsx`
- OSC keypad and Playback interception: `crates/light/adapters/headless/src/runtime/osc_highlight.rs`,
  `crates/light/adapters/headless/src/runtime/osc_playback.rs`, and
  `crates/light/adapters/headless/src/command_http/cue_recording_osc.rs`
- Focused tests: `SetInteractionProvider.test.tsx`, `PlaybackFaderBank.test.tsx`,
  `VirtualPlaybackGrid.setRouting.test.tsx`, `FixturePatchSetup.control.test.tsx`,
  `CuelistWindow.test.tsx`, `PresetsWindow.recording.test.tsx`, and
  `command_http_cue_convergence_tests.rs`
