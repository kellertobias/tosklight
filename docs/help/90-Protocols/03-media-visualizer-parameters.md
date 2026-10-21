# Media Server Visualizer Parameters

Pixel’s generated visualizers expose up to four controls, such as Size, Speed, Density or Colour.
Select a visualizer, then use its named controls in Pixel or Control’s Media pane. Changing the
selected visualizer changes the available labels and ranges. Unused controls have no effect.

These controls are separate from **FX1** and **FX2**, which remain effect slots for both generated
visuals and ordinary media.

When controlling Pixel by DMX, the four channels occupy slots 52–55 of each 59-slot layer block.
Value `0` uses the visualizer’s saved tuning or default. Values `1–255` sweep its stated range.
For a switch, values below `128` mean off and values from `128` mean on. Ordinary media ignores
these channels.

For parameter mappings and custom integrations, see the [Visualizer parameter reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/03-media-visualizer-parameters.md).
