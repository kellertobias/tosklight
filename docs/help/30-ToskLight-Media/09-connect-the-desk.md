# Connect ToskLight Desk to Media

ToskLight Desk controls the Media Server through ordinary Art-Net or sACN channels. CITP/MSEX adds library names, thumbnails, output enumeration, and live previews; it is helpful but not required for programming layers.

## Patch the Media Server fixture

Open **Show > Show Patch**, choose the **ToskLight** manufacturer, and add the combined **Media Server** fixture in its 2-layer or 8-layer mode. Match its personality, protocol, universe, and start address to the Media Server output under **Settings > DMX**. The 2-layer personality occupies 75 slots and the 8-layer personality occupies 279 slots.

The fixture contains independent logical heads for its layers and one shared master head. It remains a normal show fixture, so its values can be selected, programmed, stored in Presets and Cues, and assigned to playbacks. Unpatching it preserves that show programming but suppresses DMX output.

If no Media Server fixture is patched, the Desk's Media pane shows only **No media server is patched** and **Open Patch**. Use that action to open Show Patch. The pane does not display invented layers or stale server content.

## Configure CITP manually

After patching, configure the Media endpoint's IP address and CITP port in the patched fixture's Media settings. Port `4809` is the standard default. ToskLight Desk does not currently auto-discover this endpoint. The PreViz Rig Editor has a separate **Discover servers** action for CITP while configuring media surfaces.

CITP discovery is not ToskLight show discovery. CITP describes Media outputs and libraries; ToskLight's local-network discovery offers read-only `.show` copies between Desk and the PreViz Rig Editor.

## Work without CITP

When the Media fixture is patched but CITP is unavailable, the Media pane still shows its layers and permits numeric folder/file programming. The Desk does not need thumbnails or library names to emit the correct DMX values. This is the expected fallback, not an error that hides the layer controls.

With CITP connected, the same numeric addresses gain names, thumbnails, and program/layer previews. Treat these as operator feedback. Art-Net or sACN remains authoritative for playback.

## Diagnose the connection

Check in this order:

1. the Desk patch personality, logical universe, and start address match the Media output;
2. the Desk output route uses the intended protocol and wire universe;
3. the Media **DMX** diagnostics name the expected sender and show changing raw bytes;
4. the manually configured CITP endpoint is reachable when names or previews are required; and
5. saved Media network/output changes were applied by restarting the Media Server.

The Media Server administration interface can take over playback for testing. Release that takeover before judging desk DMX control.
