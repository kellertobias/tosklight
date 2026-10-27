# Connect another console

Open Pixel’s **DMX** page and press **Connect to Console** at the top. Choose **MagicQ**,
**grandMA2**, **grandMA3**, or **GDTF**. Download both **ToskLight Pixel Layer** and
**ToskLight Pixel Master** files from this running server. The Layer file is used for every
layer, including Layer 1; the Master is a separate fixture.

Choose the output's **2 layers** or **8 layers** personality in **Settings > Network & DMX**; the
**Configure DMX input** action at the top of the **DMX** page, marked with an external-link icon,
opens that tab. Protocol, universe, and start address apply as soon as they are saved; restart Pixel after changing the personality. Use the connection panel’s
actual universe and patch table, which is read from the running decoder. Patch one Layer fixture
per layer, followed by one Master fixture. Both personalities use the same Layer and Master files:
each Layer occupies 59 slots and Master occupies 40, so eight layers and Master occupy all 512
slots. The Layer starts are 1, 60, 119, 178, 237, 296, 355, and 414;
Master starts at 473 and ends at 512. Because the block fills the universe, an eight-layer output
must start at address 1. A two-layer output occupies 158 slots and can start anywhere up to 355.

### Import the generated files

- **MagicQ:** download the two native `.hed` files and copy them into MagicQ’s `show/heads`
  directory. Open a new empty show, open **PATCH**, choose manufacturer **ToskLight**, and
  patch the advertised number of **Pixel Layer** heads and one **Pixel Master** head at the
  advertised addresses. The Layer HED identifies itself as **CITP MSEX** and carries the
  running output's layer count, so MagicQ creates its Media server row when the first Layer
  is patched. Rescan or restart MagicQ if newly copied heads are not listed. If an older
  version of the head was already patched in a show, start a new show or replace the patched
  head; MagicQ keeps a copy of the personality inside each show.
- **grandMA2:** place both XML files in `gma2/library` on the selected drive. In
  **Setup > Patch & Fixture Schedule**, import both fixture types and add one Layer fixture per configured layer plus one Master fixture.
- **grandMA3:** import both GDTF files as fixture types in **Patch**, then add one Layer fixture per configured layer and one Master fixture.
- **GDTF:** import both GDTF files using the console’s fixture-type import. The exact menus
  depend on the console. Add one Layer fixture per configured layer and one Master fixture.

### DMX and CITP

Set Pixel and the console to the same transport and universe. Use **sACN** for the setup described here. In MagicQ, enable the patched universe’s sACN output in **SETUP > View DMX I/O**
and send to Pixel’s address. Other consoles have their own network output configuration.
For Art-Net, check the console’s zero-based wire-universe convention; sACN universes start at 1.

In MagicQ’s **SETUP > View System > Media**, confirm the row created by patching the Layer
head shows **CITP MSEX**, the first patched Layer head number, and the correct **Layers** count.
Enter Pixel’s reachable IP address and advertised CITP port. Set **Thumb con** to **Enabled**
and **Live prev** to **Yes**, then press **GET THUMBS**. The HED cannot embed the server's
network address or turn on these show settings. Consoles supporting CITP/MSEX need an
equivalent media-server association; importing a GDTF alone does not establish this
connection. Consult the console’s media-server settings where CITP is not supported by its
current software.

### Same computer and LAN

For both applications on one computer, set MagicQ’s **SETUP > View Settings > Net host options**
to **Normal + Loopback IP**. Older releases called this setting **Send to applications on this PC**;
it is the same setting, not another switch. Enable Pixel’s **Same computer** network preset; its
DMX listeners move at once, and CITP moves after a restart. Use `127.0.0.1` as the console’s explicit unicast DMX destination and CITP endpoint.
If the console cannot transmit on loopback, disable the preset, restart Pixel for CITP, and use the
computer’s reachable LAN address for both. Select the corresponding console network interface.
A reported listener bind failure must be resolved; a healthy administration page does not prove
that DMX or CITP listeners are available. Same-computer Art-Net requires Pixel’s listener on
`127.0.0.1` to share UDP 6454 with the console; use sACN for the LAN-address fallback.

For separate computers, disable **Same computer**, bind Pixel to the lighting-network interface
or `0.0.0.0`, and restart Pixel so CITP follows. Enter Pixel’s actual LAN address at the console. `0.0.0.0` is a listen
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
both computers. Art-Net and sACN listeners change as soon as they are saved; a stored CITP or interface address needs a restart before its active value changes.

### Verify the visible result

Import recognizable test images into numbered media folders. Release Pixel’s web playback
takeover, raise Master Dimmer and the selected Layer Dimmer, choose a Play Mode, and select the
image with the console’s Media Folder/File attributes. Pixel’s DMX diagnostics must identify the
sender and show changing raw values; the visible output must show the selected image.

In MagicQ’s Media window, select each of the configured layers. Each must present the same
encoder arrangement, and unused positions must be blank:

| Page | A | B | C | D | E | F | Y | X |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Media | In Point | Out Point | Blend Mode | 3D Model | Speed Multiplier | Play Mode | Media Folder | Media File |
| Colour | | | | | Cyan | Magenta | Yellow | Greyscale |
| Position | Model Pan | Model Tilt | Scale Y | Scale X | Rotation | Scale Mode | Position Y | Position X |
| Intensity | | | | | | | Volume | Dimmer |
| FX1 | FX1 Param 1 | FX1 Param 2 | FX1 Param 3 | FX1 Param 4 | | | FX1 Mix | FX1 Select |
| FX2 | FX2 Param 1 | FX2 Param 2 | FX2 Param 3 | FX2 Param 4 | | | FX2 Mix | FX2 Select |
| Visualizer | Vis Param 1 | Vis Param 2 | Vis Param 3 | Vis Param 4 | | | | |
| Beam page 5 | Mask Position X | Mask Position Y | Mask Scale X | Mask Scale Y | Mask Invert | Mask Opacity | Mask Folder | Mask File |

The **Frame** tile in MagicQ's Media window shows its built-in framing controls. Pixel's mask
controls are on **BEAM > PAGE 5**; selecting the Frame tile does not select those channels.

MagicQ’s normal preview shows the master/program output. Hold **SHIFT** and press
**VIEW SERVERS** (the soft button becomes **PREVIEW LAYER**) to switch to the selected layer’s
live preview. Select Layers 1-8 in turn and confirm that moving content in this preview belongs
to the selected layer. Switch back to verify the master/program output. The top layer strip
shows static thumbnail indicators; it does not display eight simultaneous live preview streams.
Use visibly different moving content on the selected layer and the program output to distinguish
the two sources.

Media Folder/File and Mask Folder/File are indexed 0-255; moving an encoder must advance one
index at a time. MagicQ activates each modified channel separately with its default
**Activate Chans/Head = Chans** setting: changing a Folder alone does not put the corresponding
File into the programmer, and changing a File alone does not put its Folder there. Set both
values before recording a cue that needs the pair. Confirm identifiable thumbnail images, a
visibly updating master/program preview, and separate layer previews showing the correct
layers. A successful connection,
downloaded files, or **Finished retrieving thumbnails** does not establish that images are
visible. Patch/select Master and verify its dimmer and fixed **Layer Opacity Cycle** affect the
shared output without shifting any layer’s channels. For a separate console computer, repeat the check using Pixel's LAN address.

For missing DMX, check output enablement, protocol, universe, destination, interface, and socket
conflicts. For wrong pictures with incoming DMX, check addresses, dimmers, Play Mode, and web
playback takeover. For blank thumbnails or previews, check CITP reachability and the association
between server, heads, and media addresses. After changing media, press **RELOAD THUMBS** in
MagicQ. If MagicQ keeps old pictures after a library change, reload thumbnails and confirm the new images. Keep DMX running while testing CITP feedback.

MagicQ menu and loopback guidance follows the [ChamSys media-server manual](https://docs.chamsys.co.uk/magicq/manual/media_servers.html).
