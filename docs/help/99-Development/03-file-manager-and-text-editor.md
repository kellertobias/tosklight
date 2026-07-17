# File Manager and Text Editor

The File Manager and Text Editor expose only registered roots. These include the built-in Shows root, operator-configured roots, and removable-drive roots discovered at runtime. They are operational tools for confined show-support files, not a general filesystem browser.

Use File Manager to inspect roots, create files and folders, and edit supported text files. ToskLight forms open its root-confined picker first. Desk configuration can expose **Open system file picker** as a secondary fallback, but the fallback is disabled by default and retains the calling form's extension, target, and cardinality constraints wherever the operating system supports them.

File Manager opens temporarily over the surface that launched it. Closing the built-in File Manager returns to the same Desktop or built-in window, including the same Setup section. Closing a File Manager picker dismisses only the picker and reveals its calling dialog or form. A File Manager pane remains part of its Desktop and therefore has no close button in its pane header.

The header's **Edit**, **New**, and **View** controls are dropdown menus. Edit operations use distinct icons and colors; New distinguishes files from folders. In View, List and Grid are a single-choice group, while **Show Hidden Files** and **Show Properties Sidebar** are fixed-label checkboxes. The compact location control shows the current `/path` without repeating the root name and opens a menu of its parent locations.

Add a Text Editor pane to keep one file associated with a Desktop pane, monitor its Saved/Unsaved state, and save deliberate edits. The supported text formats are `.txt`, `.md`, `.csv`, and `.log`; their contents must be UTF-8 and no larger than 4 MiB. Ordinary line breaks are preserved. Pane Settings can make a pane read-only independently of the underlying file and selects Plain Text, Rendered Markdown, or a two-column Edit + Markdown view. Read-only files can be viewed and copied with **Save As**, but cannot be overwritten; a pane configured read-only disables both Save and Save As.

The server validates paths inside each root. Relative traversal and files outside the configured boundary are rejected.

## Roots, removal, and previews

Configured roots have stable IDs, labels, absolute server paths, and optional icons. Removable drives are discovered through platform-specific macOS, Linux, and Windows adapters and are never persisted into that configured list. A disconnected drive is removed from the next roots snapshot. The UI clears stale navigation and selection state, and an operation that loses a root surfaces the server error.

The properties column reads supported native file notes without creating sidecar files. It previews common raster images and streams MP3 and WAV files through signed, short-lived content tickets with HTTP range support so seeking does not require transferring the complete file. Delete uses platform Trash only when the filesystem advertises that capability; otherwise the operator sees an explicit permanent-delete warning.

## Storage and saving

Text notes are normal files in a configured File Manager root. They are not embedded automatically in the show file. A note is portable with a show only when the operator stores or copies it into the portable show location. Text Editor does not autosave and never stores an unsaved draft in the Desktop layout: **Save** writes the associated file, while **Save As** creates another normal file and associates that Text Editor pane with it. Cursor position and scroll position may persist as non-authoritative pane view state.

Every successful read returns a content revision. Saving uses that revision as a compare-and-swap check and replaces the file atomically only when the stored revision still matches. A second editor or external writer therefore produces a visible conflict instead of silently overwriting newer text. Clean editors adopt a newer saved revision; editors with local changes retain their draft and offer comparison, reload, or **Save As**. Closing a file, removing its pane, switching files, or leaving the application asks before discarding unsaved changes.

The application does not create a separate hidden backup for every edit. Normal filesystem, show-location, and deployment backup policies remain authoritative.

## Rename, move, and delete

Renames and moves completed through a connected File Manager are announced to all connected desks. An open Text Editor follows the new root-relative path, persists the new pane association, and retains any unsaved draft. Moving or renaming a parent folder also updates open descendants. The File Manager's embedded editor follows the same rule.

Deleting a file or moving it to platform Trash leaves the last loaded text visible and marks the editor **Missing**; it is not treated as a successful save. The operator can recreate the old path, save the retained text under another name, choose a different file, or close it. If a file is moved directly by another operating-system program rather than through ToskLight, the old association is reported as missing; refresh the file list and choose the moved file explicitly.

All editor surfaces use the same server revisions and file-operation notifications. They share saved state, not an unsaved in-memory draft: simultaneous unsaved edits remain independent until one is saved, at which point the other editor receives a revision conflict.

## Verification

The executable checks for this behavior are split by boundary:

- `cargo test -p light-server file_manager` covers path confinement, operations, conflict choices, trash behavior, range streaming, notes, thumbnails, removable-root snapshots, and compatibility defaults.
- `cargo test -p light-server discovery_adapters_cover_macos_linux_and_windows_mount_layouts` covers representative removable-drive discovery input for all three supported desktop platforms.
- The focused Control UI tests cover picker constraints, system-fallback gating, form integration, Text Editor modes and read-only behavior, multi-window routing, disconnect recovery, and UI state.
- `./test e2e tests/15-text-editor.spec.ts` and `./test e2e tests/16-file-manager.spec.ts` exercise the real browser/server boundary, including same-desk OSC routing.

An actual removable-drive cycle remains a deliberate packaged-desktop check because a synthetic mount listing cannot prove that the operating system and packaged application observe real hardware correctly:

1. Run `./build open` and verify `http://127.0.0.1:5000/api/v1/readiness`.
2. Open File Manager and verify the Shows and configured roots.
3. Attach a removable drive and verify that it appears as a runtime root while Desk configuration remains unchanged.
4. Browse and read a file, begin an operation involving that drive, then detach it.
5. Verify a visible error or safe fallback, that the root disappears, and that no stale operation remains claimed.
6. Reattach the drive and verify that it is rediscovered cleanly.
