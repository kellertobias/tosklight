# Media Server

The ToskLight Media Server turns still images, video, text, clocks, countdowns, audio-reactive
visualizers, and other generated sources into video outputs controlled by Art-Net or sACN. One
server process can own several independent outputs. Each output has its own monitor or off-screen
surface, resolution, presentation rate, DMX personality, protocol, universe, and start address.

The administration interface is served by the Media Server itself. Open the address configured as
**This interface**. Its left dock follows the Light Desk operator language: the Media Server mark
and current time are followed by **Dashboard**, **Playback**, **Library**, **Audio**, and
**Settings**. The Audio destination shows the live bass, mid, treble, level, peak, waveform,
spectrum, beat, and tempo analysis alongside the stored input tuning. Those
are the only built-ins in this interface. The connection indicator changes
if live telemetry is lost. While a Light Desk is connected, its active show name appears in that
indicator and on the Dashboard. If the desk stops announcing itself, the name disappears instead
of leaving a stale show identity behind. The Light Desk's own dock continues to name the active
show as usual.

## Start and configure the server

During development, run `npm run build:media:open` after changes, or `npm run open:media` to launch
the latest existing build. A deployed server reads `media/media-server.json`
from its working directory unless `MEDIA_CONFIG` names another file. Validate an edited file before
starting outputs:

```sh
media-server --check-configuration
```

The **Settings** page separates the values deliberately. Its tabs are **Libraries**, **Outputs**,
**Network & Inputs**, and **Logs**, in that order:

- **Libraries** identifies the process, reports the current library revision, and explains where
  the `library.root` setting lives. This path is changed in the configuration file rather than in
  the browser.
- **Network & Inputs** uses the window title buttons **Network**, **DMX**, and **Audio**. **Network**
  shows each configured listener beside the address actually in use. **Where this
  server listens** contains Art-Net, sACN, CITP, and the administration interface. **Where this
  server sends** contains the optional Light Speed Group stream destination. **Light and Media are
  on this computer** temporarily resolves listeners to `127.0.0.1` without destroying the stored
  installation addresses. **DMX** contains each output's personality, Art-Net or sACN protocol,
  universe, and start address. The page limits the start address so the complete personality
  remains in one 512-slot universe. **Audio** configures the input used by audio-reactive sources.
- **Outputs** has one card per output. Choose a monitor or an off-screen surface, render size, and
  display-synchronized, fixed-rate, or diagnostic presentation.
- **Logs** uses the window title buttons **Logs** and **DMX Diagnostics**. **Logs** keeps the
  visible-log filter and running server log level together. **DMX Diagnostics** contains detailed
  receiver state, decoded channel values, and generated GDTF downloads.

Saved network and output changes are stored immediately but do not replace sockets or output
surfaces underneath a running show. Restart the Media Server, return to **Settings**, and confirm
that **Configured** and **In use now** agree. A refused edit remains open with the reason displayed;
it is never silently adjusted.

Use standard CITP port `4809` unless the installation has an explicit reason to use another TCP
port. Bind show protocols only to the trusted lighting network. `0.0.0.0` is valid for a listener
on every local interface; it is not a valid destination.

## Prepare the media library

The **Library** page uses the title buttons **Media**, **Visualizers**, and **Text** to filter one
shared three-column surface: folder preset pool, file or source preset pool, and the selected
preview and editor. Files `0` and `255` are blank sentinels and cannot hold content. Normal file
addresses are `1` through `254`. Media uses folders `1` through `199`, text uses `200` through
`249`, and generated visualizers use `250` through `255`.

- **Rename** changes a clip's operator-facing name without changing its stable identity.
- **Move** chooses a new numbered address. An occupied address is refused unless **Swap** is chosen;
  the server never silently overwrites another clip.
- A folder name can be changed or cleared. Its numbered folder remains the same.
- Drag one occupied file onto another to swap their slots, or onto an empty slot to move it there.
  Multi-select files and drag the selection onto another folder to fill that folder's first free
  slots. Allocation continues into the next folder only when the target is full.
- Drag one folder card onto another to exchange the complete folders. Folders `900` through `999`
  are parking storage: clips there stay in the library but have no CITP or DMX playback address.
  Move individual clips there, or exchange a full playable folder with a parking folder, when the
  on-air address space is full. Restore them by dragging them back.
- **Upload** selects an explicit folder and file address, preserves the uploaded source, and starts
  a visible HAP Alpha import job. Wait for the job to finish before expecting playback or its
  thumbnail. A job can be cancelled, and a failure states what is missing or unreadable.
- **Import all** converts every source in the library that does not yet have a playable clip. It is
  the replacement for the old H.264/ProRes re-encode action: the only playback format is HAP Alpha
  in a `.toskclip` container.

Thumbnail, source, metadata, and playable clip files follow a rename, move, folder reorder, or
explicit swap together. A
missing thumbnail is shown as missing; it is not substituted from another address.

### Convert a folder before copying it to the server

From a ToskLight source checkout with FFmpeg installed, point the recursive converter at an input
folder and a separate output folder:

```sh
./tools/convert-media.sh "/path/to/authored media" "/path/to/rendered media"
```

Or use `npm run convert:media -- "/path/to/input" "/path/to/output"`. The converter searches
recursively, preserves the relative folder structure, and writes HAP Alpha `.toskclip` files with
the source extension replaced. It accepts common still and video extensions, ignores hidden files
and generated hidden folders, skips an output that already exists, and continues far enough to
report every failed source. If two sources such as `Look.png` and `Look.mov` would both become
`Look.toskclip`, it stops before rendering and asks you to rename one. It never deletes or replaces
the authored input.

## Patch and diagnose DMX

Patch each Media output on the lighting desk with the same personality, protocol, universe, and
start address shown under **Settings** > **Network & Inputs** > **DMX**. The 2-layer personality
occupies 75 slots; the 8-layer personality occupies 279 slots. Open **Settings** > **Logs** >
**DMX Diagnostics** to download an output's generated GDTF rather than recreating the channel map
by hand.

The **DMX** page groups canonical channels under each layer and the master. It reports each
channel's absolute address, name, raw value, decoded value, resolution, defaults, and implemented
value ranges. Receive diagnostics show the configured protocol and universe, winning sender,
accepted rate, last-received age, active/stale state, and the exact received bytes covering this
output. Raw values and receiver state arrive through live telemetry; reloading the page repeatedly
is not a diagnostic method.

If the output is not responding, check in this order:

1. the page still says **Connected**;
2. the configured output protocol, universe, and start address match the desk patch;
3. receive diagnostics name the expected sender and remain active;
4. raw bytes change at the expected absolute channels;
5. decoded layer and master values match those bytes; and
6. the output target and presentation loaded after the last restart.

## Operate generated sources and diagnostics

Use **Library** > **Text** to create and edit static text, clocks, and countdowns. Its folder pool
covers `200` through `249`, allowing 12,700 addressable sources. Static text accepts multiple lines
and preserves their line breaks. **Library** > **Visualizers** uses folders `250` through `255` and
the same pool-and-inspector layout with a screenshot-like preview. Selecting a visualizer is only
for inspection and tuning; it does not put that source on an output. Use **Settings** > **Network & Inputs** to select and tune
the audio input that feeds audio-reactive sources; the meters prove the running input rather than
only the stored device choice. Long-running and unavailable states remain visible in that tab.

Use **Settings** > **Logs** for diagnosis. **Show** filters records already in the browser. **Server log level** is
separate and changes which records the running process captures. That level is intentionally
temporary: restarting reads `MEDIA_LOG` again. Raising it to Debug does not change the **Show**
filter, and lowering the browser filter does not change the process.

On **Playback**, take over the selected output before changing a layer. The **Effects** section has
four ordered slots. **Analog TV** starts with restrained CRT defaults: **TV curvature** 30%,
**Distortion** 18%, **Image grain** 20%, and **Glitching** 8%. Curvature bends the picture into a
rounded screen, Distortion controls continuous horizontal-sync and chroma instability, Image grain
adds continuous noise and scanlines, and Glitching controls intermittent tearing and vertical roll.
Set an individual control to 0% to remove only that contribution, choose **Bypassed** to preserve the
slot without rendering it, or choose **None** to clear the slot.
**Digital TV** is the distinct compressed-stream failure: its defaults are **Compression damage**
35%, **Block size** 35%, **Tile displacement** 25%, **Chroma damage** 20%, and **Glitching** 15%.
Compression damage adds quantization and damaged transform blocks, Block size changes the
rectangular grid, Tile displacement moves selected blocks to the wrong source region, Chroma
damage breaks color more strongly than luma, and Glitching controls held intermittent stream
failures. It deliberately does not add Analog TV's snow, scanlines, curvature, or sync roll.
**Feedback** retains earlier live frames as a temporal trail. **Feedback amount** controls how long
the trail remains, **Motion speed** controls how quickly retained frames travel, and **Motion
direction** selects **Top**, **Bottom**, **Left**, **Right**, **Rotate Left**, or **Rotate Right**.
Bypassing Feedback clears its retained-frame history and immediately restores the unmodified live
source; enabling it again starts a new trail from the current frame.
After configuring a slot, release takeover to return control to the desk. The matching **Effect 1**
through **Effect 4** DMX channel then controls that slot's mix without erasing its selected effect or
typed parameters.

Bookmarks for the retired `/layers`, `/audio`, `/dmx`, and `/logs` pages remain compatible: they
open **Playback** or the corresponding area of **Settings**. The retired `/text` and
`/visualizers` destinations open the matching **Library** filter.

This pre-1.0 address allocation replaces the earlier text `200`–`219` and visualizer `220`–`255`
split. Stored visualizer configuration is migrated into `250`–`255` in its existing entry order.
Because the address itself changes, update any cues or external control that named the old
visualizer address before using that show live.

## Check a show and retain rollback

Before relying on an installation, play representative stills, video, masks, text, and visualizers;
check the master and both ends of every layer personality; view the live preview from the real
console over CITP; and run the diagnostic pattern on every intended monitor. Monitor choice,
orientation, timing, and colour require a human looking at the physical output.

When moving from the earlier standalone Media Server, rehearse against a copy of its library. Do
not let both servers write one live library, and keep the original data untouched until the
rollback window has passed.
