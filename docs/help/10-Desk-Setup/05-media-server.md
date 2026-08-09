# Media Server

The ToskLight Media Server turns still images, video, text, clocks, countdowns, audio-reactive
visualizers, and other generated sources into video outputs controlled by Art-Net or sACN. One
server process can own several independent outputs. Each output has its own monitor or off-screen
surface, resolution, presentation rate, DMX personality, protocol, universe, and start address.

The administration interface is served by the Media Server itself. Open the address configured as
**This interface**, then use its sections to prepare the library, configure outputs, and diagnose
the running process. The connection indicator names the server instance and changes if live
telemetry is lost.

## Start and configure the server

During development, run `npm run open:media`. A deployed server reads `media/media-server.json`
from its working directory unless `MEDIA_CONFIG` names another file. Validate an edited file before
starting outputs:

```sh
media-server --check-configuration
```

The **Settings** page separates the values deliberately:

- **Server** identifies the process and reports the current library revision.
- **Network** shows each configured listener beside the address actually in use. **Where this
  server listens** contains Art-Net, sACN, CITP, and the administration interface. **Where this
  server sends** contains the optional Light Speed Group stream destination. **Light and Media are
  on this computer** temporarily resolves listeners to `127.0.0.1` without destroying the stored
  installation addresses.
- **Outputs** has one card per output. Choose a monitor or an off-screen surface, the render size,
  display-synchronized/fixed/diagnostic presentation, the 2-layer or 8-layer personality, and the
  Art-Net or sACN universe and start address. The page limits the start address so the complete
  personality remains in one 512-slot universe.
- **Library** explains where the `library.root` setting lives. This path is intentionally changed in
  the configuration file, not through the browser.

Saved network and output changes are stored immediately but do not replace sockets or output
surfaces underneath a running show. Restart the Media Server, return to **Settings**, and confirm
that **Configured** and **In use now** agree. A refused edit remains open with the reason displayed;
it is never silently adjusted.

Use standard CITP port `4809` unless the installation has an explicit reason to use another TCP
port. Bind show protocols only to the trusted lighting network. `0.0.0.0` is valid for a listener
on every local interface; it is not a valid destination.

## Prepare the media library

The **Media library** page shows numbered folders and files with their stable names, addresses, and
thumbnails. Files `0` and `255` are blank sentinels and cannot hold media. Normal media addresses
are `1` through `254`.

- **Rename** changes a clip's operator-facing name without changing its stable identity.
- **Move** chooses a new numbered address. An occupied address is refused unless **Swap** is chosen;
  the server never silently overwrites another clip.
- A folder name can be changed or cleared. Its numbered folder remains the same.
- **Upload** selects an explicit folder and file address, preserves the uploaded source, and starts
  a visible HAP Alpha import job. Wait for the job to finish before expecting playback or its
  thumbnail. A job can be cancelled, and a failure states what is missing or unreadable.
- **Import all** converts every source in the library that does not yet have a playable clip. It is
  the replacement for the old H.264/ProRes re-encode action: the only playback format is HAP Alpha
  in a `.toskclip` container.

Thumbnail, source, and playable clip files follow a rename, move, or explicit swap together. A
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
start address shown on **Settings**. The 2-layer personality occupies 75 slots; the 8-layer
personality occupies 279 slots. Use the output's **DMX** page to download its generated GDTF rather
than recreating the channel map by hand.

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

Use **Text** to create and edit static text, clocks, and countdowns. Use **Visualizers** for the
shipped generated effects and their advertised parameters. Use **Audio** to select and tune the
input that feeds audio-reactive sources; the meters prove the running input rather than only the
stored device choice. Long-running and unavailable states remain visible on their page.

Use **Log** for diagnosis. **Show** filters records already in the browser. **Server log level** is
separate and changes which records the running process captures. That level is intentionally
temporary: restarting reads `MEDIA_LOG` again. Raising it to Debug does not change the **Show**
filter, and lowering the browser filter does not change the process.

## Check a show and retain rollback

Before relying on an installation, play representative stills, video, masks, text, and visualizers;
check the master and both ends of every layer personality; view the live preview from the real
console over CITP; and run the diagnostic pattern on every intended monitor. Monitor choice,
orientation, timing, and colour require a human looking at the physical output.

When moving from the earlier standalone Media Server, rehearse against a copy of its library. Do
not let both servers write one live library, and keep the original data untouched until the
rollback window has passed.
