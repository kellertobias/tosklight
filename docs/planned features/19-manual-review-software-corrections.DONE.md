# Manual Review Software Corrections

**Implementation status: Complete.** The operator UI, confined file workflows, manual/help source, deterministic screenshots, compatibility defaults, and executable acceptance coverage described here are implemented. The separate fixture channel-editor design remains intentionally scoped to [Fixture Channel Configuration](18-fixture-channel-configuration.md).

## Implementation and verification evidence

- Pane headers, Cues editing, Help/DMX/Stage responsibilities, Desktop terminology, Desk Setup organization, fixture-browser layout, `[SET]`-gated MIB editing, Change User spacing, diagnostics access, and safe-blackout show recovery are implemented in the production control UI.
- All operator file fields use the root-confined picker with caller-specific extension/cardinality rules; File Manager and Text Editor retain their completed service, conflict, persistence, and pane-settings behavior.
- The executable contracts are [`MANUAL-019`](../testing/10-desk-lock-and-operator-ui.md), [`FILE-001`, `FILE-002`, `FILE-016`, `TEXT-001`, and `TEXT-015`](../testing/09-file-manager-and-text-editor.md), plus the focused Playwright suites under `tests/` and UI unit coverage for legacy pane hydration, picker accessibility, output routes, and recovery.
- Deterministic help screenshots were refreshed. The PDF manual was rebuilt as `output/pdf/tosklight-manual.pdf`, with rendered-page inspection artifacts retained under `tmp/pdfs/`; the same Markdown and keycap-spacing rules feed HTML and in-application Help.

## Pane reference and screenshot policy

- Explain the common **Grid width**, **Grid height**, and **Remove pane** controls once at the start of the Pane Reference.
- Do not embed a Pane Settings screenshot when a pane adds no settings beyond those common controls.
- Keep pane-specific settings screenshots only where they explain an actual pane option.
- Generate representative screenshots from a deterministic show: Presets and Groups must contain useful stored examples, Virtual Playbacks must contain assignments, the Text Editor must have a file open, the DMX pane must have a patched channel selected, and Cue/Cuelist screenshots must show populated data.
- Keep the Development pane available only through developer/help tooling; do not include it as an operator pane in the manual.

## Cues and playbacks

### Cues pane

The actual Cues-of-a-Cuelist pane, distinct from the Cuelist Pool, must include the right-hand Cue information/editor section. Its pane representation must not omit controls that make Cue inspection and editing useful.

Remove the dangerous **Delete Cue** button from the Cue timing/trigger editor. Cue deletion remains an explicit command-line operation using `[DEL]` and a complete Cue address.

### Virtual Playbacks

Pane Settings contains only the logical **Rows** and **Columns** configuration plus exclusion-zone management after that feature exists. Remove the per-cell Cuelist/action assignment list.

Assigning a Virtual Playback follows the standard playback workflow:

- **Set Source**, **Add Target**; or
- `[SET]`, select the source, then select the Virtual Playback target.

Pressing `[SET]` and then an existing or empty Virtual Playback opens the same Playback Configuration modal used by every other playback. A Virtual Playback is a one-button, faderless playback target. Its configuration additionally supports an icon or image background without creating a separate assignment model.

Named mutual-exclusion behavior is specified separately in the completed [Virtual Playback Exclusion Zones](17-virtual-playback-exclusion-zones.DONE.md) contract.

## File Manager

- Darken the center directory-content area so it is visually distinct from navigation and properties.
- Move File Manager actions into the window title/header.
- Provide **Edit** as a dropdown containing Rename, Copy, Move, and Delete.
- Provide **Create** as a dropdown containing New File and New Folder.
- Provide **View** as a dropdown containing List, Grid, and the file-properties visibility toggle.
- Place Back and Forward beside View.
- Show the current root-relative path statically beside the **File Manager** title.
- Move **Show Hidden** into File Manager Pane Settings.
- Preserve the existing root confinement, selection, conflict handling, trash/permanent-delete distinction, and click-to-claim desk-key routing.

The reusable in-application file picker remains part of the completed File Manager contract in [File Manager](16-file-manager.DONE.md).

## Text Editor

- Put **Open File**, **Refresh**, **Save**, and **Save As** in the window title/header.
- **Open File** uses the in-application File Manager picker.
- Persist the open file with the pane and restore the useful view position, including scroll/cursor state, when the operator returns.
- Add a Pane Setting for read-only versus read-write operation. Read-only must prevent writes even when the underlying file is writable.
- Add Pane Settings for plain text, rendered Markdown, and a two-column Markdown view with the editor on the left and rendered output on the right.
- Retain visible Saved, Unsaved, Missing, Read-only, and Conflict states and protect unsaved text.

## DMX, Help, and Stage panes

- Capture the DMX pane with a patched DMX channel selected so the selected-fixture sidebar is visible.
- Keep Help as two columns in a pane: navigation on the left and topic content on the right. It must not switch to two stacked rows merely because it is embedded as a pane.
- Move built-in Stage-element selection out of Pane Settings. **Add Element** opens the appropriate chooser at the point where an element is added.
- Clean up the Programmer pane's stray separator lines and ensure adjacent rendered key/button images have visible spacing without overlapping surrounding text.

## Desktop terminology

Rename saved UI workspace arrangements from **Desks** to **Desktops** on every operator-facing surface, including the dock, creation action, settings dialog, Help, screenshots, and accessibility labels. Internal persisted identifiers may remain `desk` where a migration would add risk.

Use **desk** for the physical/logical combination of ToskLight software and attached control hardware. Do not rename control-desk aliases, desk boundary tokens, or protocol concepts that refer to that physical desk.

## Desk Setup and protocol organization

- Start the Desk Setup manual chapter with the Show menu visible and **Enter Setup** highlighted so the navigation path is obvious.
- Add a later manual section that describes the OSC, REST, and WebSocket protocols; link the Desk Setup network page forward to it.
- Move output-route creation and editing under **Desk Setup > Outputs**. The DMX pane remains a live value/override monitor rather than the route-configuration owner.
- Add vertical spacing in Change User so the existing-user list, new-user input, and action button are visually separated.
- Place a File Manager below the summary area in **Shows & recovery**. It shows available show files and permits loading a show through that controlled browser.

## Fixtures, patch, and library

- Replace inline MIB and MIB Delay controls in the fixture table with the same selected-cell and `[SET]` editing model used by other editable table values. Merely clicking a checkbox or inline input must not overwrite show data.
- Left-align manufacturer and fixture names in Add Fixture and Fixture Library browsers. Right-align secondary/detail text so names and metadata form readable columns.
- Put search and neighboring actions in the modal/window title for Add Fixture and Fixture Library, using the shared browser component where possible.
- The fixture channel editor requires a separate design pass documented in [Fixture Channel Configuration](18-fixture-channel-configuration.md).

## Standard file-picker field

All ToskLight form fields that select files or folders use the File Manager picker by default. They must not open an operating-system picker directly.

Desk settings may enable a native-picker fallback. When enabled, the ToskLight picker exposes an explicit secondary **Open system file picker** action while retaining the caller's file/folder, extension, and cardinality constraints. Existing native inputs for GDTF, MVR, show files, Stage scenes/assets, and wallpapers migrate to this standard field.

## Command-line Help and key rendering

- Make the typical software-desk figure show the complete relevant command-line/key layout, not only a cropped number block.
- Document `[AT][AT]` and `[.][.]` with the key definitions instead of below the layout discussion.
- Ensure PDF, HTML, and in-application keycaps do not overlap text or each other. Adjacent buttons need visible separation.
- Correct Speed Group grammar:
  - `[SHIFT][TIME] 1 [AT] 120 [ENT]` sets Speed Group A to 120 BPM.
  - `[SHIFT][TIME] 1 [AT] [+] 5 [ENT]` and `[AT][-] 5` make relative changes.
  - `[SHIFT][TIME] 1 [AT] [SHIFT][TIME] 2 [ENT]` synchronizes Speed Group A to Speed Group B using the documented source/target direction and phase behavior.
- Keep Help, the command-line reference, and executable `CMD-002` acceptance wording aligned as the Speed Group command surface evolves.

## Compatibility and acceptance

- Persisted layouts without new pane fields must load deterministic defaults.
- Renaming the operator concept to Desktop must not break physical desk aliases, sessions, OSC routing, or existing saved layouts.
- Virtual Playback assignment must reuse authoritative playback state and remain consistent across software, keyboard, OSC, and attached hardware paths.
- File and show pickers remain confined to configured roots and must visibly report missing/disconnected locations.
- Regenerate screenshots only after the corresponding UI is implemented. Build and visually inspect the PDF and HTML manuals after the documentation images are refreshed.
