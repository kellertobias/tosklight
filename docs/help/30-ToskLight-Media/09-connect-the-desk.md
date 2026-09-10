# Connect ToskLight Control to Pixel

ToskLight Control operates ToskLight Pixel through ordinary Art-Net or sACN channels. CITP/MSEX adds library names, thumbnails, output enumeration, and live previews; it is helpful but not required for programming layers.

## Patch the Media Server fixture

Open **Show > Show Patch**, choose the **ToskLight** manufacturer, and add the combined **Media Server** fixture in its 2-layer or 8-layer mode. Match its personality, protocol, universe, start address, and **Effect banks and full master controls** channel layout to the Media Server output under **Settings > DMX**. The 2-layer personality occupies 119 slots and the 8-layer personality occupies 353 slots. Existing shows using the earlier 89/323-slot layout remain compatible when Pixel is set to **Mask positioning (v2)**.

The fixture contains independent logical heads for its layers and one shared master head. It remains a normal show fixture, so its values can be selected, programmed, stored in Presets and Cues, and assigned to playbacks. Unpatching it preserves that show programming but suppresses DMX output.

The shared Master homes at 100% intensity. Every media layer homes at 0% intensity so patching a server does not unexpectedly place all layers on air; raise the selected layer's **Intensity** when it should contribute to the output. Layer heads expose the regular Desk controls for intensity, volume, RGB colour, frame position, scale, rotation, playback, mask, and two ordered effect banks. Each bank has **Effect Select** and **Effect Strength**; Blur is selected as a configured library preset rather than a separate playback fader. The RGB operator controls are translated to the Media Server personality's physical CMY channels.

Selecting **Master** in the Media pane selects **Mask** automatically because Master has no content address. Its control sections remain available in this order: **Output**, **Geometry**, **Mask position**, **Shapers**, and **Colour**. Geometry provides scale, scaling mode, position, rotation, and flip/mirror. Shapers provide independent left, right, top, and bottom insertion and rotation plus complete module rotation.

If no Media Server fixture is patched, the Desk's Media pane shows only **No media server is patched** and **Open Patch**. Use that action to open Show Patch. The pane does not display invented layers or stale server content.

## Configure CITP manually

After patching, configure the Media endpoint's IP address and CITP port in the patched fixture's Media settings. Port `4809` is the standard default. ToskLight Control does not currently auto-discover this endpoint. Architect's Rig Editor has a separate **Discover servers** action for CITP while configuring media surfaces.

CITP discovery is not ToskLight show discovery. CITP describes Media outputs and libraries; ToskLight's local-network discovery offers read-only `.show` copies between Desk and the PreViz Rig Editor.

## Configure effect presets

The Effects tab in the Media Server Library stores the effect type, operator name, and typed
parameters behind each numbered preset. The Media Server states what every parameter accepts, and
the Desk offers exactly that range and step — an
angle stops where the server stops it, and a count moves in whole numbers. A control shows the
value it is holding while you change it, and a refusal is reported as the Media Server's own
sentence rather than a silent no-op.

Effect **Select** and **Strength** stay on DMX. The library configures the shared preset; the two
ordered banks play it. The Master exposes the fixed **Layer Opacity Cycle** beat ratio separately.

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

## Connect another console

Open Pixel’s **DMX** page and press **Connect to Console** at the top. Choose **MagicQ**,
**grandMA2**, **grandMA3**, or **GDTF**. Download both **ToskLight Pixel Layer** and
**ToskLight Pixel Master** files from this running server. The files are generated from the
same canonical channel definition used by Pixel’s decoder. The Layer file is used for every
layer, including Layer 1; the Master is a separate fixture.

Configure one output with **8 layers** and **Effect banks and full master controls** in **Settings > DMX**.
Restart Pixel after changing its startup configuration. Use the connection panel’s actual
universe and patch table, which is read from the running decoder. Patch eight Layer fixtures,
followed by one Master fixture. Do not use the current generated files with a legacy output
layout. For the full layout, each Layer occupies 39 slots and Master occupies 41; eight layers
and Master occupy 353 slots. Starting at address 1, the Layer starts are 1, 40, 79, 118, 157,
196, 235, and 274; Master starts at 313 and ends at 353. A different configured start address
shifts all these addresses equally. The complete block must fit in the configured universe.

### Import the generated files

- **MagicQ:** download the two native `.hed` files and copy them into MagicQ’s `show/heads`
  directory. Open a new empty show, open **PATCH**, choose manufacturer **ToskLight**, and
  patch eight **Pixel Layer** heads and one **Pixel Master** head at the advertised addresses.
  Rescan or restart MagicQ if newly copied heads are not listed.
- **grandMA2:** place both XML files in `gma2/library` on the selected drive. In
  **Setup > Patch & Fixture Schedule**, import both fixture types and add eight Layer fixtures
  plus one Master fixture.
- **grandMA3:** import both GDTF files as fixture types in **Patch**, then add eight Layer
  fixtures and one Master fixture.
- **GDTF:** import both GDTF files using the console’s fixture-type import. The exact menus
  depend on the console. Add eight Layer fixtures and one Master fixture.

### DMX and CITP

Set Pixel and the console to the same transport and universe. For the acceptance workflow,
use **sACN**. In MagicQ, enable the patched universe’s sACN output in **SETUP > View DMX I/O**
and send to Pixel’s address. Other consoles have their own network output configuration.
For Art-Net, check the console’s zero-based wire-universe convention; sACN universes start at 1.

In MagicQ’s **SETUP > View System > Media**, configure a server slot as **CITP MSEX**,
using Pixel’s reachable address and advertised CITP port. Use consecutive Layer head numbers **1–8**, set the first layer head to **1** and
**Layers** to **8**; patch Master as head **9**. Set **Thumbs** to **Enabled** and
**Live prev** to **Yes**, then press **GET THUMBS**. Consoles supporting CITP/MSEX need an equivalent media-server association;
importing a GDTF or personality file alone does not establish this connection. Consult the
console’s media-server settings where CITP is not supported by its current software.

### Same computer and LAN

For both applications on one computer, set MagicQ’s **SETUP > View Settings > Net host options**
to **Normal + Loopback IP**. Older releases called this setting **Send to applications on this PC**;
it is the same setting, not another switch. Enable Pixel’s **Same computer** network preset and
restart. Use `127.0.0.1` as the console’s explicit unicast DMX destination and CITP endpoint.
If the console cannot transmit on loopback, disable the preset, restart Pixel, and use the
computer’s reachable LAN address for both. Select the corresponding console network interface.
A reported listener bind failure must be resolved; a healthy administration page does not prove
that DMX or CITP listeners are available. Same-computer Art-Net requires Pixel’s listener on
`127.0.0.1` to share UDP 6454 with the console; use sACN for the LAN-address fallback.

For separate computers, disable **Same computer**, bind Pixel to the lighting-network interface
or `0.0.0.0`, and restart. Enter Pixel’s actual LAN address at the console. `0.0.0.0` is a listen
address, not a destination; `127.0.0.1` reaches only the console’s own computer. Select the
reachable lighting interface on the console. For a LAN-address connection in MagicQ, set its
**IP address** to the active LAN interface and **Net host options** to **Normal**, without
**Loopback IP**. This also applies when both programs use the same computer’s LAN address.
Automatic discovery requires multicast to reach
both computers on that interface. Manual IP/port configuration can be used when discovery is
unavailable.

Permit the selected DMX transport to Pixel: **UDP 5568 for sACN** or **UDP 6454 for Art-Net**.
CITP discovery and live previews use **UDP 4809**, multicast group **224.0.0.180**, which both
computers must be able to receive; MSEX control and library transfer use Pixel’s advertised
**TCP port**, normally **4809**. Permit the configured HTTP port to open Pixel’s interface from
another computer. The connection panel displays active listeners and the advertised port;
use these values when customized. Check firewall rules and macOS Local Network permissions on
both computers. Stored network changes need a restart before these active values change.

### Verify the visible result

Import recognizable test images into numbered media folders. Release Pixel’s web playback
takeover, raise Master Dimmer and the selected Layer Dimmer, choose a Play Mode, and select the
image with the console’s Media Folder/File attributes. Pixel’s DMX diagnostics must identify the
sender and show changing raw values; the visible output must show the selected image.

In MagicQ’s Media window, select each of the eight layers. All eight must present the same
encoder arrangement, and unused positions must be blank:

| Page | A | B | C | D | E | F | Y | X |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Media | | | | Playback BPM | Speed Multiplier | Play Mode | Media Folder | Media File |
| Colour | | | | | Cyan | Magenta | Yellow | Greyscale |
| Position | | | Scale Y | Scale X | Rotation | Scale Mode | Position Y | Position X |
| Intensity | | | | | | | Volume | Dimmer |
| FX1 | | | | | | | FX1 Parameter | FX1 Select |
| FX2 | | | | | | | FX2 Parameter | FX2 Select |
| Frame | Mask Position X | Mask Position Y | Mask Scale X | Mask Scale Y | Mask Invert | Mask Opacity | Mask Folder | Mask File |

MagicQ’s normal preview shows the master/program output. Hold **SHIFT** and press
**VIEW SERVERS** (the soft button becomes **PREVIEW LAYER**) to switch to the selected layer’s
live preview. Select Layers 1–8 in turn and confirm that moving content in this preview belongs
to the selected layer. Switch back to verify the master/program output. The top layer strip
shows static thumbnail indicators; it does not display eight simultaneous live preview streams.
Use visibly different moving content on the selected layer and the program output to distinguish
the two sources.

Media Folder/File and Mask Folder/File are indexed 0–255; moving an encoder must advance one
index at a time. Confirm identifiable thumbnail images, a visibly updating master/program
preview, and eight separate layer previews showing the correct layers. A successful connection,
downloaded files, or **Finished retrieving thumbnails** does not establish that images are
visible. Patch/select Master and verify its dimmer and fixed **Layer Opacity Cycle** affect the
shared output without shifting any layer’s channels. Repeat from a second computer using Pixel’s
LAN address before declaring the LAN workflow verified.

For missing DMX, check output enablement, protocol, universe, destination, interface, and socket
conflicts. For wrong pictures with incoming DMX, check addresses, dimmers, Play Mode, and web
playback takeover. For blank thumbnails or previews, check CITP reachability and the association
between server, heads, and media addresses. After changing media, press **RELOAD THUMBS** in
MagicQ. The installed MagicQ does not refresh automatically after Pixel’s valid library-change
notification; confirm the changed images after manually reloading. Keep DMX running while testing CITP feedback.

MagicQ menu and loopback guidance follows the [ChamSys media-server manual](https://docs.chamsys.co.uk/magicq/manual/media_servers.html).
