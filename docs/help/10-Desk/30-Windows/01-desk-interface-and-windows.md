# Desk Interface and Windows

ToskLight separates permanent Desktops from temporary full-window tools. The same windows can usually appear either as panes on a Desktop or as a full built-in workspace.

## The shell

The left dock switches between **DESKTOPS** and **BUILT-INS**. Desktops are saved arrangements of panes. Built-ins open one tool at full size without changing the saved Desktop. The bottom command line, programmer controls, playback page controls, and Show menu remain available while moving between workspaces.

The Show menu contains show creation/loading, MVR exchange, Desk Setup, Help, development tools, system controls, and desk shutdown. Help itself is a split workspace: topics on the left and the selected Markdown page on the right.

The **DMX** control beside the command line opens **Running & Output**. Its title-bar tabs keep **Running**, **Desk State**, and **Active Programmers** in one modal; a **Visualizer** tab joins them only while an external Visualizer is connected. It opens on Running unless a warning routes directly to Desk State. **All Off** stays in the title bar on every tab and asks for confirmation before it releases every running playback, dynamic, and Programmer Preload. A red triangle with a white exclamation mark beside DMX means Desk State needs attention. Rejected commands stay marked on the current command line, while connection and critical desk failures use the top status message.

Every desk window carries the same window strip with **X**, fullscreen, and move controls. On the
main window the **X** quits ToskLight, because that window is the desk itself. On an optional screen
window the **X** closes only that screen; the screen keeps its configuration and reopens from
**Desk Setup → Screens**.

Title-bar buttons within one action group use a single gray divider. The boundary between action groups uses a gray, light-blue, gray divider, making related controls visible without inserting a black gap.

## Search bars

Search uses the same title-bar control in modals and regular windows. The search group is right-aligned immediately before every defined window or modal action. Its fixed order is magnifying glass, text, **X** while text is present, and the keyboard button. When a search offers additional options, the leading magnifying-glass area widens to include a chevron; press that leading area to open Options in a stacked dialog above the owning window or modal. Searches without options show the magnifying glass without a chevron or button behavior.

Typing filters immediately unless the feature page explicitly documents a different search operation. **X** clears the query, and the keyboard button opens the shared full-text keyboard.

## Desktops and panes

A Desktop uses a 24-column by 18-row grid. Create one with **New desktop** in the DESKTOPS dock. Open its settings to rename it, change its icon, clone the current layout, delete it, or customize the desk's shared pool-color defaults. At least one Desktop always remains.

Add a pane from the empty-cell picker or **Open Window**. Open Window groups descriptive pane choices into **Programming**, **Playback & Automation**, **Show & Visual**, and **Miscellaneous** title tabs. Macro Pool is under Programming. Running, Scheduler, File Manager, Help, and Text Editor are under Miscellaneous. Cuelists is the single catalog entry for the integrated Pool, Cues, and Cuelist Settings workflow. Drag a pane by its header to move it. Open the pane settings to set its exact grid position and size, maximize it, change window-specific options, or remove it. Panes cannot overlap; moves and resizes are constrained to the grid.

Some options belong to one pane rather than the whole application. Examples include Stage 2D/3D view and Follow Preload, Preset family and pool color mode, Fixture ordering/filtering, the virtual-playback grid, and the selected text file. Pool panes can use **Type colors** or **Individual colors**. Type colors use the configured object-type or Preset-family default. Individual colors show an item's explicit presentation color and use grey when none is assigned.

## Built-in windows

The operator windows are:

- **Stage** - 2D/3D selection, visualization, fixture/scenery placement, and Preload following.
- **Fixtures** and **Channels** - current values, source ownership, selection, and fixture/channel detail.
- **Groups** and **Presets** - reusable selection and attribute pools.
- **Cuelists**, **Cues**, and **Playbacks** - stored scenes, timing, assignment, and execution.
- **Dynamics** - animated attribute pools, editing, and running-source control.
- **DMX** - live universe output, raw overrides, and diagnostics. Output routes are configured in **Desk Setup > Outputs**.
- **Patch** - fixture IDs, modes, addresses, and multi-patch instances.
- **Virtual Playbacks** - a configurable grid of playback actions.
- **File Manager** and **Text Editor** - confined files exposed by the server.
- **Desk Setup** and **Help** - persistent configuration and operator documentation.

**Running** is a pane-only window: add it to a Desktop with **Open Window**. It does not appear in the BUILT-INS dock. Hold **Shift** in BUILT-INS to reveal the alternate destinations: **Stage** becomes **Media**, **Fixtures** becomes **Groups**, **Cuelists** becomes **Timecode**, **Dynamics** becomes **Macros**, and **Channels** becomes **DMX**. Releasing **Shift** restores the normal destinations. Timecode and Macro Pool also remain available through **Open Window**.

Development diagnostics are not an operator pane and do not appear in **Open Window**. Developers can open that surface from the **Desk Status** developer menu; persisted layouts that already contain it remain compatible.

Use the [Pane Reference](index.md) for a screenshot and settings explanation for every available pane. [Help Coverage](../../99-Development/02-help-coverage.md) maps all remaining application surfaces to their detailed pages.

## Multiple screens

Configure physical screens in **Desk Setup > Screens & playback**. Each optional screen has its own
name, physical window placement, playback layout, page mode, and either a configurable Desktop or
a view-only **Fixed full-screen pane**. **Follow Main** mirrors the main playback page;
**Dedicated Page** keeps an independent page. Screen configuration belongs to the desk
installation, while show programming remains in the portable show file.

## Where settings are stored

Desktop layouts, screens, users, network inputs, output configuration, and pool presentation defaults live in desk data. Pool presentation keys include the active show identity so equal object numbers from different shows do not share an override; the preferences do not travel in a show file. Fixture patch, stage layout, groups, presets, Cuelists, and playbacks live in the active show. An unfinished command, open ordered selection/source gesture, page, and button state belong to the control desk. Confirmed temporary values belong to the active user's Programmer and are shared by that user's sessions on every desk; they remain distinct from the show until recorded.
