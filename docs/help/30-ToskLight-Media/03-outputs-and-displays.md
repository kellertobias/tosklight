# Configure Outputs and Displays

Open **Settings > Outputs**, choose the output and set its display destination. Check the actual
screen with the test pattern after changing a display, cable, resolution, rate or fullscreen state.

![Pixel output configuration](../assets/screenshots/workflows/pixel-outputs.png)

## Picture destination

Choose a monitor or **Off-screen (no window)**. Set render resolution independently of desktop
scaling, then select the supported presentation mode and frame rate. **Settings > Picture > Monitor**
can move a running output without a restart; the choice is saved for next launch.

On Windows, double-click a fullscreen picture or press **Ctrl+Shift+−** to return it to a normal
window. **Ctrl+Shift+arrow** moves a fullscreen output to the next display in that direction.
The Pixel status menu also offers **Move [output] to Display [number]**.

## Pixel Map

Open **Pixel Map** from the dock and choose the output. The picture shows that output's display
regions and pixel zones; select a row or rectangle to highlight it.

| Tab | Purpose |
| --- | --- |
| **Display Regions** | Choose each screen's canvas rectangle, rotation, fit and visibility. |
| **Pixel Zones** | Sample a rectangle into a pixel grid, choose fixture type/wiring and patch it to output routes. |

Edges run from 0 to 1 across the canvas. Drag a rectangle to move it or a corner to resize. Arrow
keys move by 0.01; Shift uses 0.1. Alt+arrow moves the bottom-right corner. Moving a zone changes
its sampled picture, not its DMX patch.

Choose row/column wiring, including folded variants, to match the installed pixels. Configure the
zone's universe/address and Art-Net or sACN routes. **Desk merge** also provides the selected zone's
desk handoff. Check the resulting slot count before enabling **Send**.

Press **Save pixel map** to commit the output's draft. Errors appear below the tables and prevent
saving. Regions and zones may overlap; they do not change the source canvas or monitor resolution.

For example, a centre screen can use Left 0.333 / Right 0.667, while two 1×30-pixel side strips
sample Left 0 / Right 0.05 and Left 0.95 / Right 1 on separate universes.

## Audio and control input

Choose the actual sound-output device for media carrying audio and rehearse it with the production
hardware. A system-default device can change when another interface connects.

Open **Settings > Network & DMX** for listeners and each output's protocol, universe, address and
personality. Match these to the desk patch. **2 layers** occupies 158 slots; **8 layers** occupies
512 and must start at address 1. Each layer uses 59 slots, followed by one 40-slot Master.

![Pixel DMX listeners and output input configuration](../assets/screenshots/workflows/pixel-network-inputs.png)

Art-Net normally uses UDP 6454 and sACN UDP 5568. A listener bind failure appears here even if the
administration page still works. Correct the address or free the port. CITP/MSEX normally uses
port 4809 for names, thumbnails and previews; a CITP connection alone does not prove DMX control.
See [Connect ToskLight Control to Pixel](09-connect-the-desk.md) for patching and other consoles.

## Which settings need a restart?

Settings save automatically. Use the heading's **Applies on restart** indicator to check whether
saved settings differ from those currently running.

| Applies immediately | Applies after restarting Pixel |
| --- | --- |
| DMX protocol, universe and address | Output target, fullscreen, resolution and presentation rate |
| Art-Net, sACN and Speed Group listener addresses | Audio input/output devices |
| Tempo source | 2-layer / 8-layer personality |
| Pixel map, routes, regions and desk handoffs | CITP listen address and HTTP interface address |
| Audio gain, beat sensitivity and EQ | Media library directory |
| Clip-switch limit, In/Out frame rate and server time | |

**Revert to current settings** cancels pending changes. Choosing **Change folder…** restarts Pixel
automatically. Playback and layer changes need no restart.

**Light and Media are on this computer** moves DMX and Speed Group listeners immediately; CITP
and HTTP follow after restart. Verify each configured output's screen, audio, patch and preview
identity, especially when running several outputs on one GPU.
