# Build the Media Library

The desk recalls content by numeric folder/file address, not filename. Ordinary media uses folders
**1–199**, text **200–249**, and visualizers **250–255**. File slots **1–254** hold content;
**0** and **255** are blank.

![Numbered media folders, file slots and the selected file inspector](../assets/screenshots/workflows/pixel-library.png)

## Upload or replace content

1. Select the required folder and file slot.
2. Upload the source and wait for conversion to finish. The job shows its progress and failure
   reason; cancel a running job if necessary.
3. Check the thumbnail and management preview before programming that address.

![Conversion jobs show progress and keep completed, running and queued files visible](../assets/screenshots/workflows/pixel-library-import.png)

Video conversion requires FFmpeg. Finishing the upload alone does not make a slot playable.
A replacement keeps the old playable clip until the new conversion succeeds.

Select a folder to name it. Select a file to preview it without changing the output. **Retry
thumbnail** chooses a new picture from the converted clip; **Upload custom thumbnail** accepts
PNG, JPEG, GIF or WebP up to 16 MiB. Neither changes the media itself.

## Preview on the physical output

Turn on **Enable preview** below **Take over playback** in the dock. A plain click on a Library
item plays it alone on Layer 1 at full level with Master raised; other layers are out. This takes
over the selected output, so use it deliberately during setup.

Preview also works on Visualizers, Text, Effects and Models. It follows the selected item and
edits. Turning preview off, or opening another page, restores the earlier layers and Master and
returns ownership to the desk unless playback was already taken over. Preview is temporary.

## Import and organize

**Import all** scans supported files already copied to the configured library and queues conversion.
Address-leading folder/file names control allocation. Check reported duplicate address claims.

Use move, swap and parking operations to reorganize. **Compact files** on a folder closes gaps
while preserving file order and metadata. These operations change numeric addresses: rehearse any
desk Cues using them afterward.

To move the library to another disk, use **Settings > Libraries > Media library directory**, copy
its contents and metadata yourself, then restart. **Revert to current directory** cancels a pending
change. Changing the setting does not move files.

## Notes, disabling and deletion

Select a file or folder to add licence terms, attribution, source links or production notes.
Command/Ctrl-click toggles items; Shift-click selects occupied files across a range. Mixed notes
show **Multiple values** until deliberately replaced or cleared. File notes follow the media
through moves and compaction.

**Enabled** off retains the file but prevents playback. Layers selecting it become transparent
and stop its audio. Turn it back on to restore the same address. Multi-selection provides
**Enable selected** and **Disable selected**.

**Delete media / Delete selected** permanently removes selected files and their stored source,
thumbnail and metadata after confirmation. Their slots become free; layers using them become
transparent. Cancel leaves them unchanged.

## Convert several local files

On macOS or Windows, choose **Convert multiple files** from Pixel's status item. Select videos,
images and folders, then the first library folder/file address. Nested folders are scanned;
audio-only and unsupported files are ignored. Windows offers separate optional file and folder
pickers; cancelling one adds nothing from that picker.

Files occupy consecutive addresses through file 254, then continue at file 1 of the next folder.
A selection that exceeds folder 199 is refused. Existing/reserved slots are not overwritten.
The Library shows overall and per-file progress. A failed job is retried once, then remains in the
conversion report with its assigned address and reason.
