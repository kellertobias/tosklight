# Solo Regions

A Solo Region is a show-owned named set of explicit Playback addresses. A permanent start releases other permanent members with the Solo handover fades. Regions can cross pages and overlap; shared targets and overlapping peers are deduplicated.

Membership applies to the exact Playback address started. Turning the winner Off starts no replacement. A Timecode's direct Cue starts do not trigger Solo. When a Timecode is assigned to a Solo member Playback, a permanent start of another member stops that Timecode.

## Flash and permanent playback

Flash bypasses Solo. Flashing a member leaves permanent peers running, and a permanent start of a peer preserves existing Flash holds. Releasing Flash removes its temporary contribution. It neither restarts a released permanent Playback nor stops the current winner.

## Create a region

Choose **Create Solo Region** in a Playbacks pane's title. Select cells, enter the name, then **Save** or **Cancel**. Change pages during the draft to include addresses from other pages. Selecting cells configures the region without pressing or operating their Playbacks.

The lower section's **PROG/PLAYBK** button also opens settings when held. On an optional screen, hold **Playback/Encoders** instead. Open **Manage Solo Regions**, then **Create New Solo Region**. The command line becomes the draft's name, Save, and Cancel controls. An optional screen shows that editing line only while its Solo draft is open.

## Edit saved regions

Open pane Settings and choose **Manage Solo Regions**. Select a region to rename it, choose its **Region color**, **Delete**, or **Edit**. Edit returns to the grid to add or remove members before Save or Cancel.

Each region keeps its own outline color. Regions without a chosen color keep the amber default. Adjacent members share one outer fence; disconnected members form separate islands, and overlapping regions retain their own outlines.

Membership survives page and layout changes, closing or removing panes, and hardware disconnection. Only explicit membership edits or region deletion change it.

[Playback surfaces](14-playback-surfaces.md) explains the displayed controls. [Preload](12-preload.md) queues eligible permanent actions separately from live Flash holds.
