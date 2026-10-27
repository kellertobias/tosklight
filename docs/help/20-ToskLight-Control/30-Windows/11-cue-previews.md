# Cue previews

Stage previews are stored pictures of Cues. Each one is drawn once (when the Cue is recorded, when it is edited, when a fixture moves on the Stage, or when a Group it uses changes) and is then stored in the show alongside the Cue. Opening a Cuelist shows those stored pictures. Because they travel inside the show file, a Cuelist opened on another desk arrives with its previews already drawn, and a desk without 3D rendering still shows them. A Cue recorded from a desk that cannot draw has no preview until a desk that can opens that Cuelist.

A Cue that contains only ToskLight Pixel content is previewed by that Media Server instead of the Stage. The desk works out the exact DMX state the Cue leaves on the server, including values tracked from earlier Cues and Group values, and the server draws that state off-screen without changing what its output currently shows:

- A Cue that addresses the server's master, or more than one of its layers, shows the composited **Program** image of the patched output, with its media, effects, tint, and master processing.
- A Cue that addresses exactly one layer shows only that layer's composited picture. Transparent areas stay transparent and appear over a checkerboard; neither the whole Program nor the library file's thumbnail is shown.

Each picture belongs to the addressed server, output, and layer. In a show with several Media Servers, a Cue for one server never shows another server's image. A Cue that mixes lighting fixtures with media, or addresses two servers, keeps its Stage preview. Media previews are not stored in the show, because they depend on the server's own library; they are asked for again whenever the Cue content changes.

When there is no picture, the Preview cell says why instead of showing a broken or older image:

| Cell | Meaning |
| --- | --- |
| **Loading media preview** | The server is still loading the selected media. The desk asks again shortly. |
| **Empty media** | The picture is valid but shows nothing: no media is selected, or the layer or master is at zero. |
| **Media output missing** | The server no longer has the patched output, its personality does not match the patch, or it cannot draw Cue previews. Update ToskLight Pixel or patch the output again. |
| **Media Server offline · Retry** | The server did not answer. Touch the cell to ask again. The desk also asks again when the server is reported online. |
