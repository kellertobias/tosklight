# Operate layers and the master

Use **Media** to select content and adjust the selected output's layers. **Master** controls their combined picture. For show playback, [operate Pixel from Control](09-connect-the-desk.md).

![Pixel Playback with Master and layer controls](../assets/screenshots/workflows/pixel-playback.png)

## Take control

Choose the output in the dock and enable **Take over playback** before changing its layers in Pixel. The browser then overrides incoming DMX for that output. Release takeover to return playback to the desk.

## Preview library items on the output

To audition content on the physical output, turn on **Enable preview** below **Take over playback** in the dock while the Library is open. Pixel takes over playback of the output chosen in the dock, puts every layer out and brings the master full up. Click a media item to loop it on **Layer 1** at full level, with every other layer out. The output keeps its current display, pixel map and effect settings. Clicking another item switches immediately. Shift- and Command/Ctrl-clicks only change the selection. Preview stays enabled while you move between Library, **Visualizers**, **Text**, **Effects** and **Models**. Each page previews its selected item:

- Visualizers and Text play their selected slot on Layer 1.
- Effects applies the selected preset at full strength in Layer 1's first bank, over the last previewed content.
- Models maps the last previewed content onto the selected model.

The next preview removes the preceding preview's effect or model. Saved editor changes appear immediately, including visualizer settings and text edits.
 Turning **Enable preview** off or opening another page restores the layers' slots, levels, play modes, banks and models, and the Master level. Playback returns to the desk unless takeover was already enabled. Preview is never saved: it changes nothing in the library and is forgotten on reload.

![Layer preview, composite, and physical output](../assets/diagrams/pixel-layer-flow.svg)

## Choose content

A layer selects one addressed content folder/file and an optional mask folder/file. Content `0` or `255` is blank. The isolated layer preview proves that layer's source and transformations; the composite preview proves the result after layer order, blending, effects, and the Master are applied.

While a new clip loads, the layer keeps playing the preceding clip and shows **Loading**. It switches when the new clip's first frame is ready.

Set the hold limit in **Settings > Libraries > Clip switch** using the number field, step buttons or touch keypad. The default is 500 ms; `0` disables the hold. Values above 10000 are refused. The saved limit applies to the next content change without restart.

If loading exceeds that limit, the layer becomes transparent until the new clip is ready. Blank content, disabled/deleted files and load failures make it transparent immediately.

## Transform and shape a layer

Layer controls include position, scale, rotation, opacity, crop or shaper behavior, blending, color treatment, and typed effects supported by the active personality. Edit one control at a time while watching both the isolated and composite previews. A correct isolated layer can still disappear in the composite because of its order, blend, opacity, mask, Master, or another layer.

### Layer control tabs

The Media pane groups a layer's controls into tabs:

- **Playback** - Play mode, Dimmer, Volume, Speed, and Playback BPM, followed by the **Playback range** heading. It holds the read-only **Clip length** of the clip the layer shows, with **✕ Clear playback range** at its right, then **In point** and **Out point**. One press of **✕** sets In and Out both back to 0, so the whole clip plays again. It is greyed out while the range is already the whole clip. The points are shown and typed as `mm:ss.ff` at the frame rate in **Settings > Libraries > In and Out points** (see [Playback range](#playback-range)). The range is shown on this tab only.
- **Frame** - Scale, Scaling mode, Position, and Rotation, followed by the **3D model** heading with **Model**, **Pan**, and **Tilt**. **Model** lists **Flat** and every slot in **Library › Models**. A new layer is **Flat** at Pan 0° and Tilt 0°, and Pan and Tilt still turn a Flat layer.
- **Colour**, **Mask**, and **Blend** - tint and greyscale, the mask placement, and blend mode with strobe. From the Desk, the Color special dialog's colour is the tint and its **White Blend** is the greyscale; the layer's Dimmer and the Master stay separate.
- **Effects** - the two effect banks. When the layer shows a generated visualizer, a **Visualizer** tab comes first with that visualizer's own settings for this layer and its four Visualizer Parameter channels, each named by the visualizer; a channel the visualizer does not use reads **Unused** and is disabled. The Visualizer tab never changes **Bank 1** or **Bank 2**. Other content shows only **Bank 1** and **Bank 2**.

## Master output

Check Master as well as the selected layer when the composite stays black. Its dimmer, playback state and control ownership affect the whole output.

Release **Take over playback** to return the output to the next valid Art-Net or sACN frame.

## Playback range

**In point** and **Out point** are 16-bit frame counts. The In point counts frames from the clip's
start and the Out point counts frames back from its end, so **0** on both plays the whole clip. Every
play mode stays inside the range: a loop wraps from the Out point back to the In point, a bounce turns
at both, and a single pass ends on the Out point. An In point beyond the clip holds its last frame,
and an Out point that would end the range before the In point plays through to the clip's end. Changing the range while a clip plays
keeps the playhead where it is when it is still inside the range. Stills, text, and visualizers
ignore the range.

The points count frames at the server-wide **Frame rate** in **Settings > Libraries > In and Out
points**, 25 fps by default and 1-120 fps. The rate turns a count into a time in the clip: at 25 fps
an In point of 250 starts 10 seconds in, whatever the clip's own frame rate, and a clip recorded at
that rate starts exactly on frame 250. The setting saves automatically and applies immediately, including to playing layers. Control and Pixel show and accept points as `mm:ss.ff`: the In point from the clip's start, the Out point before
its end, with **End of clip** for an Out point of zero. Touch the value to type a time; there is no
0-65535 fader.

## Mirror the output

Set **Master scale X** or **Master scale Y** below zero to mirror the finished output along that axis. Both controls cover −4× to +4×. Use these scale controls for Master mirroring.
