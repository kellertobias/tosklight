# File Manager and Text Editor

Add simple text editing to the existing File Manager and provide a dedicated Text Editor window for persistent operator notes and other text files.

## File Manager editing

The File Manager should be able to open a supported plain-text file in a simple editor. From there, the operator can read, edit, save, and close the file without leaving the desk application. The first version does not need to be a source-code IDE, but it should preserve ordinary line breaks and text reliably, show whether there are unsaved changes, report save errors visibly, and ask before discarding edits.

Planning must define which file types and encodings are supported, whether new text files and folders can be created, maximum practical file size, read-only behavior, rename and delete interactions, and how an external modification or a second editor produces a conflict rather than silently overwriting newer content. File access must remain within the locations intentionally exposed by the File Manager.

## Dedicated Text Editor window

Add **Text Editor** as a pane/window type in the normal configurable window system. Each Text Editor instance can select one stored text file and keep that file associated with the window, making it suitable for show notes, cue notes, contact details, run sheets, or other free-form operator information.

The window title/header provides **Open File**, **Refresh**, **Save**, and **Save As**. Open File uses the root-confined ToskLight File Manager picker. The header also shows the file name/path and save state, with explicit handling for a missing, moved, deleted, or read-only file. Its selected file, cursor, and scroll position persist with the window configuration, but unsaved text must not be mistaken for safely stored show data.

Pane Settings selects read-only or read-write operation and one of three views: Plain Text, Rendered Markdown, or a two-column Edit + Markdown view with editable source on the left and rendered output on the right. A pane-level read-only choice prevents writing even when the underlying file is writable.

Multiple Text Editor windows may point to different files. If two windows open the same file, they must share updates or surface revision conflicts; they must not independently overwrite one another. Planning must also decide whether note files are normal files managed by the File Manager, assets embedded in the show, or both, and how those choices affect show portability, autosave, backup, Save As, and multi-user editing.

The dedicated window and the File Manager editor should use the same authoritative read/write and conflict-handling model so that a file saved from either surface is reflected correctly in the other.
