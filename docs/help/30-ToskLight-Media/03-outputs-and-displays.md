# Configure Outputs and Displays

Each Media output combines a rendering destination, presentation configuration, audio destination, control personality, network input, and CITP identity.

## Picture destination

Choose either a physical monitor or an off-screen output. A monitor output stores the exact monitor selector and whether it is fullscreen. Configure the render resolution independently from the desktop's apparent size, then choose the supported presentation mode and frame rate for the destination.

Use the test pattern after every monitor, cable, resolution, refresh-rate, or fullscreen change. A browser preview proves that the engine rendered a frame; only the physical test pattern proves that the intended display received it.

## Audio

Choose the output's sound device explicitly when content carries audio. The system default is useful for a portable workstation but may change when an interface is connected or disconnected. Rehearse the actual device and latency path used in production.

## Control personality and DMX

Choose the Media personality and configure its Art-Net or sACN universe and start address. The master and layer profiles patched in ToskLight Desk must match that personality and address layout. Art-Net normally listens on UDP `6454`, sACN on UDP `5568`, and CITP/MSEX on the configured TCP/UDP port, normally `4809`.

The configuration supports one or more outputs; the shipped and certified baseline is one Main output. Treat additional outputs as an explicit production configuration and verify each monitor, GPU load, audio path, control footprint, and preview identity.

Saved network and output changes apply after restart. The interface can offer to return saved configuration to the active values when a restart should be deferred. Layer and playback changes do not require a restart.
