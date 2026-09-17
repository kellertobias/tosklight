# 3D Model Mapping

A layer can be drawn onto a 3D model instead of as a flat picture. The layer's complete look — its content, effects, tint, greyscale, mask, and dimmer — is wrapped onto the model through the model's texture coordinates, and the model is placed and turned on the output with the layer's own position, scale, and rotation plus two extra channels, **Model pan** and **Model tilt**.

## The model library

Models live in numbered slots **1–255** in the Library's **Models** tab. A slot keeps its number across restarts and edits, so a Cue that maps a layer onto model 12 keeps doing so.

### Built-in models

Five test models ship with every Media Server and need no import:

| Slot on a new server | Model | Shape and image |
| --- | --- | --- |
| 1 | **Plane** | A flat screen facing the output, always in the output's aspect ratio, with the layer's flat picture on it. The default. |
| 2 | **Cube** | The whole image on each of its six faces. |
| 3 | **Sphere** | The image wrapped once around it, its top edge at the top pole. |
| 4 | **Cylinder** | As tall as it is wide; the image wrapped once around its side, the whole image on each end. |
| 5 | **Pyramid** | A square base; the image as a triangle on each side, the whole image on the base. |

To put a built-in model in any slot, select the slot and press its button under **Built-in model**. The choice is saved with the Media Server's configuration and takes effect on the output at once; the slot shows **Built-in** and the model's name, which you can change with **Save name**. Choosing a built-in model for a slot that held an imported model deletes that model's file. **Clear slot** empties a built-in slot too; press the model's button to put it back.

An existing installation receives the built-in models the first time it starts with this version, each in its slot above unless an imported model already occupies it. Imported models never move.

### The Plane has the output's shape

The Plane is never square: it always takes the aspect ratio of the output it is drawn on — 16:9 on a 1920×1080 output, 4:3 on a 1024×768 output — whichever slot it is in and when it stands in for a missing model. At Pan 0°, Tilt 0°, and scale 1 it covers the whole output, and the layer's picture sits on it exactly as the flat layer would draw, **scaling mode included**. A clip on the Plane therefore looks the same as the clip drawn Flat until you turn it; parts of the output the picture leaves empty (for example Fit bars) stay transparent on the Plane. When the output's resolution changes, the Plane takes the new shape on the next frame. Nothing is stored for this: existing slots and cues keep working unchanged.

### Importing a model

1. Open **Library > Models** and select a slot.
2. Drop a `.glb` file on **Model file**, or open the picker. The upload shows its progress, then **Importing…** while the server reads the model.
3. When the import succeeds the slot shows the model's name (taken from the file name), its vertex count, and its triangle count. When it fails, the slot stays as it was and the reason is shown under the upload field.

Uploading onto a slot holding an imported model replaces it and keeps its name; uploading onto a built-in slot names the slot after the file. **Save name** renames the slot; **Clear slot** empties it and deletes the stored file. Models are stored with the Media Server's library (in the hidden `.models` folder of the library root) and their slot assignments with its configuration.

### Import requirements

- **glTF 2.0 Binary (`.glb`)**, self-contained: buffers must be embedded, not external files. Export "glTF Binary" from Blender or your 3D tool.
- **Texture coordinates (`TEXCOORD_0`) on every mesh.** They decide where the layer image lands on the model. A mesh without them is refused with a message naming it; UV-unwrap it and export again.
- Triangle meshes (triangle lists, strips, or fans). Points, lines, sparse accessors, and Draco-compressed meshes are refused.
- Normals are read when present and computed when not. Materials and embedded textures are ignored — the layer is the texture — and the model is drawn unlit.
- At most 256 MiB and four million vertices.

Every node transform in the default scene is applied, then the whole model is **centred on its bounding box and scaled to fit a sphere of radius 1**. A 2 cm prop and a 40 m set piece therefore frame the same way; use the layer's scale to size the result.

## Selecting a model on a layer

The layer's **3D model** channel (slot 34 of the current layer layout) selects the model. **0 is Flat.** Values **1–255** select that model slot. **Model pan** and **Model tilt** are 16-bit channels covering −360° to 360°. In the Media Server's Media pane, the same three values are **Model**, **Pan**, and **Tilt** under the **3D model** heading on the layer's **Frame** tab. On the desk they are the Media attributes **Model Pan** and **Model Tilt**, beside **3D Model** on Media encoder page 9. They are separate from a moving light's Pan and Tilt, so Aim, position presets, and the Pan/Tilt tools never turn a media layer.

**Flat is the default.** A new layer is Flat at Pan 0° and Tilt 0°, and draws exactly like a layer without 3D mapping. Pan and Tilt still turn a Flat layer: it then draws as a flat card in 3D, which starts at the same size, position, and scaling mode as the flat layer and turns from there. Setting both back to 0° returns it to the plain flat drawing.

**A missing model falls back to the Plane.** When the selected slot is empty, or its file cannot be loaded, the layer is mapped onto the built-in **Plane** — never onto another model, and never drawn black. The server logs the problem once, the Models tab marks an unloadable slot **Cannot load** with the reason, and the layer's status reports `missing` or `unloadable`.

## Camera and placement

Every output looks at its models through the same fixed camera: a perspective view with a **40° vertical field of view**, looking straight at the centre of the output. A model at scale 1 fills roughly the output height.

- **Position X / Y** move the model across the output in the same units as a flat layer: ±1 puts its centre on the left/right or top/bottom edge.
- **Scale X / Y** scale the model along its own width and height; its depth follows their average. On the Plane they scale the output-shaped screen, so Scale X 0.5 makes it half the output width. Scaling mode has no effect on an imported or other built-in model, because the image follows the model's texture coordinates. A turned Flat layer and the Plane keep the layer's scaling mode.
- **Rotation** is the model's **roll**.

### Rotation order: pan, then tilt, then roll

The three rotations always apply in this order, like a moving head:

1. **Pan** turns the model around the vertical axis. Positive pan turns its front to the right.
2. **Tilt** then tips the model around its own horizontal axis, as already panned. Positive tilt tips its front up.
3. **Roll** finally spins the model around its own facing axis. Positive roll turns clockwise as seen on the output.

At pan 0 and tilt 0 the model faces the camera, so roll looks exactly like the flat layer's Rotation. A flat screen-shaped model at pan 90° is seen edge-on and almost disappears. Both sides of every surface are drawn, so a flat model stays visible from behind.

## What still applies

The mapped image is composited like any other layer. **Blend mode**, **Strobe**, the layer **Dimmer**, and the **Opacity Cycle** effect behave exactly as they do on a flat layer, and layer order is unchanged.
