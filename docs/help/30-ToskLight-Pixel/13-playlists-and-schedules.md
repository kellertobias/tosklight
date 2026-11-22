# Playlists and schedules

Use **Playlists** to prepare ordered media and **Schedule** to run it automatically on a Pixel output.
The Pixel server runs the schedule even when its browser interface is closed.

## Prepare a playlist

Choose **New playlist**, enter a name and add clips from the addressed library folders.
Use **Move up** and **Move down** to arrange them. **Plays** is the total number of passes:
`1` plays the clip once and `3` plays it three times before advancing.
Still images need an **Image duration** for each pass.

Each clip has its own **Frame** controls. Set **scale X** and **scale Y** independently,
move it with **position X** and **position Y**, and set its **rotation (°)**.
Scale is a multiplier: `1` is the original size and `0.5` is half size.
Position `0` is centred. **Reset frame** restores that clip to scale `1`, position `0`
and rotation `0°`, without changing the other clips. The saved frame applies on every play,
including repeats and playlist loops.

Set **Fade to next** on each clip to choose the transition duration in seconds. Zero makes a cut.
The last clip's fade also applies when the playlist loops back to its first clip.
**Default fade** supplies the initial value for newly added clips.
Transparent media follows the ordinary layer source-over blend; its transition is not a linear
mix of two complete pictures. Check the result over any fixed background layers.

Choose two distinct **Playback layers**. Clips alternate between them so the next clip can load
before it becomes visible. Layers 2 and 3 are the default on an eight-layer output.
A two-layer output requires an explicit pair of layers 1 and 2.
The pair must exist on every output that uses the playlist.

In **Other layers**, choose **Leave unchanged**, **Off**, or **Fixed content** for each remaining layer.
Fixed images stay displayed; a fixed video can loop using its normal play mode.
These layers keep their settings while the foreground playlist advances.
Use the existing level, placement, mask, effect-bank and model controls to compose the output.
The playlist's Master dimmer and volume apply to the complete picture and sound.

Enable the playlist after adding its content. Pixel follows media identity when a file moves
or the library is compacted. A missing, parked or disabled file appears as unavailable;
it does not silently select the new occupant of its former address.
Pixel keeps each file's identity in its dotted library metadata. Keep these files when copying
the library. If an existing clip has no saved identity, Pixel writes one before publishing it.
An unwritable or damaged identity file produces an actionable startup error and preserves the original.

## Preview a step on the output

Choose the **Preview output**, then press **Preview step** on any clip.
Pixel takes over that output and shows the selected step with its saved frame, the playlist's
other layers, and its Master dimmer and volume. The selected clip loops for inspection;
preview does not advance to the following step or run a scheduled event.
The active clip is marked **Previewing**. Changing its frame updates the output while it plays.

Use **Stop preview** to restore the previous composition. Switching output or leaving Playlists
also ends preview. Pixel restores only a preview it still owns; newer desk or operator control
is never overwritten. If takeover was already enabled, it stays enabled after preview.
Otherwise stopping preview releases takeover so scheduling or incoming DMX can resume.

If the browser closes before its cleanup reaches Pixel, takeover can remain active.
Reopen Pixel and release **Take over playback** to return control.
Playlists saved before per-clip framing was added open with their original size and placement.

## Schedule an event

Open **Schedule**, choose an output and add an event referencing an enabled playlist.
A **Fixed** event uses a date and time. A **Weekly** event uses selected weekdays and a local start time.
All schedule times follow the Pixel server computer's local time zone, displayed on the page.
The browser's zone and the separate clock/text UTC offset do not change schedule times.

Leave the end unset to run the playlist once, including each clip's configured plays.
Add an end to loop the playlist until that instant, even if the final clip has not finished.
For a weekly overnight event, set the end time and the number of days after the start.
Optional first and last dates limit which weekly occurrences are generated.

The higher playlist **Priority** wins an overlap. Equal priorities prefer the earlier event start,
then a stable event identity. Pixel runs one scheduled playlist per output, including its fixed layers.
Switches between complete playlists are cuts; fades between their individual clips remain configurable.
When an eligible lower-priority window returns, its playlist restarts from its first clip.

Choose a **Fallback playlist** to loop whenever no scheduled event wins.
Enable **Scheduling** to start automatic playback. The current owner, playlist, clip and any loading
or failure reason appear beside the settings. The upcoming list shows events and overlap decisions.

## Desk and manual control

Valid incoming Art-Net or sACN for the selected output suspends automatic playback immediately.
**DMX silence timeout** controls how long Pixel waits after the last valid packet before resuming;
the default is 10 seconds. Repeated packets with unchanged values still count.
Packets for unrelated universes do not extend the wait.

After that silence, Pixel starts the currently eligible playlist or fallback from its first clip.
It does not replay start-only events missed during DMX control, manual takeover, downtime or a clock change.
A completed or interrupted single-pass event is not queued to run again later.

**Take over playback** and library output preview suspend the scheduler while the operator controls
the output. Release takeover to allow the schedule or a live desk to take control again.
Stopping scheduling restores captured settings only while the scheduler still owns them;
it never restores an old snapshot over newer desk or operator control.

## Edits, restart and failures

Edits save with visible pending, saved or error feedback. A running activation keeps its playlist
content until the next activation. Use **Apply now** to restart it with saved changes.
Disabling or deleting an event immediately re-evaluates which playlist wins.
Remove event and fallback references before deleting a playlist they use.

On restart, Pixel starts an active looping window or fallback from the beginning.
Past single-pass events remain missed. At a daylight-saving change, nonexistent local start times
are skipped and repeated local start times run once at their first occurrence.

Pixel skips unavailable clips with a visible reason and bounds loading attempts.
An activation with no playable clips fails instead of retrying continuously.
Pixel can then try the fallback; if that also fails, it clears the content it owns and retains the error.
Older supported configurations open with scheduling disabled and no authored playlists or events.
