# Application Layout and Window Manager

ToskLight separates permanent desk layouts from temporary full-window tools. The same windows can usually appear either as panes in a desk or as a full built-in workspace.

## The shell

The left dock switches between **DESKS** and **BUILT-INS**. Desks are saved arrangements of panes. Built-ins open one tool at full size without changing the saved desk. The bottom command line, programmer controls, playback page controls, and Show menu remain available while moving between workspaces.

The Show menu contains show creation/loading, MVR exchange, Desk Setup, Help, development tools, system controls, and desk shutdown. Help itself is a split workspace: topics on the left and the selected Markdown page on the right.

## Desks and panes

A desk uses a 24-column by 18-row grid. Create a desk with **New desk** in the DESKS dock. Open its settings to rename it, change its icon, clone the current layout, or delete it. At least one desk always remains.

Add a pane from the empty-cell picker or window picker. Drag a pane by its header to move it. Open the pane settings to set its exact grid position and size, maximize it, change window-specific options, or remove it. Panes cannot overlap; moves and resizes are constrained to the grid.

Some options belong to one pane rather than the whole application. Examples include Stage 2D/3D view and Follow Preload, Preset family and pool colors, Fixture ordering/filtering, Development view, virtual-playback grid, and the selected text file.

## Built-in windows

The operator windows are:

- **Stage** - 2D/3D selection, visualization, fixture/scenery placement, and Preload following.
- **Fixtures** and **Channels** - current values, source ownership, selection, and fixture/channel detail.
- **Groups** and **Presets** - reusable selection and attribute pools.
- **Cuelists**, **Cues**, and **Playbacks** - stored scenes, timing, assignment, and execution.
- **Dynamics** - phaser and dynamic-attribute work.
- **DMX** - universe output, routes, raw overrides, and diagnostics.
- **Patch** - fixture IDs, modes, addresses, and multi-patch instances.
- **Virtual Playbacks** - a configurable grid of playback actions.
- **File Manager** and **Text Editor** - confined files exposed by the server.
- **Desk Setup**, **Help**, and **Development** - persistent configuration, documentation, and diagnostics.

Use the [Pane Reference](05-Pane-Reference/index.md) for a screenshot and settings explanation for every available pane. [Help Coverage](99-Development/02-help-coverage.md) maps all remaining application surfaces to their detailed pages.

## Multiple screens

Configure physical screens in **Desk Setup > Screens & playback**. Each screen has its own name, dimensions, touch capability, assigned desk, playback slot range, row count, and page mode. **Follow Main** mirrors the main playback page; **Dedicated Page** keeps an independent page. Screen configuration belongs to the desk installation, while show programming remains in the portable show file.

## Where settings are stored

Desk layouts, screens, users, network inputs, and output configuration live in desk data. Fixture patch, stage layout, groups, presets, Cuelists, and playbacks live in the active show. Programmer selection and temporary values belong to the active operator session and remain distinct from both until recorded.
