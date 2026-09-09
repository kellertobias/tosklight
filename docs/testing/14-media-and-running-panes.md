# Media and Running Panes

These scenarios are the operator acceptance contract for the capability-gated Media operating surface and the authoritative Running overview.

## MEDIA-005 — two effect banks and shared presets

Given a Media Server is patched and its Effects library assigns two distinct presets, when an
operator selects them in Bank 1 and Bank 2 and changes each Effect Strength, then both the Media
Server and ToskLight Control show exactly those two ordered banks with the literal controls
**Effect Select** and **Effect Strength**. Slot 0 reads Off, missing slots remain visibly
unassigned, and the isolated layer preview matches the final output ordering.

When the operator edits either preset in the Effects library, every bank selecting that slot uses
the new settings without changing its Select or Strength value. The legacy four-amount personality
continues to decode with its previous meaning.

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

Folder/File and Mask Folder/Mask File encoders remain immediate. The touch browser presents a discoverable **Media / Mask** choice only when masks are advertised. Secondary controls follow fixture and connection capabilities, and a native ToskLight Media Server action appears only behind its advertised capability. Restarting the desktop, changing show, disconnecting, and reconnecting preserve the pane's stable server, layer, browser, section, and secondary-region configuration without changing portable show data.

## PIXEL-001 — desktop recursive multi-file conversion

Given the packaged Pixel application on macOS or Windows, choose **Convert multiple files** from its menu-bar item on macOS or notification-area item on Windows. On macOS, select a mixture of multiple supported videos/images and multiple nested folders in the same native picker. On Windows, select individual supported videos/images in the first optional picker and multiple nested folders in the second optional picker. Clicking and navigating in a dialog keeps it visible until the operator confirms or cancels it; cancelling one Windows step does not discard selections from the other. Audio, unrelated files, duplicate selections, and directory symlinks produce no jobs. Enter a starting folder and file and verify the first source receives that exact address, file `254` continues at file `1` in the next folder, and a selection that exceeds folder `199` is refused before any job starts. An occupied or reserved address is reported as failed and is not overwritten; later sources retain their consecutive assigned addresses.

## PIXEL-002 — conversion progress, retry, and report

After a macOS or Windows multi-file selection is accepted, the Library opens in the default browser and reports completed files against the batch total while every active file shows honest determinate or indeterminate progress. A first conversion failure retries once at the same address and visibly says **Retrying**. Success on that retry finishes normally. A second failure skips the file and leaves its address, filename, two-attempt state, and final reason in the completed conversion report. Other queued files continue, and cancelling an active file does not retry it.

## PIXEL-003 — Windows fullscreen recovery and display movement

On Windows, start Pixel with a monitor output configured fullscreen. The picture covers that display without a border, frame, or title. Clicking the picture shows a short overlay naming **Ctrl + Shift + -** as the way back to a normal decorated window and **Ctrl + Shift + Arrow** as display movement; it remains visible above a blacked-out master and disappears without changing the picture. With displays arranged on multiple sides, each arrow chord moves the focused picture to the nearest display in that direction and keeps it fullscreen. A direction with no display does nothing. The minus chord returns only the focused output to its configured window size.

## PIXEL-004 — media file and folder notes

In Pixel's Library, select one media file and save multiline licence and attribution text in **Note**. Restart Pixel and verify the exact whitespace remains. Move that media to another address and then into Parking; after rediscovery, the note still follows that media file rather than its former numeric slot. Select the first file in a folder and Shift-click a later file; every occupied slot between them is selected in ascending address order while gaps remain harmless. Command-click on macOS or Control-click on Windows another range start, then Shift-click its end; the new range is added without losing the first. With different notes selected, verify the editor explicitly says **Multiple values** before applying one exact note to all selected files. Repeat with one and several folder cards, including an empty folder. **Clear note** removes only the selected notes while retaining media names, authored BPM corrections, folder names, icons, pictures, addresses, and content. A request with a duplicate, invalid, or missing target is refused without changing any selected note.

## PIXEL-005 — disable, re-enable, and delete media files

Select one media file in Pixel's Library and turn **Enabled** off. The card remains at the same address and retains its name, note, authored BPM, thumbnail, source, and playable bytes through a restart. Select that address on both a content layer and a mask layer, including while it was already playing: each layer immediately becomes transparent, releases the playback/audio session, and shows no stale frame. Turn **Enabled** on and verify playback starts normally again from the same address. With several files selected, use **Disable selected** and **Enable selected** and verify exactly those stable media identities change in one replay-safe request without changing their addresses or metadata. Press **Delete media**, cancel confirmation, and verify nothing changes. Confirm deletion and verify the playable file, preserved source, thumbnail, and per-address metadata are removed, the address is free, other files remain unchanged, and every layer still selecting that address is transparent without stale video or audio. Repeat through **Delete selected**: one confirmation removes exactly the selected set through one replay-safe request, while cancellation changes nothing. Retrying either accepted edit with the same request ID does not execute it twice; an invalid, duplicate, empty, or missing selection is refused without changing the library.

## PIXEL-006 — useful, retryable, and custom media thumbnails

Import a multi-frame clip whose opening samples are black and whose later samples contain visible detail. Verify the generated thumbnail uses a useful non-black sampled frame and that the selection is deterministic across retries. Select the media file and choose **Retry thumbnail**; verify one replay-safe request regenerates exactly that stable item's thumbnail from the playable clip, refreshes the visible image, and changes no address, content, name, note, BPM correction, or enabled state. If regeneration fails, the previous thumbnail remains. Choose **Upload custom thumbnail**, supply a valid PNG, JPEG, GIF, or WebP image no larger than 16 MiB, and verify Pixel normalizes it to JPEG and refreshes only the selected file. Invalid, empty, non-image, oversized, or excessive-dimension uploads are refused without replacing the current thumbnail. Move, swap, park, and compact the item and verify both automatic and custom thumbnails follow the media identity.

## RUNNING-001 — containment, deduplication, and identity

Start one Cuelist through several assignments or control surfaces, with a Dynamic contained in it; start one independent Dynamic, Timecode, and Macro. Running shows exactly four rows. The Cuelist row uses the Cuelist's own number and name plus its current Cue, not an assignment number, and suppresses the contained Dynamic. The other rows use their own stable identities and show **Cue —**.

## RUNNING-002 — filters and live reconciliation

All is the default. A pane's **Running kind** setting persists All, Cuelists, Dynamics, Timecodes, or Macros independently from other Running panes. Running is available through **Open Window** and is absent from Built-ins. Start, Cue change, pause, resume, completion, release, cancellation, and stop update or remove rows without reopening. Every empty filtered view names the selected kind.

## RUNNING-003 — exact Off convergence

For each row kind, press **Off** and verify only the named runtime is released, stopped, or cancelled. A Cuelist row addresses the shared Cuelist identity rather than whichever surface first exposed it. A rejected action leaves an actionable error on the Running surface. Repeated presses while the request is pending submit one action. The same final runtime state and row removal result when the transition originates from software, a Virtual Playback, attached hardware, keyboard, OSC, WebSocket, or HTTP.
