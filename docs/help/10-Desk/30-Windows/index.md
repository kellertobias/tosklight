# Windows and Panes

ToskLight Control uses reusable panes, temporary full built-ins, dedicated setup workflows, object editors, and sibling applications. [Desk Interface and Windows](01-desk-interface-and-windows.md) explains the shared window chrome and saved Desktop.

Panes are the building blocks of a saved Desktop. Open a pane from an empty grid cell, move it by its header, and use **Settings** in that header to configure only that pane. The same pane type may appear more than once with different settings.

Every pane has a **Pane Settings** tab. **Grid width** uses 1-24 columns, **Grid height** uses 1-18 rows, and **Remove pane** removes that instance without deleting show data. Pane positions and options are saved in the current user's Desktop layout.

Those common grid and removal controls work the same way for every pane and are documented only here. The reference shows a Pane Settings image only when that pane adds settings of its own.

The following pages document every operator choice currently offered by **Open Window**. They
include a settings screenshot only when that pane adds settings beyond the common controls above.
The Dynamics reference uses the production full-application story accepted during the Dynamics
visual-review checkpoint.

- [Programming and visualization windows](02-programming-and-visualization.md)
- [Cue and playback windows](03-cues-and-playbacks.md)
- [Output and Help windows](04-utility-and-diagnostics.md)
- [Programming Windows](06-programming-windows.md)
- [Channel Faders](07-channel-faders.md)

Use a full built-in window for temporary focused work. Use a pane when the surface should remain part of a reusable operator layout.

## Function map

<!-- table: rows-per-page=12; row-weight=3.7; continue-after-table -->
| Function | Normal surface | Purpose |
| --- | --- | --- |
| Fixtures | Fixture Sheet pane | Select fixtures, inspect values and ownership, and work by attribute columns. |
| Groups and Presets | Pool panes | Recall ordered selections and reusable attribute values. |
| Stage | Stage pane or built-in Stage | Select from 2D/3D, inspect the rig, and follow live or Preload state. |
| Dynamics | Dynamics pool and editor | Create reusable modulation, optionally tied to a fixed Group. |
| Cuelists and Cues | Cuelist Pool, Cue pane, Cuelist View | Store, order, time, and inspect Cues. |
| Playbacks | Physical/touch surface and Virtual Playbacks pane | Run assigned Cuelists, Groups, masters, Macros, and other functions. |
| Timecode | Timecode Pool and editor | Map timed events to an internal or selected external source. |
| Macros | Macro Pool and editor | Store and run desk command sequences. |
| Running & Output | Built-in diagnostic window | Inspect running objects, desk state, active programmers, and connected PreViz control. |
| Media | Media pane | Select Media layers and content; CITP enriches names and previews but is optional. |
| DMX | DMX pane | Inspect complete logical universes and final rendered channel values. |
| Help | Help pane | Read this same nested operator manual inside the Desk. |

Show Patch, Fixture Library, and Desk Setup are focused built-in workflows. ToskLight Architect's renderer and Rig Editor, and ToskLight Pixel's media-server surfaces, are separate applications rather than embeddable Control panes.
