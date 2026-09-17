# Configure Outputs and Displays

Each Media output combines a rendering destination, presentation configuration, audio destination, control personality, network input, and CITP identity.

## Picture destination

Choose either a physical monitor or an off-screen output. A monitor output stores the exact monitor selector and whether it is fullscreen. Configure the render resolution independently from the desktop's apparent size, then choose the supported presentation mode and frame rate for the destination.

Use the test pattern after every monitor, cable, resolution, refresh-rate, or fullscreen change. A browser preview proves that the engine rendered a frame; only the physical test pattern proves that the intended display received it.

On Windows, fullscreen picture outputs are borderless, have no title bar or window frame, and stay above other windows on their display. Click a fullscreen output to show its short recovery hint. Double-click the picture, or press **Ctrl + Shift + -**, to return that output to a normal decorated window. Press **Ctrl + Shift + an arrow key** to move the still-fullscreen output to the nearest display in that direction; at the edge of the desktop, the output stays on its current display.

> [!danger] Missing graphic
> Add an output-configuration screenshot showing physical versus off-screen destination, monitor selection, fullscreen, render resolution, presentation rate, and test pattern.

## Pixel Map

Open **Pixel Map** in the Pixel dock. Pixel mapping is no longer a Settings tab. The window shows the output's live picture beside the configuration, with the display regions and pixel zones drawn over it at the output's aspect ratio. When more than one output is enabled, choose the output above the picture.

The two title tabs switch between the two kinds of rectangle:

- **Display Regions** shows each screen's slice of the canvas in two tables. **Placement** holds **Name**, **Left**, **Top**, **Right**, and **Bottom**; **Presentation** holds **Rotation**, **Fit**, **Show**, and **Remove**. **Add display region** in the title bar adds a region covering the whole canvas.
- **Pixel Zones** shows each zone in two tables. **Placement** holds **Name**, the four edges, and the **Across** and **Down** pixel counts; **Patch** holds **Fixture type**, **Wiring** order, output **Universe** and **Address**, the resulting **Slots**, **Send**, and **Remove**. **Add pixel zone** in the title bar adds a zone at the next free address. The same tab holds the **Operating mode**, the **Output routes** table (name, Art-Net or sACN, universe, destination, send), and, in **Desk merge** mode, the desk handoff form for the selected zone.

Splitting each kind into two tables keeps them readable beside the picture without scrolling sideways. **Wiring** reads **Rows** (left to right), **Columns** (top to bottom), or the **folded** variants, which turn back at the end of each row or column.

Edges are fractions of the canvas from `0` to `1`. Selecting a row in either table, or editing one of its cells, marks its rectangle on the picture; pressing a rectangle on the picture selects its row in both tables. Only the open tab's rectangles respond to a press; the other tab's are drawn faintly for reference.

### Moving and resizing on the picture

Drag a rectangle on the picture, with the mouse or a finger, to move it; it stops at the canvas edge and keeps its size. The selected rectangle shows four corner handles; drag one to resize from that corner. A corner never crosses the opposite one, so the rectangle always keeps some size. A tap without movement only selects. With the rectangle focused, the arrow keys move it by a hundredth of the canvas (a tenth with **Shift**), and **Alt** with an arrow moves its bottom-right corner instead. Moved edges appear in the table at once, rounded to a thousandth. Moving a zone changes which part of the picture it samples, never its patch.

### Example: a centre screen with side strips

On an HDMI output, add a display region named **Centre**, set **Left** `0.333` and **Right** `0.667`, and choose **Turned clockwise**: that screen shows only the middle third of the canvas, turned. On **Pixel Zones**, add two output routes, set the second to **sACN**, then add **Left strip** (Left `0`, Right `0.05`) and **Right strip** (Left `0.95`, Right `1`), each **1** across and **30** down, on universes 1 and 2. Save. Choose the projector output and add its own regions; they may overlap the HDMI slice, because each output's regions only choose what that output shows. No map changes the canvas, the output resolution, or the monitor.

Edits stay a draft until **Save pixel map**, which stores the whole map for that output and nothing else. The button stays unavailable while nothing has changed or while the map has a problem, which is listed under the tables. Saved maps open in the Pixel Map window unchanged, including maps created before the window existed.

## Audio

Choose the output's sound device explicitly when content carries audio. The system default is useful for a portable workstation but may change when an interface is connected or disconnected. Rehearse the actual device and latency path used in production.

## Control personality and DMX

Open **Settings > Network & DMX**. Network and DMX input share one tab because the listen addresses at the top are the transport DMX arrives on. Below them, **DMX input** sets each output's personality, protocol, universe, and start address; the protocol only chooses which of the listeners above feeds that output. Art-Net normally listens on UDP `6454`, sACN on UDP `5568`, and CITP/MSEX on the configured TCP/UDP port, normally `4809`.

Choose **2 layers** (158 slots) or **8 layers** (512 slots). These are the only personalities, and both use the 3D-object-mapping channel layout of 59 slots per layer followed by a 40-slot Master. The master and layer profiles patched in ToskLight Control must match that personality and address. An eight-layer output fills its universe, so it starts at address 1.

### Download console personalities

Open **DMX > Connect to Console** in the Pixel management interface. Download both the
**Layer** and **Master** files for the target desk. Patch one Layer for every configured Pixel layer
at consecutive addresses, then patch one Master immediately after the last layer. Do not patch the
Master once per layer.

- **MagicQ:** install both native `.hed` files. Pixel also provides generated channel and range
  `.csv` developer sources used to maintain the exact MagicQ attribute assignment and indexed
  one-value-at-a-time Media Folder/File encoders without re-entering every value. Configure the
  server as **CITP MSEX**, set the first
  layer head and the exact number of layers, enable thumbnail connection and live preview, and set
  Pixel's IP address. When MagicQ and Pixel run on the same computer, start Pixel first and set
  MagicQ's **Net host options** to **Normal + Loopback IP**. Use **GET THUMBS** after connecting.
- **grandMA2:** copy both `.xml` files into the `gma2/library` folder of the selected drive, import
  them as fixture types, then patch the Layer and Master footprints in order.
- **grandMA3:** import the two `.gdtf` files directly. The same files are also the standard GDTF
  download for other compatible desks.

Pixel uses Art-Net or sACN for control and CITP/MSEX for discovery, media names, thumbnails, and live
preview. A working CITP connection does not prove the DMX universe and start address are correct;
verify both the Media window previews and an actual Folder/File/Dimmer change.

If Pixel cannot bind its configured Art-Net or sACN port, it keeps running without that input instead of withholding the administration interface. The Network settings show an alert naming that condition; release the port or correct the listen address. A corrected Art-Net or sACN address is bound as soon as it is saved, and the alert disappears once the listener is running.

The configuration supports one or more outputs; the shipped and certified baseline is one Main output. Treat additional outputs as an explicit production configuration and verify each monitor, GPU load, audio path, control footprint, and preview identity.

Settings save automatically, and most of them apply to the running server at once:

| Applies immediately | Applies after a restart, and why |
| --- | --- |
| DMX protocol, universe, and start address | Output target, monitor, full-screen, resolution, and presentation rate — the window, graphics surface, and frame clock are created when the output opens |
| Art-Net, sACN, and Speed Group listen addresses | Sound output device and audio input device — the device stream is opened once |
| Tempo source (Playback BPM or a Speed Group) | Personality — layer state, render slots, and the layer list consoles read over CITP are sized when the output opens |
| Pixel map zones, routes, desk handoffs, and regions | CITP listen address — consoles hold a connection to it and discovered its port |
| Audio gain, auto gain, beat sensitivity, and EQ | Interface (HTTP) address — it serves the page you are editing in |
| Clip switch hold and server time | Media library directory — the library, importer, and model store are opened at startup |

**Light and Media are on this computer** moves the Art-Net, sACN, and Speed Group listeners at once and CITP and the interface on the next start. A section heading shows **Applies on restart** only while such a change is waiting; **Revert to current settings** then returns the waiting fields to the values the server is running with. Choosing another media and configuration folder restarts the server by itself. Layer and playback changes never require a restart.
