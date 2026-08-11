# Desk Setup

Desk Setup contains installation-specific configuration. It does not travel with a show file. Configure the desk before patching so operators, screens, control inputs, network binding, backups, and output behavior are predictable.

Open the Show menu and choose **Enter Setup**.

![Show menu with Enter Setup](../assets/screenshots/workflows/show-menu.png)

Work through these pages in order:

1. [Screens and Desktop Layouts](01-screens-and-layouts.md)
2. [OSC, Extensions, and Network Control](02-osc-midi-and-network.md)
3. [DMX Output and Universe Routes](03-dmx-output.md)
4. [Operators, Sessions, and Recovery](04-users-sessions-and-recovery.md)
5. [Media Server](05-media-server.md)

**Preferences** contains the focused **Defaults**, **Attributes & encoders**, **Highlight**, and **Others** pages. **Defaults** selects **Record & Update**, **Playback**, or **Pool colors** from the window title. Desk Setup saves each change immediately through the service that owns that setting; there is no separate Save changes step. Operator switching is available from **Show > Change User**, not from Desk Setup.

**Attributes & encoders** selects its section from the window title: **Encoder groups**, **Attribute
activation groups**, and **Attributes**.

**Encoder groups** edits the live encoder layout at the width configured with **Screens & playback → Configure encoder placement**. There is no separate read-only preview: this page is the layout. Every encoder
group has its own section, headed with how many encoders and pages it holds, and each page inside it
shows one slot per encoder. An empty slot is drawn dimmed and dashed; choose **Assign attribute** on
it to place an attribute that currently has no encoder.

Drag an assigned encoder — with a mouse or a finger — onto any slot to reorder it, move it to
another page, or move it into another encoder group. The layout follows the pointer: the encoders
behind the drag snap into the slots they would take, so the arrangement you see before you let go is
the one that is kept. Both affected groups are renumbered so no slot is left with a hole. Release
outside every slot to leave the layout as it was. The arrow buttons remain for keyboard use.

**Attribute activation groups** starts from a preset — **None**, **All**, **By Encoder Group**, or
**Intelligent** (the server-projected recommendation) — in its own section, alongside **Restore
recommended defaults**. A preset replaces the groups outright. Each activation group then gets its
own section, so you can rename it, delete it, remove a member, or add an unassigned attribute
without losing your place. An attribute belongs to at most one activation group, so adding it to a
group moves it out of its previous one, and a group disappears once its last member leaves.

**Attributes** gives every encoder group its own section listing the attributes this show can
program, each showing whether it is Built-in or Custom and where it sits.

**New custom attribute** creates a show-owned control for something the desk does not have yet — a
Media Group, say — with your own name and the encoder group it belongs to. It is placed on the first
free encoder slot of that group and can be dragged elsewhere from **Encoder groups**.

**Imported attribute names** is for the other case: a fixture file that spells an attribute the desk
already has differently. Enter the name as the GDTF file writes it and choose the attribute it
means — a GDTF `MediaRank` onto `media.folder`, `MediaPosition` onto `media.file` — instead of
adding a second control for the same thing. Imports record their own choices here too, and every
entry stays editable or can be forgotten. These mappings belong to the desk, not to the show file,
and a fixture revision that already resolved keeps its mapping.

Saving some server settings reports **Restart required**. Finish the configuration, restart once, then recheck the status and output diagnostics.
