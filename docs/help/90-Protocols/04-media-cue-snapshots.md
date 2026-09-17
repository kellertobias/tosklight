# Media Server Cue Snapshots

A ToskLight Media Server can draw a picture of a DMX state it is *not* currently playing. ToskLight
Control uses this to preview media-only Cues: it sends the slots a Cue would transmit, and the
server returns the picture that state produces. The live output is not changed, and Art-Net or
sACN control is not interrupted.

## Request

`POST http://<server>:8080/api/v2/outputs/<output id>/snapshot` with a JSON body:

| Field | Meaning |
| --- | --- |
| `slots` | The output's complete personality footprint, one number 0–255 per slot: 158 slots for 2 layers, 512 for 8 layers. Layers come first, then the 40-slot master, exactly as on the wire. |
| `layer` | Optional zero-based layer. Omit it (or send `null`) for the whole Program. |
| `width`, `height` | Optional box the picture is fitted into, 1–1024 pixels. The default is 320 × 180. The picture keeps the output's aspect ratio. |

The slots are decoded by the same personality decoder and state rules that Art-Net and sACN use,
applied to a private copy of the output. Settings that live only on the Media Server stay as the
output has them: effect presets, visualizer tuning, text, 3D models, and the pixel-map region.

## Response

A successful request returns `image/png`:

- **Program** (no `layer`): the opaque composited output, including the master, its tint, mask,
  shaper, and effect banks.
- **Layer**: only that layer, with its effects and transparency. Pixels outside the layer are
  transparent. Other layers and the master are not drawn.

Response headers:

| Header | Meaning |
| --- | --- |
| `x-tosklight-snapshot-content` | `content`, or `empty` when nothing in scope draws: no media selected, layer dimmer at 0, or (for the Program) master dimmer at 0 |
| `x-tosklight-snapshot-cache` | `hit` when the picture was remembered, `miss` when it was drawn now |
| `x-tosklight-preview-width`, `x-tosklight-preview-height` | The picture's size |
| `ETag` | The identity of this picture |

The server remembers the most recent 64 pictures. Each is keyed by the output, layer, slots, and
size, together with the library revision and the server configuration. A changed library or
configuration therefore draws a fresh picture. Failures are never remembered.

The server waits up to four seconds for selected clips, masks, and generated sources to become
drawable. A clip is shown at its first frame from the In point.

## Errors

Errors return JSON `{ "code": "…", "message": "…" }`:

| Status | `code` | Meaning |
| --- | --- | --- |
| 400 | `malformed-output-id` | The output id is not a UUID |
| 400 | `snapshot-slots-mismatch` | `slots` does not match the output's personality footprint |
| 400 | `snapshot-slot-range` | A slot is outside 0–255 |
| 400 | `snapshot-size` | `width` or `height` is outside 1–1024 |
| 404 | `unknown-output` | The server has no such output |
| 404 | `layer-not-found` | The output's personality has no such layer |
| 503 | `snapshot-not-ready` | The selected media did not finish loading in time. Ask again. |
| 503 | `snapshot-unavailable` | This process cannot render, for example because it has no graphics device |

A Media Server older than this interface answers `404` without a `code`. ToskLight Control shows
such a Cue as **Media output missing** and asks the operator to update ToskLight Media.
