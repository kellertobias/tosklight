# Desk Setup

Desk Setup contains installation-specific configuration. It does not travel with a show file. Configure the desk before patching so operators, screens, control inputs, network binding, backups, and output behavior are predictable.

Open the Show menu and choose **Enter Setup**.

![Show menu with Enter Setup](../assets/screenshots/workflows/show-menu.png)

Work through these pages in order:

1. [Screens and Desktop Layouts](01-screens-and-layouts.md)
2. [OSC, MIDI, and Network Control](02-osc-midi-and-network.md)
3. [DMX Output and Universe Routes](03-dmx-output.md)
4. [Operators, Sessions, and Recovery](04-users-sessions-and-recovery.md)

**Preferences** contains the focused **Defaults**, **Attributes & encoders**, **Highlight**, and **Others** pages. Operator switching is available from **Show > Change User**, not from Desk Setup.

**Attributes & encoders** selects its section from the window title: **Encoder groups**, **Attribute
activation groups**, and **Attributes**.

**Encoder groups** edits the live encoder layout at the width configured in Screens & playback →
Encoder placement, not a fixed preview. Every encoder group has its own section, and each page shows
one slot per encoder. An empty slot is drawn dimmed and dashed; choose **Assign attribute** on it to
place an attribute that currently has no encoder. Drag an assigned encoder onto any slot to reorder
it, move it to another page, or move it into another encoder group — both affected groups are
renumbered so no slot is left with a hole. The arrow buttons remain for keyboard and touch use.

**Attribute activation groups** starts from a preset — **None**, **All**, **By Encoder Group**, or
**Intelligent** (the server-projected recommendation). A preset replaces the groups outright; every
group stays editable afterwards, so you can rename it, delete it, remove a member, or add an
unassigned attribute. An attribute belongs to at most one activation group, so adding it to a group
moves it out of its previous one, and a group disappears once its last member leaves.

**Attributes** lists every attribute this show can program, grouped by encoder group, each showing
whether it is Built-in or Custom and where it sits. Custom attributes are still created and edited
from that same page.

Saving some server settings reports **Restart required**. Finish the configuration, restart once, then recheck the status and output diagnostics.
