"""Private 4K image entrypoint; the existing .blend export entrypoint remains unchanged."""
import math
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from parent_guard import install_parent_guard
install_parent_guard()
import bpy
import to_blend as frozen

arguments = sys.argv[sys.argv.index("--") + 1:]
if len(arguments) == 3 and arguments[0] in ("image", "legacy"):
    mode, folder, destination = arguments
elif len(arguments) == 2:
    mode, (folder, destination) = "legacy", arguments
else:
    raise RuntimeError("Expected image/legacy, frozen package and output PNG")
if mode == "image":
    import image_scene
    import image_media
    import image_optics
    import image_fog
    snapshot, geometry = image_scene.read_manifest(folder)
else:
    snapshot, geometry = frozen.read_snapshot(folder)
print("TOSKLIGHT_PROGRESS importing frozen geometry", flush=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.context.scene.unit_settings.system = "METRIC"
bpy.context.scene.unit_settings.length_unit = "METERS"
frozen.import_geometry(geometry)
frozen.set_up_render(snapshot)
if mode == "image":
    image_scene.configure(snapshot)
    image_media.add_panels(snapshot, folder)
    image_media.add_projectors(snapshot)
    image_optics.add_optics(snapshot, folder)
    image_fog.add_fog(snapshot, folder)
else:
    frozen.add_lights(snapshot)
    frozen.add_haze(snapshot)
    frozen.add_camera(snapshot)
scene = bpy.context.scene
scene.render.resolution_x = 3840
scene.render.resolution_y = 2160
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.render.use_file_extension = False
scene.render.filepath = destination
# The frozen camera stores vertical FOV/half-height. Keep its lens and derive 16:9 framing,
# rather than stretching an old raster or treating orthographic half-height as landscape width.
view = snapshot.get("camera", {})
if scene.camera is None:
    raise RuntimeError("Frozen snapshot has no camera")
if mode == "legacy" and view.get("orthographic"):
    scene.camera.data.ortho_scale = max(float(view.get("orthographic_size", 8.0)) * 2.0 * 16.0 / 9.0, 0.01)
elif mode == "legacy":
    scene.camera.data.sensor_fit = "VERTICAL"
    scene.camera.data.angle_y = math.radians(float(view.get("fov_degrees", 45.0)))
# No simulation or live media is queried. Native optics/media that cannot transfer are refused
# before this process starts, rather than silently approximated here.
scene.frame_set(1)
print("TOSKLIGHT_PROGRESS rendering 3840x2160 Cycles PNG", flush=True)
bpy.ops.render.render(write_still=True)
if not os.path.isfile(destination):
    raise RuntimeError("Blender completed without producing a PNG")
print("TOSKLIGHT_PROGRESS image written", flush=True)
