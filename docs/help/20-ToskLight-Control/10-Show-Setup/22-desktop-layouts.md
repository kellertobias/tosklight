# Build task Desktops

Create separate Desktops for common jobs such as Programming, Playback, Patch, and diagnostics. Add only the panes needed for that job. Use full built-ins for temporary work that should not change a Desktop. Layout changes autosave with the show's Desktop definitions.

Hold a Desktop name to open its settings. Choose **Record pane positions** on the current Desktop to capture its pool and list scroll areas. Later scrolling leaves that recorded snapshot unchanged. Select the Desktop again to recall its recorded positions after the content loads.

Recording covers panes using the standard pool/list scroll area, including Groups and Presets. Custom editors and tables keep their own navigation state.

Each Desktop keeps its own positions. Pools remember the first visible slot, so recalling a Desktop at a different width returns to that working area. **Clone current desktop** captures the current pane positions in the new Desktop while preserving the source Desktop's recorded snapshot.

Recorded positions survive reopening the show. A Desktop without recorded positions keeps its ordinary initial scroll location. Recording pane positions does not capture fixture selection or Programmer values.

Desktop layout controls arrange operator panes; they do not define fixture selection geometry.
There is no built-in Layout pane or window. Use Show Patch or Architect CAD to edit fixture X/Y/Z positions, Group
settings to define shared Projection and Phase ranking, and a Dynamic's Projection tab for a local
override. Older Desktops that contained the retired Layout pane open without it and keep their
remaining panes.

For pane geometry and available windows, see [Application Layout and Window Manager](../30-Windows/01-desk-interface-and-windows.md).
