# File Manager

Add a reusable three-column File Manager that can be placed on any desk and opened as a full utility workspace from **Enter Setup → File Manager**. It must not appear in the permanent Built-ins dock. The same component must also support a picker mode for workflows that need the operator to choose files or folders.

## Window layout

The File Manager has one toolbar row and three columns:

- The narrow left column is a lazily loaded folder tree. Configured roots and connected removable drives are its top-level items. Ordinary folders use a folder icon; the Shows root, configured roots, removable drives, and other special roots use distinct icons.
- The flexible middle column shows the current directory. It supports a folders-first list/table view and a folders-first thumbnail grid. The list includes name, type, size, and modified time. The grid uses image thumbnails where available and suitable folder or file-type icons otherwise.
- The narrow right column shows properties for the current selection: name, type, size, created time when available, modified time, a note editor when supported by the filesystem, a raster-image preview, or an audio player for MP3 and WAV files.

The window kit's existing responsive behavior may collapse the side columns behind Navigation and Info controls when a desk pane is too narrow, but the normal full-window layout shows all three columns.

The toolbar contains Back and Forward navigation on the left, a breadcrumb path, and view/action controls on the right. The path is a clickable breadcrumb rather than an editable absolute path. The controls include list/grid mode, hidden-file visibility, Rename, Copy, Move, and Delete.

Clicking or tapping an item selects it and updates Properties. Double-clicking a folder opens it. The folder tree and breadcrumbs remain available while files are selected. Ctrl/Command toggles items and Shift selects a range; while a file operation is armed, ordinary taps toggle source items so multi-selection also works on a touchscreen.

## Roots and path safety

Desk configuration gains a backward-compatible list of File Manager roots. Each configured root has a stable ID, operator-facing label, server path, and optional icon. A desk without this configuration receives the desk's Shows directory as its default root.

Mounted removable drives are detected automatically on macOS, Linux, and Windows and appear as runtime roots without being written into the configured-root list. A disconnected drive disappears cleanly and an operation involving it reports a visible error.

The server exposes only root-relative paths. Every source, destination, and parent is canonicalized and confined to a registered root. Path traversal and symlink escapes are rejected. The UI never exposes a whole-machine filesystem root or an unrestricted absolute server path.

Hidden-file behavior follows dotfile conventions on Unix and filesystem hidden attributes on Windows. Creation time is optional because not every filesystem provides it.

## Selection and picker mode

Management mode allows browsing and file operations without requiring a result callback.

Picker mode is configurable by its caller with:

- files, folders, or either as valid targets;
- single or multiple selection;
- an optional list of allowed file extensions;
- an optional initial root and directory; and
- explicit Select and Cancel callbacks.

Picker mode shows Select and Cancel controls. ENTER chooses the current valid selection; ESC cancels. Merely highlighting an item must not close the picker.

This picker is the standard file and folder field for ToskLight UI forms. A form must open the root-confined ToskLight File Manager picker first, rather than opening the operating system picker directly. Desk settings may enable an explicit **Open system file picker** fallback; only when that setting is enabled may the ToskLight picker offer a secondary action that opens the native operating-system picker. The fallback is never the default action and must preserve the same allowed file types and selection constraints where the operating system supports them.

## Desk-key targeting

SET, CPY, MOV, and DEL use the desk's normal button-selection model. Pressing one of these keys creates a pending, unowned action. A File Manager must not capture the action merely because it is visible, was used most recently, or currently has keyboard focus.

The next pointer interaction determines the target:

- If the operator clicks anywhere inside a File Manager instance, that instance claims the pending action.
- If the operator clicks outside the File Manager, existing lighting behavior continues unchanged.
- If several File Manager panes are visible, only the instance actually clicked may claim the action.

After a File Manager claims an action, it owns that operation until ENTER completes it, ESC cancels it, the window is removed, or the connection is lost. Only during this claimed operation may the server route subsequent OSC ENTER and ESC actions to the File Manager instead of the lighting command line. Touch keypad, physical keyboard shortcuts, and OSC must produce identical behavior.

The key workflows are:

- `[CPY]` → click source item or items → navigate to the destination or remain in the current folder → `[ENTER]`.
- `[MOV]` → click source item or items → navigate to the destination or remain in the current folder → `[ENTER]`.
- `[DEL]` → click source item or items → `[ENTER]` opens confirmation → `[ENTER]` confirms. `[ESC]` cancels.
- `[SET]` → click one item → edit its name → `[ENTER]` commits. `[ESC]` cancels.

Toolbar actions use the same operation state. Rename and Delete require an existing selection. Toolbar Copy and Move begin with the current selection. Once Copy or Move starts, Rename, Copy, Move, and Delete disappear and are replaced by **Copy Here** or **Move Here** plus **Cancel**.

## Copy, move, rename, and delete behavior

When copy, move, or rename encounters an existing name, show Replace, Keep Both, and Skip. Multi-item operations also offer Apply to All.

Cross-root moves copy the complete source first and delete the source only after the copy succeeds. Partial failures remain visible and must not silently discard source data.

Delete always requires confirmation. Use the platform trash where the target filesystem supports it. If trash is unavailable, the confirmation must explicitly say that deletion is permanent. A failed trash operation must report an error rather than silently retrying as a permanent deletion.

## Notes and previews

Store notes only in native filesystem metadata: a namespaced extended attribute on macOS and Linux, or an equivalent native metadata stream on Windows when supported. If a filesystem does not support the required metadata, show Notes as unavailable and do not create hidden sidecar files or desk-database records.

The first version previews common raster images such as JPEG, PNG, GIF, and WebP. It streams MP3 and WAV files with range support to the audio player. Video, PDF, document, and source-code previews are outside this feature. Plain-text editing is implemented separately in [File Manager and Text Editor](15-text-editor.DONE.md) and reuses the same confined file API.

## Server interface

Add authenticated server operations for:

- listing configured and removable roots;
- listing directories and reading metadata and filesystem capabilities;
- streaming file content with HTTP range support;
- producing cached raster-image thumbnails;
- copying, moving, renaming, trashing or deleting files and folders;
- reading and updating supported native notes; and
- claiming and releasing a transient file-operation input context for the active desk session.

The input context is created only after a pointer interaction inside the File Manager claims a pending action. It is never created from focus, visibility, or the original keypress.

## Verification contract

Automated coverage must include configured/default roots, removable-drive discovery adapters, root confinement, traversal and symlink escapes, hidden files, optional timestamps, range streaming, thumbnails, note capability, copy and move across roots, conflict choices, rename validation, trash capability, and drive disconnection.

UI coverage must verify the three-column layout, folders-first ordering, lazy tree navigation, breadcrumbs, history, view and hidden toggles, properties, previews, multi-selection, toolbar replacement, conflicts, and every picker configuration.

Picker coverage must also prove that form file fields open the ToskLight picker by default, that the operating-system picker action is absent while disabled in Desk settings, and that enabling the fallback exposes it without bypassing the calling form's target and extension constraints.

Input-routing coverage must prove that focus or visibility alone never captures SET, CPY, MOV, or DEL; clicking inside claims the pending action only for the clicked File Manager; clicking elsewhere preserves lighting behavior; and touch, physical shortcuts, and OSC have matching ENTER and ESC behavior. The lighting command line must not execute or change while a claimed file operation is active.

Final desktop verification uses `./build open` with the configured Shows root and an actual removable-drive attach/detach cycle. Existing desk layouts and configurations without File Manager data must continue to load unchanged.
