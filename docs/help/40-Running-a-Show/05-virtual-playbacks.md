# Virtual Playbacks

Virtual Playbacks place playback actions in a configurable on-screen grid.

Add a Virtual Playbacks pane and choose its row and column count. Cell 1 addresses Virtual Playback 1001, cell 2 addresses 1002, and so on through 9998. These are dedicated page-qualified assignments: for example, Virtual 1.1001, Virtual 2.1001, and physical Playback 1 are three independent controls.

Choose **Follow Main** to display the control desk's current main page, or **Pinned** to keep the pane on one fixed page. Changing the effective page changes only the addresses shown; it never operates a Playback. Right-click a cell, or press `[SET]` and then press the cell, to open the same Playback Configuration modal used by physical controls. A virtual cell is a one-button, faderless playback target with an additional icon or image-background choice.

Virtual actions have their own Preload capture switch. This allows physical controls to remain live while virtual actions are queued, or the reverse. Test the chosen capture combination before operation.

Use Virtual Playbacks for task-specific buttons, not as a substitute for assigning and documenting the underlying playback. The target, action, page, and release behavior must remain understandable from playback configuration.

## Playback Exclusion Zones

An exclusion zone is a named set of cells where at most one assigned playback may be On. Hold Shift and select at least two cells, choose **Create Exclusion Zone**, and enter its name. Shift-selection and zone creation are configuration gestures: they do not press, start, or stop the selected cells.

Saved zone members have an amber fence. Directly neighboring members share one outer fence with no internal fence edge. Disconnected members are outlined as separate islands, so the grid shows each connected part of the zone without relying on color alone.

When one member turns On, it wins and every other active member turns Off. Turning the winning member Off does not start another member. Touch, mouse, OSC, and restored playback state all use this server-owned rule. Virtual Playbacks do not use the physical F1–F8 Playback shortcuts. Automatic full-override release remains a separate playback option.

Open the pane's **Settings → Exclusion Zones** tab to rename or delete a zone. Choose **Edit Zone** to close Settings and select its cells on the live grid. The window title then offers **Update Exclusion Zone** and **Cancel Edit**. A cell may belong to several zones; activating it releases the deduplicated union of the other members. Zone membership stores cell positions. A Follow Main zone resolves those positions on the desk's current page; a Pinned zone resolves them on its fixed page.

Zone configuration is stored once for the active show, partitioned by control desk and virtual-playback surface. Moving the pane retains its surface partition. Shrinking the grid retains out-of-range cells as visible hidden memberships in Settings; expanding the grid restores those members. Sessions on the same control desk use the same partition. A different desk used by the same user has its own zone partition and button/page state, while programmer values remain shared at the user level.
