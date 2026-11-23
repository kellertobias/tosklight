# Pixel playlist scheduling

## PIXEL-020: autonomous playlists, calendar events and desk handoff

Use an isolated Pixel configuration, addressed library and output. Keep all generated fixtures,
captures and reports under the canonical `.artifacts` paths. Do not change the operator's active library.
Executable browser coverage belongs to `tests/media-playlist-schedule.spec.ts`.

### Playlist preparation

Create a disabled draft playlist through the web interface. Add two videos from different folders
and one still image. Arrange them in a different order, set one video to three total plays,
give the image a positive duration and set both zero and nonzero outgoing fades.
Enable, reload and verify the saved order and values.

Verify **New playlist** and **New event** are fully visible within their title bars at desktop,
tablet and phone widths. Both must use the shared window-title action group, with flat corners
and unwrapped labels, matching the other Pixel windows. Create a playlist through the button while the initial list loads;
it must wait for that list, then select the actual new disabled draft without switching to an
existing playlist. A failed create must show an actionable error.

Give two entries different X/Y scale, X/Y position and rotation values. Reload and verify both
frames independently; **Reset frame** changes only the selected entry. A version-11 playlist
without framing fields must reopen with identity framing and the same clip identities.

Select layers 2 and 3 on an eight-layer output. Configure a fixed marketing image on another layer
and a looping video on a further layer. Verify their placement, mask, effect-bank and level settings.
They must remain stable through clip changes and repeated passes of the same clip.
Select layers 1 and 2 explicitly on a two-layer output; duplicate or unavailable layers must be refused.

Choose a preview output and press **Preview step** on each clip. Verify the actual selected output
shows that clip's frame and the playlist's fixed layers, without advancing. Edit its frame while
previewing; the clip and fixed auxiliary video must not restart just because its placement changes.
Switch rapidly between steps, then stop; only the last requested step may appear and the captured
composition must be restored. Switching output or leaving Playlists must end the preview.
Acquire a newer manual/DMX owner before the old preview stops; its stale restore must be refused.
Verify scheduling is suspended during preview and re-evaluates correctly after release.

Move or compact a referenced library asset, then reactivate the playlist. It must follow the same asset
at its new address. Disable or delete an asset and verify a visible skip reason with bounded attempts.
An all-unplayable playlist and an all-unplayable fallback must leave a persistent error without a retry loop.

### Calendar and priority

Create a fixed start-only event and verify one playlist pass. Create a fixed start/end event and
verify looping until the exact end. Repeat both forms for a selected weekday.
Verify an overnight weekly window, its day offset and optional first/last calendar dates.

Overlap two windows with different playlist priorities and verify that only the higher priority wins.
When it ends, the still-eligible lower priority must restart from its first clip.
Verify deterministic ties and an optional fallback between events.
Miss a start-only event while another playlist wins and verify that it is never played late.

Run the browser in a different time zone from the server computer. The schedule must follow the
computer's local zone shown in Pixel. Inject a forward or backward wall-clock change and a DST gap/fold:
there must be no catch-up burst or duplicate single pass.
Record the injected clock tests separately from native OS time-zone verification.

### DMX and operator ownership

Set the silence timeout to 10 seconds. Send valid matching Art-Net or sACN while a playlist runs.
The first packet must hand control to DMX. Continue sending unchanged packets and verify that automation
remains suspended. Stop input and verify automatic resumption after 10 seconds from the last packet.

Repeat with a different configured timeout, intermittent gaps and more than one sender.
Unrelated universes and short or invalid packets must not refresh the deadline.
The display must identify DMX ownership and show the remaining wait.
An event ending during suspension must not return, and a missed single-pass event must not run late.

Take over manually and use library preview. Release preview after a calendar boundary and after new
DMX arrives. No stale preview snapshot may overwrite the new owner.
Stop scheduling after an intervening manual/DMX acquisition and verify that restoration cannot overwrite it.

### Rendered transitions and lifecycle

Capture the actual compositor at transition start, midpoint and completion for both alternating
layer directions. Opaque clips must not dip in brightness from opposing source-over alpha ramps.
Inspect transparent clips over fixed content using the ordinary layer-compositing behavior.
Verify nonadjacent layer selection, zero-duration cuts, repeated identical clips and a delayed first frame.
Verify an off-centre scaled and rotated clip through both alternating layers, a default-framed next
clip, repeated plays and loop wrap. Framing must never leak between entries; fixed overlays stay put.

Close the browser while a window runs and verify that clip progression continues on the server.
Restart Pixel during an active window and verify restart from entry one.
Completed, suppressed and interrupted single-pass events must not be replayed after restart.

Open a supported older configuration through actual server startup. It must retain its library,
outputs, network and clock settings while initializing scheduling disabled.
An invalid configuration must retain the original bytes and show an actionable startup error.
Record unit, route, browser, GPU and packaged-app results separately from human acceptance.
