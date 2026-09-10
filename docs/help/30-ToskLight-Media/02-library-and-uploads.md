# Build the Media Library

The Media Library is addressed deliberately so the Desk can recall the same content without depending on filenames. Ordinary media uses folders `1`–`199`, text uses `200`–`249`, and generated visualizers use `250`–`255`. Within an addressed folder, file `0` and file `255` are blank; usable content occupies the remaining slots.

To keep the library on another disk or shared storage, open **Settings > Libraries**, enter the folder on the Media Server computer under **Media library directory**, and save it. The new directory is used after Pixel restarts. Pixel does not move the existing library, so copy the addressed folders, media, thumbnails, and dotted metadata files yourself before restarting when the content must follow the setting. **Revert to current directory** cancels a pending directory change before restart.

The **Effects** tab owns the 255 numbered presets used by playback. Slot **0** is always **Off**;
slots **1–255** may be assigned, named, cleared, and configured. Editing a preset updates every
layer bank that selects that number—effect settings are not copied into individual layers or cues.

Select an ordinary media folder to give it an operator-facing name. The name is saved when you leave the field or select another folder and remains attached to that folder after Pixel restarts. Empty folders use a subdued neutral card treatment so populated folders remain more prominent; the selected empty folder still keeps a clear selection outline.

## Upload a slot

Choose the exact folder and file position before uploading. The original source is accepted as a multipart upload and queued for conversion. Video is converted to the Media Server's playable HAP Alpha `.toskclip` form with FFmpeg. The job reports queued and running progress, can be cancelled, and retains a visible reason when it fails.

An upload is not playable merely because the source transfer finished. Wait for the conversion job to complete and verify the resulting thumbnail or preview. If FFmpeg is unavailable or the source is unsupported, correct that failure rather than programming a slot that has no playable asset.

After conversion, Pixel samples a bounded set of frames across the finished playable clip and chooses the most useful thumbnail by preferring visible content, detail, and colour. A later representative frame therefore wins over an all-black or nearly black opening when one is available. Still images and very short clips use the same safe selection path.

Select a video in the Library to see a moving management preview in its inspector. Pixel decodes a small sequence of JPEG pictures from its native playable clip; it does not start a layer, change the output, or expose the source file to the browser. A still image continues to show its thumbnail. If a frame cannot be decoded, Pixel leaves the existing thumbnail visible.

To reconsider an existing file, select it and choose **Retry thumbnail**. Pixel reruns automatic selection from the stored playable clip, so the original upload does not need to remain available. A failed retry keeps the current thumbnail. Choose **Upload custom thumbnail** to supply one PNG, JPEG, GIF, or WebP image up to 16 MiB; Pixel validates it and normalizes it to the library JPEG size. Both operations change only the thumbnail. The result stays attached to the media file through moves, swaps, parking, and compaction.

Replacing an occupied slot keeps the old playable clip available until the replacement has been accepted and converted. A failed replacement therefore does not silently destroy the content currently used by the show.

> [!danger] Missing graphic
> Add a Media Library screenshot showing addressed folder and file slots together with queued, running, completed, and failed conversion states.

## Import files already on the server

**Import all** scans supported files copied into the configured library and creates the required conversion jobs. Address-leading folder and file names keep library allocation deterministic. Duplicate address claims use the first supported relative name in deterministic order and appear in diagnostics instead of changing on each scan.

Use library move, swap, and parking operations when reorganizing addressed content. Rehearse the result with the real Desk cues after changing addresses: the Desk stores numeric folder/file values, not the source filename that used to occupy them.

To close gaps inside one folder, select its folder card and choose **Compact files** in the inspector. Pixel preserves the files' existing order and moves them into consecutive slots beginning at file `1`. Only that folder changes, and each file keeps its name, thumbnail, note, BPM correction, enabled state, and preserved source. Because compaction changes numeric file addresses, rehearse any Desk cues that refer to the folder afterwards. Empty and already compact folders are safe no-ops.

## Notes and licence information

Select one media file to add a free-form note in its Library inspector. To select a continuous range within the current folder, select the first file and then Shift-click the last file; only occupied file slots in that numeric range are selected, so gaps are harmless. Command-click on macOS or Control-click on Windows toggles one file and makes it the start of another range. Shift-click another file to add that new range without losing the earlier selection. Folder cards use the same Command/Control-click selection, so one note can cover one folder or several folders at once. When the selection contains different notes, the editor says **Multiple values** and leaves the field empty until you deliberately replace or clear them.

Notes are intended for licence terms, attribution, purchase references, source URLs, and production reminders. A file note belongs to the stable media file, not its numbered slot: it follows that file through moves, swaps, parking, and compaction, and remains linked after Pixel restarts and rediscovers the library. **Clear note** removes the note from every selected file or folder without changing names, BPM corrections, pictures, addresses, or media content.

## Disable or delete a media file

Select one media file to find its **Enabled** control. Turn **Enabled** off when the file must remain in the library but must not play. The address, media bytes, thumbnail, name, BPM correction, and note remain intact. Any layer selecting the disabled file becomes transparent, exactly as though that layer selected an empty address; an already-running video or audio session is stopped. Turn **Enabled** on again to restore normal playback from the same address.

With several files selected, use **Enable selected** or **Disable selected** to apply the same playback state to the whole selection in one operation. The selected files keep their individual addresses, content, names, BPM corrections, and notes.

Use **Delete media** only when the file must be removed permanently. Pixel asks for confirmation before deleting the playable media, preserved source, thumbnail, and per-address metadata. The address becomes free, and a layer that was using the deleted file becomes transparent without retaining its last frame or audio. Cancelling the confirmation changes nothing.

For a multiple-file selection, **Delete selected** asks once and then permanently removes exactly those selected files and their companion artifacts in one operation. Cancelling that one confirmation changes nothing.

## Convert several local files on macOS or Windows

Choose **Convert multiple files** from the Pixel menu-bar item on macOS or notification-area item on Windows. On macOS, one native picker accepts any mixture of multiple videos, images, and folders. On Windows, Pixel offers an optional multi-file picker followed by an optional multi-folder picker; cancelling either Windows picker simply adds nothing from that step. Nested folders are scanned recursively. Audio and unsupported files are ignored. Pixel then asks for the first library folder and file. Sources keep their stable selection/traversal order and occupy consecutive addresses, continuing at file `1` of the next folder after file `254`. The whole selection is refused before conversion when it would run past folder `199`.

The Library screen opens automatically and shows overall and per-file progress. A failed conversion is retried once at the same address. If the retry also fails, Pixel skips that file and retains its address, filename, and final reason in the conversion report. Existing or already-reserved addresses are never overwritten; they appear as failed entries in that report while later files continue at their assigned addresses.
