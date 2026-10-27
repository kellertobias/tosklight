# Connect ToskLight Control to Pixel

ToskLight Control operates ToskLight Pixel through ordinary Art-Net or sACN channels. CITP/MSEX adds library names, thumbnails, output enumeration, and live previews; it is helpful but not required for programming layers.

## Patch the Media Server fixture

Open **Show > Show Patch**, choose the **ToskLight** manufacturer, and add the combined **Media Server** fixture in its 2-layer or 8-layer mode. Match its personality, protocol, universe, and start address to the Media Server output under **Settings > Network & DMX**. There are exactly two personalities, and both use the 3D-object-mapping channel layout: the 2-layer personality occupies 158 slots and the 8-layer personality occupies 512 slots, so an 8-layer patch fills its universe and must start at address 1. A show using a legacy media personality must be repatched to the current layout.

The fixture contains independent logical heads for its layers and one shared master head. It remains a normal show fixture, so its values can be selected, programmed, stored in Presets and Cues, and assigned to playbacks. Unpatching it preserves that show programming but suppresses DMX output.

The shared Master homes at 100% intensity. Every media layer homes at 0% intensity so patching a server does not unexpectedly place all layers on air; raise the selected layer's **Intensity** when it should contribute to the output. Layer heads expose the regular Desk controls for intensity, volume, RGB colour, frame position, scale, rotation, playback with **In point** and **Out point**, **Blend mode** and strobe, mask, four **Visualizer Parameters**, **3D model** with **Model pan** and **Model tilt**, and two ordered effect banks. Each bank has **Effect Select**, **Effect Strength**, and **Parameter 1**-**Parameter 4**; Blur is selected as a configured library preset rather than a separate playback fader. Pixel translates the regular colour controls to the personality's CMY channels.

The four **Visualizer Parameters** are channels of their own, never effect-bank parameters. When Pixel is reachable, the Media pane names each control for the selected visualizer, such as **Size** or **Colour**, and shows its value. **Default** means 0: keep the layer's tuning or the configured value. A channel the shown visualizer does not use, or any channel while the layer shows ordinary media, reads **Unused** but stays programmable, so a cue can set it before the visualizer is shown. Without that connection the channels read **Parameter 1**-**Parameter 4**.

Selecting **Master** in the Media pane selects **Mask** automatically because Master has no content address. Its control sections remain available in this order: **Output**, **Geometry**, **Mask position**, **Shapers**, and **Colour**. Geometry provides scale, scaling mode, position, and rotation. A negative scale mirrors the output along that axis; there is no separate flip/mirror control. Shapers provide independent left, right, top, and bottom insertion and rotation plus complete module rotation.

If no Media Server fixture is patched, the Desk's Media pane shows only **No media server is patched** and **Open Patch**. Use that action to open Show Patch. The pane does not display invented layers or stale server content.

## Discover Pixel or configure its endpoint

Open **Show > Show Patch > Media Servers** and choose **Refresh Discovery**. Inspect the discovered output and its suggested patch before choosing **Patch suggested** or **Patch address**. Discovery alone changes nothing. Follow [Discovering a Pixel Media Server](../10-ToskLight-Architect/02-Fixtures-and-Patch/07-media-server-connections.md#discovering-a-tosklight-pixel-media-server) for address and route choices.

When discovery is unavailable, patch manually. Open **Show Patch > Media Servers**, enter Pixel's reachable **IP address** and **CITP port**, then choose **Apply**. The default CITP port is `4809`.

For a **ToskLight Media** row, also enter its **HTTP port**, normally `8080`. Both ports use the row's IP address. HTTP supplies Pixel's native controls, output configuration, and media Cue pictures. CITP does not advertise the HTTP port, and Control does not derive one from the CITP port. Generic CITP servers expose only the CITP port.

**Refresh Discovery** checks configured ToskLight Media endpoints at their stored HTTP ports. **Patch suggested** and **Patch address** bind the selected output and preserve its HTTP port while IP and CITP still match. Unconfigured discovery checks standard ports `4809` and `8080`; configure custom ports manually. Resolve conflicting output bindings or endpoints before patching.

![Media patch, names, and preview connection states](../assets/diagrams/media-connection-states.svg)

CITP supplies media output and library information. Show discovery and Control/Architect show synchronization are separate workflows.

## Follow the desk's Speed Groups

Synchronized play modes can follow a ToskLight Control Speed Group instead of each layer's own
**Playback BPM**, which is set in the Media pane rather than over DMX. The Media Server only receives Speed Groups; the desk stays the
authority.

1. In Media **Settings > Network & DMX**, enter a **Speed Groups** address such as `0.0.0.0:4810`.
   The Media Server starts listening there as soon as the address is saved.
2. In the same tab's **DMX input** section, set **Synchronized playback follows** to **Light desk Speed Group
   A**-**E** for each output. This applies immediately.
3. Keep the Media Server fixture's CITP endpoint set on the desk. While a show is open, the desk
   sends every Speed Group to UDP port `4810` on that address.

**Settings > Network & DMX** then shows the followed desk, each group's tempo, and any refused messages.
If the desk stops sending, the output keeps its last tempo and the panel reports the desk as lost.
When the desk returns, the output follows it again without a restart. For message formats, see the [Speed Group integration reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/media-speed-groups.md).

## Recall media in a show

Select the media layer's logical head in Control, then set its Folder/File and Intensity in **Media**. Use **Master** for the output-wide level. Store and update the result with the normal [Cue programming workflow](../20-ToskLight-Control/20-Programmer-and-Cues/04-programming-cues.md).

The [programmer command line](../20-ToskLight-Control/20-Programmer-and-Cues/01-command-line.md) selects the same fixture and head IDs and records the same values. Use its documented selection and Record commands; Pixel's browser controls do not create a second command language.

## Configure effect presets

Prepare [effect presets](08-effects-and-masks.md) on Pixel, then use Control's effect bank controls to choose the number, strength and four parameters. The library preset is shared; each layer's bank controls its playback. The Media pane uses Pixel's advertised ranges and steps, including whole-number choices, and reports any refused edit.

The Master exposes **Layer Opacity Cycle** separately. It cycles loaded layers without rewriting their stored dimmers.

## Work without CITP

When the Media fixture is patched but CITP is unavailable, the Media pane still shows its layers and permits numeric folder/file programming. The Desk does not need thumbnails or library names to emit the correct DMX values. Use numeric addresses while the library connection is unavailable.

With CITP connected, the same numeric addresses gain names, thumbnails, and program/layer previews. Treat these as operator feedback. Art-Net or sACN remains authoritative for playback.

Cue previews for media-only Cues come from the server's administration interface on its configured HTTP port (default `8080`), separately from CITP. The desk sends the DMX state a Cue would leave, and the server draws it off-screen as a Program or layer picture while its output keeps playing (see [Media Server Cue Snapshots](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/media-cue-snapshots.md)). When that HTTP endpoint is unreachable, those Cues read **Media Server offline** in the Cue list.

## Diagnose the connection

Check in this order:

1. the Desk patch personality, logical universe, and start address match the Media output;
2. the Desk output route uses the intended protocol and wire universe;
3. the Media **DMX** diagnostics name the expected sender and show changing raw bytes;
4. the configured CITP endpoint is reachable for library names and live previews, and the HTTP endpoint is reachable for native controls and media Cue pictures;
5. **Settings > Network & DMX** reports the desk as receiving when outputs follow a Speed Group; and
6. no Media section shows **Applies on restart**; restart Pixel for a pending personality, CITP or picture-setting change. Monitor selection applies immediately.

The Media Server administration interface can take over playback for testing. Release that takeover before judging desk DMX control.

![Pixel DMX diagnostics and decoded layer state](../assets/screenshots/workflows/pixel-dmx-diagnostics.png)

### Read the DMX page

Each output has its own named section; the first is **Main**. **Listening for DMX on** shows its running **Protocol**, **Universe** and **Start address**. Changes from Network & DMX appear without reopening the page. A start
address that is not set reads **Not configured**, with a note saying where to set it.

The table has a **Master** row for the output's dimmer, volume, tint and mask, followed by its **Layer** rows. Layer 1 is at the bottom of the mix. **DMX patch** is the block of channels
that row answers on, as universe and first-last address. **Selected media** is the folder and file
the row is showing from the library, which is not a DMX address: a layer showing nothing reads
**None selected**, and the Master's mask **No mask**, rather than a row of zeros.
