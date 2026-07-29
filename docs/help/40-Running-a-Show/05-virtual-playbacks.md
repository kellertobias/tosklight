# Virtual Playbacks

Virtual Playbacks place playback actions in a configurable on-screen grid.

Add a Virtual Playbacks pane and choose its row and column count, up to 20×15 or 300
cells. Virtual Playback page 1 contains numbers 1001–1300, page 2 contains 1301–1600,
and page 3 contains 1601–1900. Every later page advances by another 300 numbers. These
are normal show-owned Playbacks: two desks displaying Virtual Playback 1301 operate the
same assignment and runtime, even when their pane layouts differ.

Choose **Follow Main** to display the control desk's current main page, or **Pinned** to keep the pane on one fixed page. Changing the effective page changes only the addresses shown; it never operates a Playback. Right-click a cell, or press `[SET]` and then press the cell, to open the same Playback Configuration modal used by physical controls. A virtual cell is a one-button, faderless playback target with an additional icon or image-background choice.

Virtual actions have their own Preload capture switch. This allows physical controls to remain live while virtual actions are queued, or the reverse. Test the chosen capture combination before operation.

Use Virtual Playbacks for task-specific buttons, not as a substitute for assigning and documenting the underlying playback. The target, action, page, and release behavior must remain understandable from playback configuration.

## Playback Exclusion Zones

An exclusion zone is a named set of Virtual Playback numbers where at most one assigned
playback may be On. Hold Shift and select at least two displayed cells, choose **Create
Exclusion Zone**, and enter its name. ToskLight resolves those cells to their stable
playback numbers before saving. Shift-selection and zone creation are configuration
gestures: they do not press, start, or stop the selected Playbacks.

Saved zone members have an amber fence. Directly neighboring members share one outer fence with no internal fence edge. Disconnected members are outlined as separate islands, so the grid shows each connected part of the zone without relying on color alone.

When one member turns On, it wins and every other active member turns Off. Turning the winning member Off does not start another member. Touch, mouse, OSC, and restored playback state all use this server-owned rule. Virtual Playbacks do not use the physical F1–F8 Playback shortcuts. Automatic full-override release remains a separate playback option.

Open the pane's **Settings → Exclusion Zones** tab to rename or delete a zone. Choose
**Edit Zone** to close Settings and select its visible playback numbers on the live
grid. The window title then offers **Update Exclusion Zone** and **Cancel Edit**. A
Virtual Playback may belong to several zones; activating it releases the deduplicated
union of the other numbered members.

Zone configuration is stored once for the active show and is shared by every desk,
pane, OSC controller, and other control path. Moving, duplicating, resizing, or
removing a pane and removing a historical desk do not copy, retarget, or delete zones.
Only the explicit zone-delete action removes a zone. Desk layouts and current pages may
differ without changing the underlying playback numbers or zone membership.
