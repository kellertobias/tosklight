# Configure Outputs and Displays

Each Media output combines a rendering destination, presentation configuration, audio destination, control personality, network input, and CITP identity.

## Picture destination

Choose either a physical monitor or an off-screen output. A monitor output stores the exact monitor selector and whether it is fullscreen. Configure the render resolution independently from the desktop's apparent size, then choose the supported presentation mode and frame rate for the destination.

Use the test pattern after every monitor, cable, resolution, refresh-rate, or fullscreen change. A browser preview proves that the engine rendered a frame; only the physical test pattern proves that the intended display received it.

On Windows, fullscreen picture outputs are borderless and have no title bar or window frame. Click a fullscreen output to show its short recovery hint. Press **Ctrl + Shift + -** to return that output to a normal window. Press **Ctrl + Shift + an arrow key** to move the still-fullscreen output to the nearest display in that direction; at the edge of the desktop, the output stays on its current display.

> [!danger] Missing graphic
> Add an output-configuration screenshot showing physical versus off-screen destination, monitor selection, fullscreen, render resolution, presentation rate, and test pattern.

## Audio

Choose the output's sound device explicitly when content carries audio. The system default is useful for a portable workstation but may change when an interface is connected or disconnected. Rehearse the actual device and latency path used in production.

## Control personality and DMX

Choose the Media personality and configure its Art-Net or sACN universe and start address. The master and layer profiles patched in ToskLight Control must match that personality and address layout. Art-Net normally listens on UDP `6454`, sACN on UDP `5568`, and CITP/MSEX on the configured TCP/UDP port, normally `4809`.

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

If Pixel cannot bind its configured Art-Net port, it starts without Art-Net input instead of withholding the administration interface. The Network settings show an alert naming that condition; release the port or correct the listen address, then restart Pixel before relying on Art-Net control. Other configured protocols retain their normal startup behavior.

The configuration supports one or more outputs; the shipped and certified baseline is one Main output. Treat additional outputs as an explicit production configuration and verify each monitor, GPU load, audio path, control footprint, and preview identity.

Saved network and output changes apply after restart. The interface can offer to return saved configuration to the active values when a restart should be deferred. Layer and playback changes do not require a restart.
