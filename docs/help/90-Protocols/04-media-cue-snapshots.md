# Media Server Cue Previews

Control can show a preview of a media-only Cue without playing it on Pixel’s live output.
The preview uses the Cue’s programmed DMX values and Pixel’s current content library and settings.

If the preview is empty, check the Cue’s media selection, layer dimmer and master dimmer. If it is
unavailable, check Pixel’s connection and wait for content to finish loading. **Media output missing**
can also mean Pixel is too old to support Cue previews; update it to match Control.

Previewing a Cue does not interrupt Art-Net or sACN. Recheck the actual output during rehearsal:
a thumbnail cannot prove physical display routing, alignment or sound.

The [Cue snapshot integration reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/04-media-cue-snapshots.md) describes the interface for custom clients.
