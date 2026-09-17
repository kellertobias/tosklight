# Media Server Visualizer Parameters

Every ToskLight Media Server layer has four dedicated **Visualizer Parameter** channels. DMX input,
the Media Server's web Media pane, and ToskLight Control's Media pane all address the same four
bytes. The effect banks are separate: **FX1** and **FX2** stay ordinary effect slots whether a
layer shows a visualizer or ordinary media.

## DMX channels

The channels are slots 52–55 of each 59-slot layer block (zero-based offsets 51–54), in both the
2-layer (158 slots) and 8-layer (512 slots) personality. The desk attributes are
`media.visualizer.parameter.1` to `media.visualizer.parameter.4`.

| Raw value | Meaning |
| --- | --- |
| 0 | Keep the default: the layer's own tuning of the shown visualizer, else its configured value |
| 1–255 | Sweep the parameter from the minimum to the maximum the shown visualizer defines |

The visualizer the layer shows defines each channel's name, range, and default. Channel *n* is the
visualizer's *n*th parameter in the order the Visualizers editor lists them; a visualizer with fewer
than four parameters leaves the rest inert. A layer showing library media, text, or nothing ignores
all four channels.

How a byte becomes a value:

| Parameter kind | 1–255 selects |
| --- | --- |
| Number (Size, Speed, Amount, …) | `minimum + (raw − 1) / 254 × (maximum − minimum)` |
| Count, Iterations | The same, rounded to a whole number |
| Colour, Second colour | A fully saturated hue, 0°–360° |
| Mirror, Filled, Wireframe | Off below 128, on from 128 |
| Variant | `raw − 1` |

Some visualizers name or range a channel their own way. Flying Props, for example, names Variant
**Flight pattern** (bytes 1, 2, 3 and 4 select Fly-through, Drift, Orbit and Rise, and byte 5
starts the list again), names Count
**Density** with a range of 1–48, and narrows Size to 0.02–0.5:

| Channel | Flying Props parameter | Byte 1 | Byte 255 |
| --- | --- | --- | --- |
| 1 | Flight pattern | 0 (Fly-through) | 254 (pattern 254 mod 4 = Orbit) |
| 2 | Speed | 0 | 8 |
| 3 | Density | 1 | 48 |
| 4 | Size | 0.02 | 0.5 |

## Media Server HTTP

`GET /api/v2/outputs` reports, for every layer:

- `visualizerControls` — the four raw bytes;
- `visualizerChannels` — the channels the shown visualizer defines, each with `index` (0–3),
  `parameter`, `label`, `minimum`, `maximum`, `step`, `defaultValue`, the current `raw` byte, and the
  `value` in effect. The list is empty for ordinary media;
- `visualizerParameters` — the layer's own tuning of the shown visualizer, or `null`.

`GET /api/v2/visualizers` reports the same `channels` for each configured visualizer, with its
configured defaults.

`POST /api/v2/outputs/{id}/layers/{layer}/update` accepts:

- `visualizerParameterIndex` (0–3) with `visualizerParameterValue` (0–255) — set one byte, exactly
  as DMX would;
- `visualizerParameters` — replace the layer's tuning of the visualizer it shows. The layer must be
  showing a visualizer (`visualizer-controls-source` otherwise). No `effectSlot` is involved;
- `resetVisualizerParameters: true` — drop the layer's tuning.

A tuning belongs to the address it was made for and applies only while the layer shows that
address.

## ToskLight Control

The desk reads each layer's `visualizerChannels` through its native Media snapshot and names the
Media pane's Visualizer faders after them. It reloads them when the layer's folder or file changes.
