# 3D Model Mapping

A layer can map its complete picture onto a model instead of displaying it flat. The layer's
content, tint, effects, mask and dimmer follow the model's UV coordinates.

## Choose or import a model

Open **Library → Models**. Slots **1–255** keep stable numbers for Cue recall. A new server includes
Plane, Cube, Sphere, Cylinder and Pyramid in slots 1–5. Existing imported slots are not replaced.
Clear a built-in slot before replacing it; choosing a built-in for an imported slot deletes its
stored model file.

1. Select a slot and upload a `.glb` through **Model file**.
2. Wait for import and check its thumbnail, name and mesh counts.
3. Use **Save name** to rename an imported slot or **Clear slot** to remove it.

A failed import leaves the previous slot intact and explains why. Replacing an imported model keeps
its slot name. The thumbnail's orange checker quarter marks where the picture's top-left lands.

### Prepare the file

Export self-contained **glTF 2.0 Binary (.glb)** with UV coordinates on every mesh. UV-unwrap a mesh
that is refused for missing coordinates. Use triangle meshes without Draco compression; maximum
256 MiB and four million vertices. Materials/textures are ignored because the layer supplies the
picture. The imported object is centred and normalized; use layer scale to size it on the output.

## Put it on a layer

On **Frame → 3D model**, choose **Model**, **Pan** and **Tilt**. On Control these are Media attributes
**3D Model**, **Model Pan** and **Model Tilt**, separate from lighting Pan/Tilt.

- **0 / Flat** is the default. Pan/Tilt can turn that flat card; return both to zero for the usual
  flat picture.
- **1–255** selects the library model slot. A missing/unloadable model uses Plane and reports why.
- **Plane** matches the output's aspect ratio and scaling mode; scale 1 covers the output at zero
  pan/tilt. Other models use their own UV mapping rather than the layer's scaling mode.

## Position and rotate

**Position X/Y** moves across the output; ±1 reaches an edge. **Scale X/Y** changes width/height,
with depth following their average. The fixed camera has a 40° vertical field; scale 1 roughly
fills the output height.

Rotations apply as **Pan**, then **Tilt**, then **Rotation (roll)**. Positive pan faces right,
positive tilt faces up, and positive roll is clockwise on the output. A plane at pan 90° is
edge-on. Both surface sides are visible.

Layer order, blend, strobe, dimmer and Opacity Cycle still apply to the mapped picture.
