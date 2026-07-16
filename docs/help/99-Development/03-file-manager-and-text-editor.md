# File Manager and Text Editor

The File Manager and Text Editor expose only server-configured roots. They are operational tools for confined show-support files, not a general filesystem browser.

Use File Manager to inspect roots, create files and folders, and edit supported text files. Add a Text Editor pane to keep one file associated with a desk window, monitor its Saved/Unsaved state, and save deliberate edits. The supported text formats are `.txt`, `.md`, `.csv`, and `.log`; their contents must be UTF-8 and no larger than 4 MiB. Ordinary line breaks are preserved. Read-only files can be viewed and copied with **Save As**, but cannot be overwritten.

The server validates paths inside each root. Relative traversal and files outside the configured boundary are rejected.

## Storage and saving

Text notes are normal files in a configured File Manager root. They are not embedded automatically in the show file. A note is portable with a show only when the operator stores or copies it into the portable show location. Text Editor does not autosave and never stores an unsaved draft in the desk layout: **Save** writes the associated file, while **Save As** creates another normal file and associates that Text Editor pane with it. Cursor position and scroll position may persist as non-authoritative pane view state.

Every successful read returns a content revision. Saving uses that revision as a compare-and-swap check and replaces the file atomically only when the stored revision still matches. A second editor or external writer therefore produces a visible conflict instead of silently overwriting newer text. Clean editors adopt a newer saved revision; editors with local changes retain their draft and offer comparison, reload, or **Save As**. Closing a file, removing its pane, switching files, or leaving the application asks before discarding unsaved changes.

The application does not create a separate hidden backup for every edit. Normal filesystem, show-location, and deployment backup policies remain authoritative.

## Rename, move, and delete

Renames and moves completed through a connected File Manager are announced to all connected desks. An open Text Editor follows the new root-relative path, persists the new pane association, and retains any unsaved draft. Moving or renaming a parent folder also updates open descendants. The File Manager's embedded editor follows the same rule.

Deleting a file or moving it to platform Trash leaves the last loaded text visible and marks the editor **Missing**; it is not treated as a successful save. The operator can recreate the old path, save the retained text under another name, choose a different file, or close it. If a file is moved directly by another operating-system program rather than through ToskLight, the old association is reported as missing; refresh the file list and choose the moved file explicitly.

All editor surfaces use the same server revisions and file-operation notifications. They share saved state, not an unsaved in-memory draft: simultaneous unsaved edits remain independent until one is saved, at which point the other editor receives a revision conflict.
