# Connect ToskLight Control to Pixel

ToskLight Control operates ToskLight Pixel through ordinary Art-Net or sACN channels. CITP/MSEX adds library names, thumbnails, output enumeration, and live previews; it is helpful but not required for programming layers.

## Patch the Media Server fixture

Open **Show > Show Patch**, choose the **ToskLight** manufacturer, and add the combined **Media Server** fixture in its 2-layer or 8-layer mode. Match its personality, protocol, universe, start address, and **Full master controls** channel layout to the Media Server output under **Settings > DMX**. The 2-layer personality occupies 118 slots and the 8-layer personality occupies 352 slots. Existing shows using the earlier 89/323-slot layout remain compatible when Pixel is set to **Mask positioning (v2)**.

The fixture contains independent logical heads for its layers and one shared master head. It remains a normal show fixture, so its values can be selected, programmed, stored in Presets and Cues, and assigned to playbacks. Unpatching it preserves that show programming but suppresses DMX output.

The shared Master homes at 100% intensity. Every media layer homes at 0% intensity so patching a server does not unexpectedly place all layers on air; raise the selected layer's **Intensity** when it should contribute to the output. Layer heads expose the regular Desk controls for intensity, volume, RGB colour, frame position, scale, rotation, playback, mask, and effects. Blur is a playback fader with its own DMX channel, not an effect slot. The RGB operator controls are translated to the Media Server personality's physical CMY channels.

Selecting **Master** in the Media pane selects **Mask** automatically because Master has no content address. Its control sections remain available in this order: **Output**, **Geometry**, **Mask position**, **Shapers**, and **Colour**. Geometry provides scale, scaling mode, position, rotation, and flip/mirror. Shapers provide independent left, right, top, and bottom insertion and rotation plus complete module rotation.

If no Media Server fixture is patched, the Desk's Media pane shows only **No media server is patched** and **Open Patch**. Use that action to open Show Patch. The pane does not display invented layers or stale server content.

## Configure CITP manually

After patching, configure the Media endpoint's IP address and CITP port in the patched fixture's Media settings. Port `4809` is the standard default. ToskLight Control does not currently auto-discover this endpoint. Architect's Rig Editor has a separate **Discover servers** action for CITP while configuring media surfaces.

CITP discovery is not ToskLight show discovery. CITP describes Media outputs and libraries; ToskLight's local-network discovery offers read-only `.show` copies between Desk and the PreViz Rig Editor.

## Work without CITP

When the Media fixture is patched but CITP is unavailable, the Media pane still shows its layers and permits numeric folder/file programming. The Desk does not need thumbnails or library names to emit the correct DMX values. This is the expected fallback, not an error that hides the layer controls.

With CITP connected, the same numeric addresses gain names, thumbnails, and program/layer previews. Treat these as operator feedback. Art-Net or sACN remains authoritative for playback.

> [!danger] Missing graphic
> Add a four-state Desk Media pane comparison: no Media Server patched, patched without CITP, patched with CITP, and patched server offline.

## Diagnose the connection

Check in this order:

1. the Desk patch personality, logical universe, and start address match the Media output;
2. the Desk output route uses the intended protocol and wire universe;
3. the Media **DMX** diagnostics name the expected sender and show changing raw bytes;
4. the manually configured CITP endpoint is reachable when names or previews are required; and
5. saved Media network/output changes were applied by restarting the Media Server.

The Media Server administration interface can take over playback for testing. Release that takeover before judging desk DMX control.
