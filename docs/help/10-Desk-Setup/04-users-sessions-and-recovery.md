# Users, Sessions, and Recovery

Desk users separate operator programmers while sharing the same show and engine.

## Users and sessions

Open **Desk Setup > Users & sessions** to see enabled users and switch the current operator. Selection, command line, temporary programmer values, editing context, and undo/redo belong to that user and are shared across the user's connected devices. A different user has an independent programmer.

![Enabled users and current session](../assets/screenshots/workflows/desk-setup-users.png)

Use **Show > Change User** to switch or add an operator from the active show surface.

![Change or add the active operator](../assets/screenshots/workflows/show-change-user.png)

## Shows and recovery

**Shows & recovery** displays the active show, library count, server state, and autosave status. Show mutations autosave to the portable `.show` file. Named revisions are explicit restore points; they do not disable later autosaves.

![Desk show and recovery status](../assets/screenshots/workflows/desk-setup-shows-recovery.png)

The desk database stores users, show-library index, active-show choice, configuration, and durable session programmers. Portable show files are stored separately. Keep both when backing up an installation.

If startup reports an invalid show, preserve the affected file, load a known revision or other show, and inspect diagnostics before overwriting anything. See [Shows, Revisions, and MVR](../20-Show-Setup/02-shows-revisions-and-mvr.md).
