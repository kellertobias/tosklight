# Media and Running Panes

These scenarios are the operator acceptance contract for the capability-gated Media operating surface and the authoritative Running overview.

## MEDIA-005 — two effect banks and shared presets

Given a Media Server is patched and its Effects library assigns two distinct presets, when an
operator selects them in Bank 1 and Bank 2 and changes each Effect Strength, then both the Media
Server and ToskLight Control show exactly those two ordered banks with the literal controls
**Effect Select**, **Effect Strength**, and **Parameter 1** through **Parameter 4**. Slot 0 reads
Off, missing slots remain visibly unassigned, and the isolated layer preview and the final output
both visibly show the selected effects in bank order.

When the operator raises one bank parameter above 0, the output changes the matching parameter of the
selected effect, in the order the Effects library lists it; returning it to 0 restores the preset's
stored value. A parameter beyond the selected effect's own parameters has no visible effect. The
2-layer and 8-layer personalities occupy 158 and 512 slots, and the Media Server's Connect to Console
downloads patch the same 59-slot layer and 40-slot master.

Given a Media Server configuration stored with a retired channel layout (legacy, mask positioning,
full master, or effect banks), when the Media Server starts, then the output keeps its 2-layer or
8-layer personality on the 3D-object-mapping layout, and Settings offers only those two
personalities. When an operator selects a Blur preset in a bank, the output visibly blurs; no
channel is labelled as a legacy or ignored Blur.

When the operator edits either preset in the Effects library, every bank selecting that slot uses
the new settings without changing its Select or Strength value.

Given loaded layers have dimmer above zero, when Master **Layer Opacity Cycle** is enabled at a beat
multiplier or divider, then the effective opacity advances in stable layer order on that timing.
Layers without loaded media or with dimmer zero are skipped, and their authoritative dimmer values
remain unchanged.

## MEDIA-001 — eligibility and unavailable state

Given a show without a patched physical media-server master or configured CITP/MSEX connection, **Media** remains present through **Shift + Stage** in Built-ins and through **Open Window**. Opening it states that no CITP Media Server is available while retaining all 0–255 folder/file choices and the Content/Mask configuration surface. When an eligible fixture exists, advertised names and live data reconcile into those same stable choices; unadvertised values remain configurable. A saved Media pane remains in the Desktop when its server disconnects, its patch disappears, or a required capability becomes unavailable; it explains that state and does not silently select another server or layer.

## MEDIA-002 — advertised preview and selection identities

Given an eligible physical master whose advertised composite source, logical layer, fixture head, and CITP source IDs differ, the Media window shows the advertised Program output and each advertised layer status without substituting any of those IDs. Loading, stale, failed, and unsupported feedback remains outside the actual preview. Touching one layer replaces the authoritative desk selection with that exact logical head, and an external selection change is reflected without a Media-only selection split.

## MEDIA-003 — atomic touch browsing

Given live Folder A / File X, touching Folder B changes only the draft browser and fetches B's files and thumbnails. Programmer values, DMX, and Undo history remain unchanged. Touching File Y commits Folder B / File Y through one grouped Programmer mutation and one Undo step, with no observable Folder B / File X state. Cancelling, switching server or layer, disconnecting, losing the library revision, or receiving a rejected write leaves the live pair unchanged.

## MEDIA-004 — capability-derived controls and persistence

Folder/File and Mask Folder/Mask File encoders remain immediate. The touch browser presents a discoverable **Media / Mask** choice only when masks are advertised. Secondary controls follow fixture and connection capabilities, and a native ToskLight Pixel action appears only behind its advertised capability. Restarting the desktop, changing show, disconnecting, and reconnecting preserve the pane's stable server, layer, browser, section, and secondary-region configuration without changing portable show data.

## MEDIA-006 — current Media Server configuration in Show Patch

Given three discovered Media Servers — one whose output reports the current 8-layer personality, one that still reports a retired channel layout or lacks the current output fields, and one that answers CITP discovery but not its configuration API — **Show Patch > Media Servers** shows each output with its suggested DMX `universe.address`, its **2 layers** or **8 layers** mode, its protocol, and its tempo source, never the raw Media Server identifiers. The configured output is **Not patched** until **Patch suggested** creates an 8-layer ToskLight Media Server fixture at that address. The outdated output reads **Needs update**, explains that ToskLight Media must be updated, and keeps both patch actions disabled. The unreachable server reads **Unavailable** and asks the operator to check that it is running and reachable on port 8080. A desk fixture patched with the other personality reads **Mode differs**, and **Patch suggested** switches it to the output's mode. A Media Server refusal of an address change names the refused value or the save failure and restores the desk patch. The desk's discovery reader and the Media Server's configuration route assert the same reference output document.

## MEDIA-007 — patched Media Servers as a live configuration table

Given a show with a CITP media server patched at an address where nothing answers and a ToskLight Media Server patched with network control off, **Show Patch > Media Servers** lists both in the **Patched Media Servers** table with the columns **#**, **Name**, **Type**, **Protocol**, **IP address**, **Port**, **Status**, and **Actions**; their types read **CITP media server** and **ToskLight Media**. **Refresh Discovery** is the only button in its own window-title group, left of the Fixtures / Media Servers / Tracking tabs, and repeats discovery; a discovered server at a row's address is reported in that row's status. The CITP row's desk connection attempt turns it from **Checking…** to **Offline** without a manual refresh, and the row shows the server's reason with what to check. **Refresh Thumbnails** on that row reports its failure there only, and the other row stays usable. Choosing **CITP** on the ToskLight row with no address, or with `999.1.1.1`, explains the problem and keeps **Apply** disabled; a valid address applies, the row confirms the endpoint, and it checks the server at the new address. **⚙** on Media Servers opens Show Patch Settings on **Media Servers**, where **Clear Thumbnail Cache** reports how many cached thumbnails were dropped; clearing again reports none. Setting a row to **Off** and applying keeps the server patched and in the table.

## MEDIA-008 — coordinated Media Server patching follows the desk routes

Given the default stage, which sends desk universe 1 as Art-Net 1 and sACN 101, and three discovered Media Servers, **Show Patch > Media Servers** shows:

- Rack A listening to sACN 101 at address 355
- Rack B listening to Art-Net 30, which no desk route sends
- a spare server that answers discovery but not its configuration API

Each card names the server's IP address, **ToskLight Media**, its CITP port, **Online** or **Offline**, and the desk connection. Rack A suggests **DMX 1.355** and reads **Not patched**. Rack B says that no desk route sends its universe. The spare reads **Unavailable** and **Offline**.

For Rack B, **Patch address**:

- at 1.1 is refused for the overlap
- at 1.400 is refused because the 2-layer footprint does not fit the universe
- at universe 20, which no route sends, is refused before anything changes

Each refusal leaves the Media Server untouched.

A **Patch address** at 1.180 whose server update fails restores the desk: no fixture remains bound to that output, and the card still reads **Not patched**. Retrying sends the server Art-Net universe 1 at address 180. The card then confirms that desk and Media Server use DMX 1.180 and reads **Patched**, and the desk fixture stores that address and the server's CITP endpoint.

**Patch suggested** on Rack A patches DMX 1.355 without contacting the server and reads **Patched**. **Check connection** on its patched row turns the row **Offline** with what to check, and the discovered card follows with **Desk connection: Offline**.

A desk fixture whose address, endpoint, or delivered universe differs from the output reads **Address differs**, **Endpoint differs**, or **Not received** with both sides and the fixing action, never **Patched**.

The coordinated update route refuses a sACN universe 0, an Art-Net universe above 32767, and an unknown protocol. The desk's request carries the route protocol and universe. The Media Server applies an Art-Net 0, Art-Net 12, or sACN 101 move live.

## PIXEL-001 — desktop recursive multi-file conversion

Given the packaged Pixel application on macOS or Windows, choose **Convert multiple files** from its menu-bar item on macOS or notification-area item on Windows. On macOS, select a mixture of multiple supported videos/images and multiple nested folders in the same native picker. On Windows, select individual supported videos/images in the first optional picker and multiple nested folders in the second optional picker. Clicking and navigating in a dialog keeps it visible until the operator confirms or cancels it; cancelling one Windows step does not discard selections from the other. Audio, unrelated files, duplicate selections, and directory symlinks produce no jobs. Enter a starting folder and file and verify the first source receives that exact address, file `254` continues at file `1` in the next folder, and a selection that exceeds folder `199` is refused before any job starts. An occupied or reserved address is reported as failed and is not overwritten; later sources retain their consecutive assigned addresses.

## PIXEL-002 — conversion progress, retry, and report

After a macOS or Windows multi-file selection is accepted, the Library opens in the default browser and reports completed files against the batch total while every active file shows honest determinate or indeterminate progress. A first conversion failure retries once at the same address and visibly says **Retrying**. Success on that retry finishes normally. A second failure skips the file and leaves its address, filename, two-attempt state, and final reason in the completed conversion report. Other queued files continue, and cancelling an active file does not retry it.

## PIXEL-003 — Windows fullscreen recovery and display movement

On Windows, start Pixel with a monitor output configured fullscreen. The picture covers that display without a border, frame, or title and stays above other applications on that display. Clicking the picture shows a short overlay naming double-click or **Ctrl + Shift + -** as the way back to a normal decorated window and **Ctrl + Shift + Arrow** as display movement; it remains visible above a blacked-out master and disappears without changing the picture. Double-clicking returns only that output to its configured window size and normal window level. With displays arranged on multiple sides, each arrow chord moves the focused picture to the nearest display in that direction and keeps it fullscreen and always on top. A direction with no display does nothing. The minus chord provides the same focused-output restore behavior.

## PIXEL-004 — media file and folder notes

In Pixel's Library, select one media file and save multiline licence and attribution text in **Note**. Restart Pixel and verify the exact whitespace remains. Move that media to another address and then into Parking; after rediscovery, the note still follows that media file rather than its former numeric slot. Select the first file in a folder and Shift-click a later file; every occupied slot between them is selected in ascending address order while gaps remain harmless. Command-click on macOS or Control-click on Windows another range start, then Shift-click its end; the new range is added without losing the first. With different notes selected, verify the editor explicitly says **Multiple values** before applying one exact note to all selected files. Repeat with one and several folder cards, including an empty folder. **Clear note** removes only the selected notes while retaining media names, authored BPM corrections, folder names, icons, pictures, addresses, and content. A request with a duplicate, invalid, or missing target is refused without changing any selected note.

## PIXEL-005 — disable, re-enable, and delete media files

Select one media file in Pixel's Library and turn **Enabled** off. The card remains at the same address and retains its name, note, authored BPM, thumbnail, source, and playable bytes through a restart. Select that address on both a content layer and a mask layer, including while it was already playing: each layer immediately becomes transparent, releases the playback/audio session, and shows no stale frame. Turn **Enabled** on and verify playback starts normally again from the same address. With several files selected, use **Disable selected** and **Enable selected** and verify exactly those stable media identities change in one replay-safe request without changing their addresses or metadata. Press **Delete media**, cancel confirmation, and verify nothing changes. Confirm deletion and verify the playable file, preserved source, thumbnail, and per-address metadata are removed, the address is free, other files remain unchanged, and every layer still selecting that address is transparent without stale video or audio. Repeat through **Delete selected**: one confirmation removes exactly the selected set through one replay-safe request, while cancellation changes nothing. Retrying either accepted edit with the same request ID does not execute it twice; an invalid, duplicate, empty, or missing selection is refused without changing the library.

## PIXEL-006 — useful, retryable, and custom media thumbnails

Import a multi-frame clip whose opening samples are black and whose later samples contain visible detail. Verify the generated thumbnail uses a useful non-black sampled frame and that the selection is deterministic across retries. Select the media file and choose **Retry thumbnail**; verify one replay-safe request regenerates exactly that stable item's thumbnail from the playable clip, refreshes the visible image, and changes no address, content, name, note, BPM correction, or enabled state. If regeneration fails, the previous thumbnail remains. Choose **Upload custom thumbnail**, supply a valid PNG, JPEG, GIF, or WebP image no larger than 16 MiB, and verify Pixel normalizes it to JPEG and refreshes only the selected file. Invalid, empty, non-image, oversized, or excessive-dimension uploads are refused without replacing the current thumbnail. Move, swap, park, and compact the item and verify both automatic and custom thumbnails follow the media identity.

## PIXEL-007 — library media preview

In Pixel's Library web view, select an imported video and verify its inspector advances through a sequence of pictures while the item remains selected; this is a management preview and does not start a layer or alter output. The source stays private and Pixel decodes its native clip format to bounded JPEG frames because that format is not generally browser-playable. Select an image and verify its thumbnail remains visible. If a video frame cannot be decoded, the inspector keeps the item's existing thumbnail instead of showing a broken image, and no library metadata, address, playback state, or media bytes change.

## PIXEL-008 — Art-Net startup fallback

Configure Pixel with an enabled Art-Net output and occupy its configured Art-Net UDP port before starting Pixel. Pixel still starts its administration interface and does not accept Art-Net input for that run. **Settings > Network & DMX** visibly warns that Pixel started without Art-Net input and retains the stored address so the operator can correct the conflict. Releasing the port and restarting restores Art-Net; Pixel never silently changes the configured address or reports the listener as active while it is unavailable.

## PIXEL-009 — Pixel Map workflow and the reference multi-output layout

Open **Pixel Map** from the Media Server dock. The window shows the chosen output's live picture beside its configuration, with **Display Regions** and **Pixel Zones** as window-title tabs; Settings no longer carries a pixel-map editor. Display regions are one table; pixel zones are a **Placement** table and a **Patch** table that both fit beside the picture in a 1280-pixel window without scrolling sideways. Selecting a row marks its shape on the picture, and pressing a shape selects its row in every table of the open tab; the other tab's shapes stay visible but do not answer a press.

Drag the selected shape on the picture with a mouse or a finger to move it, and drag one of its finger-sized corner handles to resize it. The shape never leaves the canvas and a corner never crosses the opposite one; the moved values appear in the row at once, a tap without movement only selects, and the arrow keys (Shift for larger steps, Alt to resize) do the same from the keyboard. Nothing is sent until **Save pixel map**.

On an HDMI output, configure the reference layout: a **Centre** display region covering the middle third of the canvas and turned clockwise, and **Left strip** and **Right strip** RGB zones at the canvas edges, one sent over Art-Net and one over sACN. Choose a second, projector output and give it two independent display regions that overlap the HDMI slice. Save both. Each save carries only that output's pixel map, so neither output's resolution, monitor, nor canvas changes. Reload the page: each output reopens with exactly what was saved, and the first output's shapes and rows are unchanged by the second output's edits. Switching to **Desk merge** still offers a desk input patch per zone, and a zone whose universe no enabled route carries still blocks the save with a named problem.

## PIXEL-010 — DMX page names where each output listens

Open Pixel's **DMX** page. For each output, confirm **Listening for DMX on** shows the protocol, universe and start address set in Settings > Network & DMX, and that the page explains the output's name (such as **Main**), the **Master** row and the **Layer** rows. Confirm the table gives each of Master and Layer 1–8 its **DMX patch** as universe and first–last address matching Connect to Console's patch, and that a layer with nothing selected reads **None selected** in **Selected media** rather than 000/000. Change the start address in Settings and confirm the page follows within a few seconds without reopening it; clear the start address and confirm **Not configured** with a note on where to set it.

## PIXEL-011 — isolated live preview from the Library

With a desk driving an output on two layers and the master at half, open Pixel's **Library** and confirm **Enable preview** sits below **Take over playback**, and that choosing media while it is off changes no layer. Turn it on: playback is taken over, every layer goes out and the master goes full. Click a media item: it plays on Layer 1 at full level, looping, with every other layer out and the master full; click another and the output switches to it. Turn preview off: each layer's slot, level and play mode and the master level return to what they were, and playback returns to the desk. Turn it on again and navigate to another page: the output is restored the same way. Reload the page and confirm preview is off and the library is unchanged.

## PIXEL-012 — a countdown to a time every day

Set the server's UTC offset. Create a text source of kind **Countdown to a time every day** and enter 21:00 with no date. Confirm that its preview and an output layer showing it count to 21:00 local time and tick down live. Set **After zero** to count up and move the target just before now. Confirm that it counts up after zero until local midnight and then counts to the next day's time. Change the server's UTC offset and confirm the countdown follows. Confirm an existing **Countdown to a moment** still counts to its saved date and time.

## RUNNING-001 — containment, deduplication, and identity

Start one Cuelist through several assignments or control surfaces, with a Dynamic contained in it; start one independent Dynamic, Timecode, and Macro. Running shows exactly four rows. The Cuelist row uses the Cuelist's own number and name plus its current Cue, not an assignment number, and suppresses the contained Dynamic. The other rows use their own stable identities and show **Cue —**.

## RUNNING-002 — filters and live reconciliation

All is the default. A pane's **Running kind** setting persists All, Cuelists, Dynamics, Timecodes, or Macros independently from other Running panes. Running is available through **Open Window** and is absent from Built-ins. Start, Cue change, pause, resume, completion, release, cancellation, and stop update or remove rows without reopening. Every empty filtered view names the selected kind.

## RUNNING-003 — exact Off convergence

For each row kind, press **Off** and verify only the named runtime is released, stopped, or cancelled. A Cuelist row addresses the shared Cuelist identity rather than whichever surface first exposed it. A rejected action leaves an actionable error on the Running surface. Repeated presses while the request is pending submit one action. The same final runtime state and row removal result when the transition originates from software, a Virtual Playback, attached hardware, keyboard, OSC, WebSocket, or HTTP.
