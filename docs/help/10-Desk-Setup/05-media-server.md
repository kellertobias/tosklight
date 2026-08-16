# Media Server

The ToskLight Media Server turns still images, video, text, clocks, countdowns, audio-reactive
visualizers, and other generated sources into video outputs controlled by Art-Net or sACN. One
server process can own several independent outputs. Each output has its own monitor or off-screen
surface, resolution, presentation rate, DMX personality, protocol, universe, and start address.

The administration interface is served by the Media Server itself. Open the address configured as
**This interface**. Its left dock follows the Light Desk operator language: the Media Server mark
and current time are followed by **Playback**, **Library**, **Audio**, **DMX**, and
**Settings**. The Audio destination shows the live bass, mid, treble, level, peak, waveform,
spectrum, beat, and tempo analysis alongside the stored input tuning. Those
are the only built-ins in this interface. The connection indicator changes
if live telemetry is lost. While a Light Desk is connected, its active show name appears in that
indicator. If the desk stops announcing itself, the name disappears instead
of leaving a stale show identity behind. The Light Desk's own dock continues to name the active
show as usual.

The administration interface opens directly on **Playback**. Its title shows the running server's
usable IP address and the selected output's active DMX universe/start address on the first line,
with the current library item count underneath. These are running-process facts: a saved output
address that applies on restart is not presented as active before that restart happens.

## Start and configure the server

During development, run `npm run build:media:open` after changes, or `npm run open:media` to launch
the latest existing build. A deployed server reads `media/media-server.json`
from its working directory unless `MEDIA_CONFIG` names another file. Validate an edited file before
starting outputs:

```sh
media-server --check-configuration
```

The **Settings** page separates the values deliberately. Its tabs are **Libraries**, **Picture**,
**Sound**, **Network**, **DMX**, and **Logs**, in that order. Every edit saves automatically;
there is no separate Save settings action:

- **Libraries** identifies the process, reports the current library revision, and explains where
  the `library.root` setting lives. This path is changed in the configuration file rather than in
  the browser.
- **Network** shows each configured listener beside the address actually in use. **Where this
  server listens** contains Art-Net, sACN, CITP, and the administration interface. **Where this
  server sends** contains the optional Light Speed Group stream destination. **Light and Media are
  on this computer** temporarily resolves listeners to `127.0.0.1` without destroying the stored
  installation addresses. **DMX input** contains each output's personality, Art-Net or sACN protocol,
  universe, and start address. The page limits the start address so the complete personality
  remains in one 512-slot universe. **Audio** configures the input used by audio-reactive sources.
- **Picture** has one editor per output. The monitor list shows the operating-system name and
  current pixel resolution. Render size can follow that monitor, use the 480p, 720p, or 1080p
  preset, or use a manual width and height. Presentation can be display synchronized, unlocked,
  or fixed to a listed broadcast-aware rate, including PAL and fractional NTSC rates. **Sound**
  chooses the output device independently.
- **Logs** keeps the visible-log filter and running server log level together.

Saved network and output changes are stored immediately but do not replace sockets or output
surfaces underneath a running show. Their section header reads **Saved automatically · Applies on
restart**. The explanatory notice and **Revert to current settings** action appear only while the
stored next-start values differ from the values this process actually started with. Revert writes
those active values back through the same settings API; restarting applies a kept change. A refused
edit remains marked **Not saved** with the reason displayed; it is never silently adjusted.

Use standard CITP port `4809` unless the installation has an explicit reason to use another TCP
port. Bind show protocols only to the trusted lighting network. `0.0.0.0` is valid for a listener
on every local interface; it is not a valid destination.

The centred **Take over playback** switch remains available in the left dock on every Media Server
screen. It controls the selected output, updates immediately, and remains at the requested value
while the server confirms the change; a refusal restores the authoritative value and appears as a
toast.

Select **Master** in Playback to transform the completed program after all layers have composited.
**Geometry** provides position, independent scale axes, rotation, and Fit, Fill, Native, or Stretch
scale modes. **Shapers** move the left, right, top, and bottom edges, rotate each edge, or rotate the
complete shaper module. The Master mask shapes the completed program; it does not select ordinary
layer content. These final controls are part of the same compositor pass used for the display,
browser live preview, and CITP Program preview.

The Playback title shows a second **Media / VIS / Text** tab group while Content or Mask browsing
is visible. It filters the pool to ordinary media folders, generated visualizers, or text sources
without changing an address by itself. Master Content cards are disabled because Master has no
content address; switch to **Mask** to select or clear its output-level mask.

## Prepare the media library

The **Library** page uses the title buttons **Media**, **Visualizers**, and **Text** to filter one
shared three-column surface: folder preset pool, file or source preset pool, and the selected
preview and editor. Files `0` and `255` are blank sentinels and cannot hold content. Normal file
addresses are `1` through `254`. Media uses folders `1` through `199`, text uses `200` through
`249`, and generated visualizers use `250` through `255`.

- **Rename** changes a clip's operator-facing name without changing its stable identity.
- **Move** chooses a new numbered address. An occupied address is refused unless **Swap** is chosen;
  the server never silently overwrites another clip.
- Select a folder normally to open its inspector. Its name and built-in icon can be changed or
  cleared, and media can be uploaded there without changing the numbered folder.
- Drag one occupied file onto another to swap their slots, or onto an empty slot to move it there.
  Multi-select files and drag the selection onto another folder to fill that folder's first free
  slots. Allocation continues into the next folder only when the target is full.
- Drag one folder card onto another to exchange the complete folders. Folders `900` through `999`
  are parking storage: clips there stay in the library but have no CITP or DMX playback address.
  Move individual clips there, or exchange a full playable folder with a parking folder, when the
  on-air address space is full. Restore them by dragging them back.
- Select an empty media slot to name it and upload directly to that explicit folder/file address.
  Select an occupied slot to replace its media at the same address; the currently playable clip
  remains available until the complete replacement source has been accepted for import.
- **Upload** preserves the uploaded source and starts a visible HAP Alpha import job. Wait for the
  job to finish before expecting playback or its thumbnail. A job can be cancelled, and a failure
  states what is missing or unreadable.
- **Import all** converts every source in the library that does not yet have a playable clip. It is
  the replacement for the old H.264/ProRes re-encode action: the only playback format is HAP Alpha
  in a `.toskclip` container.
- Visualizer cards use representative frames rendered by the actual built-in visualizers. Both
  visualizer and text previews use the configured main output's aspect ratio, and content is fitted
  inside that frame rather than cropped.
- Select an empty visualizer slot and choose its built-in kind first; that exact numbered address is
  then created. An occupied slot starts with the same kind chooser before its tuning controls.
  **New visualizer** in the window title selects the next empty slot. Several instances of the same
  kind can therefore keep independent names, sizes, and colours at stable addresses. Kind, name,
  and tuning changes are live; there is no Save action.
- **City Tunnel** travels continuously through an enclosed neon urban corridor. **Speed** sets the
  forward travel rate, **Count** changes the building/window density, **Size** changes the tunnel
  structure, **Amount** controls its light intensity, and the two colour controls tune the city
  frame and window appearance.
- **Matrix Digital Rain** is a standalone generated visualizer: it produces falling procedural code
  rather than altering a selected media asset. **Count** sets the column density, **Speed** sets the
  fall rate, **Amount** sets brightness, and the two colour controls tune heads/trails. It can be
  assigned, duplicated, transformed, masked, and processed like every other visualizer source.
- **Grid Landscape** travels through transparent synthwave mountain grids on both sides of a
  central route. **Radius** sizes the softly fading horizon sun; **Speed**, **Count**, **Size**,
  **Amount**, and the two colours tune travel, grid, mountains, and light. **Left scenery** and
  **Right scenery** independently select **Off**, **Street lamps**, or **Palm trees**.
- Selecting a text source opens **Edit text** immediately, and every content or appearance change
  is live without a Save action. **New text source** remains in the window title.

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
start address shown under **Settings** > **DMX**, in the **DMX input** section. The 2-layer personality occupies 75 slots;
the 8-layer personality occupies 279 slots. Open **DMX** in the left dock for the dedicated
**Diagnostics** page and generated GDTF downloads. Its **Configure DMX input** action returns to
the corresponding Settings tab.

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
and preserves their line breaks. Existing sources update live without a Save action. Clock formats
are `HH:MM`, `HH:MM:SS`, `hh:mm`, and `hh:mm:ss`; the fixed UTC offset makes the rendered server
output independent of the browser's time zone. Countdown formats are seconds, minutes, `mm:ss`,
`hh:mm:ss`, and `h:mm:ss`. A countdown can hold at zero, continue negative, or count upward after
zero; `mm:ss` can optionally roll its minutes over at 60. The separator is configurable for both
clock and countdown sources. **Library** > **Visualizers** uses folders `250` through `255` and
the same pool-and-inspector layout with a screenshot-like preview. Selecting a visualizer is only
for inspection and tuning; it does not put that source on an output. Use **Audio** to select and tune
the audio input that feeds audio-reactive sources; the meters prove the running input rather than
only the stored device choice. Long-running and unavailable states remain visible in that tab.

Use **Settings** > **Logs** for diagnosis. **Show** filters records already in the browser. **Server log level** is
separate and changes which records the running process captures. That level is intentionally
temporary: restarting reads `MEDIA_LOG` again. Raising it to Debug does not change the **Show**
filter, and lowering the browser filter does not change the process.

For **Equalizer Bars**, **Bloom** controls the additive glow around the bars from `0` (no glow) to
`1` (full glow), with a default of `1`. The same persisted value is available in the visualizer
editor and as **Slot 1 · Bloom** while an Equalizer layer is active.
For **Waveform Oscilloscope**, **Size** controls the trace's vertical expansion from `0.005`
(10% of the designed height) through the persisted default `0.05` to `0.1` (200%). The same Size
value is available as **Slot 1 · Size** on an active Waveform layer and updates live.

On **Playback**, take over the selected output with the switch in the left dock before changing a
layer. Until takeover, folders, files, and playback controls remain read-only. The master picture
is the same demand-driven composite frame advertised to CITP, at the configured output aspect
ratio; it is not the thumbnail of the currently browsed file. Taking over also removes the startup
connection message, so program output does not depend on receiving a DMX packet first. When the
renderer is running, every layer tile is an isolated live renderer frame rather than a catalog
thumbnail. A damaged or unsupported frame marks that layer failed and surfaces the operator-safe
cause in Playback; the detailed decoder record remains available under **Settings** > **Logs**.
When the
full play-mode list is available, use its dropdown or the always-visible **Stop**, **Play**, and
**Play looped** actions. The **Effects** section has
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
**Beat Move** temporarily offsets a layer whenever the audio detector reports a new beat, then
returns it to the saved resting position. **Movement amount** sets the maximum offset,
**Direction** selects Up, Down, Left, or Right, and **Return time** sets the smooth decay duration.
Bypassing the effect immediately restores the exact configured layer position.
**Kaleidoscope** mirrors the live layer image around its centre without changing the source asset.
**Mirror repetitions** selects one, two, or a higher supported number of repeated wedges, while
**Angle** rotates the mirror axis. One repetition is the unchanged source; bypassing the effect
also restores that exact source immediately.
**Rasterized Print** turns the live image into a dot-based print without changing its source asset.
**Print mode** selects **Black and White** ink or a four-colour **CMYK** treatment, and **Dot size**
sets the apparent print-cell size from 2 to 32 source pixels. Bypassing restores the exact source.
**Beat Scan** sends one or more bright scan lines across the live image on each detected beat. The
audio hit strength decides whether that beat spawns one, two, or three lines; there is deliberately
no fixed spawn-count control. **Scan width**, **Sharp** or **Soft** edges, **Edge falloff**, and
**Travel time** shape the lines. Consecutive beats keep earlier lines travelling until they leave.
**Beat Scale and Turn** pulses the live image larger on each detected beat, then eases it back to
its configured resting transform. **Turn** independently enables a subtle rotation; **Scale
amount**, **Rotation amount**, and **Return time** define the pulse without changing the source.
**Beat Grid Wave** replaces the live image with a perspective grid and launches a visible wave on
each detected beat. Select **Centre**, **Top**, **Right**, **Bottom**, or **Left** as the origin,
then tune **Grid density**, **Wave height**, **Travel time**, **Grid hue**, and **Brightness**.
Repeated beats keep their own waves while they overlap; bypass restores the unchanged source.
**Beat Form Flash** uses the selected live layer image as a form. Each detected beat places one or
more independently shrinking and fading copies at deterministic varied positions. **Start size**,
**Lifetime**, **Forms per beat**, and **Variation** control the result; a held beat does not create
extra copies, and bypass restores the unchanged source immediately.
**Drawn Image** turns the live source into a color-reduced illustration with source-derived ink
lines. **Stylization strength** blends from the unchanged source to the illustrated treatment;
**Line detail** chooses broader graphic edges or finer subject detail. Bypass restores the exact
source immediately and never alters the library asset.
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
