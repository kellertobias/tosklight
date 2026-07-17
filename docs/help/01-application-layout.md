# Application Layout and Window Manager

ToskLight separates permanent Desktops from temporary full-window tools. The same windows can usually appear either as panes on a Desktop or as a full built-in workspace.

## The shell

The left dock switches between **DESKTOPS** and **BUILT-INS**. Desktops are saved arrangements of panes. Built-ins open one tool at full size without changing the saved Desktop. The bottom command line, programmer controls, playback page controls, and Show menu remain available while moving between workspaces.

The Show menu contains show creation/loading, MVR exchange, Desk Setup, Help, development tools, system controls, and desk shutdown. Help itself is a split workspace: topics on the left and the selected Markdown page on the right.

## Desktops and panes

A Desktop uses a 24-column by 18-row grid. Create one with **New desktop** in the DESKTOPS dock. Open its settings to rename it, change its icon, clone the current layout, or delete it. At least one Desktop always remains.

Add a pane from the empty-cell picker or window picker. Drag a pane by its header to move it. Open the pane settings to set its exact grid position and size, maximize it, change window-specific options, or remove it. Panes cannot overlap; moves and resizes are constrained to the grid.

Some options belong to one pane rather than the whole application. Examples include Stage 2D/3D view and Follow Preload, Preset family and pool colors, Fixture ordering/filtering, the virtual-playback grid, and the selected text file.

## Built-in windows

The operator windows are:

- **Stage** - 2D/3D selection, visualization, fixture/scenery placement, and Preload following.
- **Fixtures** and **Channels** - current values, source ownership, selection, and fixture/channel detail.
- **Groups** and **Presets** - reusable selection and attribute pools.
- **Cuelists**, **Cues**, and **Playbacks** - stored scenes, timing, assignment, and execution.
- **Dynamics** - phaser and dynamic-attribute work.
- **DMX** - live universe output, raw overrides, and diagnostics. Output routes are configured in **Desk Setup > Outputs**.
- **Patch** - fixture IDs, modes, addresses, and multi-patch instances.
- **Virtual Playbacks** - a configurable grid of playback actions.
- **File Manager** and **Text Editor** - confined files exposed by the server.
- **Desk Setup** and **Help** - persistent configuration and operator documentation.

Development diagnostics are not an operator pane and do not appear in **Open Window**. Developers can open that surface from the **Desk Status** developer menu; persisted layouts that already contain it remain compatible.

Use the [Pane Reference](05-Pane-Reference/index.md) for a screenshot and settings explanation for every available pane. [Help Coverage](99-Development/02-help-coverage.md) maps all remaining application surfaces to their detailed pages.

## Multiple screens

Configure physical screens in **Desk Setup > Screens & playback**. Each screen has its own name, dimensions, touch capability, assigned desk, playback slot range, row count, and page mode. **Follow Main** mirrors the main playback page; **Dedicated Page** keeps an independent page. Screen configuration belongs to the desk installation, while show programming remains in the portable show file.

## Where settings are stored

Desktop layouts, screens, users, network inputs, and output configuration live in desk data. Fixture patch, stage layout, groups, presets, Cuelists, and playbacks live in the active show. An unfinished command, open ordered selection/source gesture, page, and button state belong to the control desk. Confirmed temporary values belong to the active user's Programmer and are shared by that user's sessions on every desk; they remain distinct from the show until recorded.
