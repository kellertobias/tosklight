# Build the Media Library

The Media Library is addressed deliberately so the Desk can recall the same content without depending on filenames. Ordinary media uses folders `1`–`199`, text uses `200`–`249`, and generated visualizers use `250`–`255`. Within an addressed folder, file `0` and file `255` are blank; usable content occupies the remaining slots.

## Upload a slot

Choose the exact folder and file position before uploading. The original source is accepted as a multipart upload and queued for conversion. Video is converted to the Media Server's playable HAP Alpha `.toskclip` form with FFmpeg. The job reports queued and running progress, can be cancelled, and retains a visible reason when it fails.

An upload is not playable merely because the source transfer finished. Wait for the conversion job to complete and verify the resulting thumbnail or preview. If FFmpeg is unavailable or the source is unsupported, correct that failure rather than programming a slot that has no playable asset.

Replacing an occupied slot keeps the old playable clip available until the replacement has been accepted and converted. A failed replacement therefore does not silently destroy the content currently used by the show.

## Import files already on the server

**Import all** scans supported files copied into the configured library and creates the required conversion jobs. Address-leading folder and file names keep library allocation deterministic. Duplicate address claims use the first supported relative name in deterministic order and appear in diagnostics instead of changing on each scan.

Use library move, swap, and parking operations when reorganizing addressed content. Rehearse the result with the real Desk cues after changing addresses: the Desk stores numeric folder/file values, not the source filename that used to occupy them.
