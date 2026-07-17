# Virtual Playbacks

Virtual Playbacks place playback actions in a configurable on-screen grid.

Add a Virtual Playbacks pane and choose its row and column count. Assign a cell through **Set Source**, **Add Target**, or the normal `[SET]`, source, target sequence. Pressing `[SET]` and then a cell opens the same Playback Configuration modal used by physical controls; a virtual cell is a one-button, faderless playback target with an additional icon or image-background choice.

Virtual actions have their own Preload capture switch. This allows physical controls to remain live while virtual actions are queued, or the reverse. Test the chosen capture combination before operation.

Use Virtual Playbacks for task-specific buttons, not as a substitute for assigning and documenting the underlying playback. The target, action, page, and release behavior must remain understandable from playback configuration.

## Playback Exclusion Zones

An exclusion zone is a named set of cells where at most one assigned playback may be On. Hold Shift and select at least two cells, choose **Create Exclusion Zone**, and enter its name. Shift-selection and zone creation are configuration gestures: they do not press, start, or stop the selected cells.

When one member turns On, it wins and every other active member turns Off. Turning the winning member Off does not start another member. Touch, mouse, the F1–F8 current-page shortcuts, REST, OSC, and restored playback state all use this server-owned rule. Automatic full-override release remains a separate playback option.

Open the pane's **Settings → Virtual Playbacks** tab to rename a zone, change its cells, or delete it. A cell may belong to several zones; activating it releases the other members of all those zones. Changing playback page keeps the same cell positions and applies them to the new current-page assignments.

Zones persist with the active show, control desk, and virtual-playback surface. Moving the pane retains them. Shrinking the grid retains out-of-range cells as visible hidden memberships in Settings; expanding the grid restores those members. Sessions on the same control desk use the same zones. A different desk used by the same user has its own zones and button/page state, while programmer values remain shared at the user level.
