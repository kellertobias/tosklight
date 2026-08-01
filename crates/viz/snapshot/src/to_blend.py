"""Assemble a ToskLight rig snapshot into a Blender file.

Run by the visualizer, not by hand:

    blender --background --factory-startup --python to_blend.py -- <snapshot folder> <output.blend>

The snapshot folder holds ``rig.glb`` — every fixture body, lamp face, truss and piece of scenery
where this frame put them — and ``snapshot.json``, which carries what triangles cannot say: which
lamp is which, where its light goes, how wide the cone is, how hard its rim is, and how bright it
was at the moment the operator pressed the key.

What this builds is a starting point a designer finishes, not a finished render. The geometry, the
placement, the aim and the colour are the desk's and are exact. The haze density, the exposure and
the sample count are a look: they are gathered at the top of this file so they can be changed in
one place, and everything downstream of them is ordinary Blender data that can be edited by hand.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector

# --- The look -----------------------------------------------------------------------------------

# Haze at 100% in the visualizer, as a Principled Volume density. Beams are only visible in air
# that has something in it, so this is what makes an exported rig look like the picture rather
# than like a set of lamps in a vacuum.
HAZE_DENSITY_AT_FULL = 0.06
# Haze scatters forwards, which is why a beam pointed at the camera flares and the same beam seen
# from the side does not.
HAZE_ANISOTROPY = 0.35
# Metres of clear air kept around the rig so a beam does not leave the haze before it lands.
HAZE_MARGIN = 6.0
# Every lamp's power is multiplied by this. One number to make the whole rig hotter or cooler.
POWER_SCALE = 1.0
SAMPLES = 256
RESOLUTION = (1920, 1080)


def fail(message):
    """Stop with something the visualizer can put in front of the operator."""
    print("Error: " + message, file=sys.stderr)
    sys.exit(1)


def arguments():
    if "--" not in sys.argv:
        fail("no snapshot folder was given")
    rest = sys.argv[sys.argv.index("--") + 1:]
    if len(rest) < 2:
        fail("expected a snapshot folder and an output path")
    return rest[0], rest[1]


def read_snapshot(folder):
    document = os.path.join(folder, "snapshot.json")
    if not os.path.isfile(document):
        fail("no snapshot.json in " + folder)
    with open(document, "r", encoding="utf-8") as handle:
        snapshot = json.load(handle)
    if snapshot.get("format") != 1:
        fail("snapshot format {} is not understood by this script".format(snapshot.get("format")))
    geometry = os.path.join(folder, snapshot.get("geometry_file", "rig.glb"))
    if not os.path.isfile(geometry):
        fail("no geometry file beside the snapshot")
    return snapshot, geometry


def import_geometry(path):
    """Import the rig. The glTF importer turns its Y-up scene into Blender's Z-up one."""
    if hasattr(bpy.ops.import_scene, "gltf"):
        bpy.ops.import_scene.gltf(filepath=path)
    else:  # Blender renamed the operator; keep working across versions.
        bpy.ops.wm.gltf_import(filepath=path)


def collection(name):
    made = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(made)
    return made


def add_lights(snapshot):
    """One spot per emitting head, aimed where the desk aimed it."""
    lights = collection("Rig Lights")
    for entry in snapshot.get("lights", []):
        data = bpy.data.lights.new(name=entry["name"], type="SPOT")
        data.color = tuple(entry["colour"])
        data.energy = entry["power_watts"] * POWER_SCALE
        # A cone angle is the whole cone; the snapshot carries it that way already.
        data.spot_size = math.radians(entry["cone_degrees"])
        # Blender's blend is the fraction of the cone that is the soft edge, which is exactly what
        # the desk means by a rim: 0 cuts like a profile at focus, 1 fades away like a wash.
        data.spot_blend = min(max(entry["blend"], 0.0), 1.0)
        # A lamp is not a point. Giving the source its real radius is what makes the shadow of a
        # truss soften with distance instead of staying razor sharp everywhere.
        data.shadow_soft_size = max(entry["radius"], 0.001)

        light = bpy.data.objects.new(name=entry["name"], object_data=data)
        light.location = Vector(entry["position"])
        # A spot shines down its own -Z.
        direction = Vector(entry["direction"])
        if direction.length > 1e-6:
            light.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
        light["tosklight_fixture"] = entry.get("fixture", "")
        if entry.get("address"):
            light["tosklight_address"] = entry["address"]
        if entry.get("fixture_number") is not None:
            light["tosklight_number"] = entry["fixture_number"]
        lights.objects.link(light)
    return lights


def add_haze(snapshot):
    """A box of hazed air over the rig, which is what a beam is visible in."""
    fog = float(snapshot.get("look", {}).get("fog", 0.0))
    if fog <= 0.0:
        return None
    bounds = snapshot["bounds"]
    low = Vector(bounds["min"]) - Vector((HAZE_MARGIN, HAZE_MARGIN, 0.5))
    high = Vector(bounds["max"]) + Vector((HAZE_MARGIN, HAZE_MARGIN, HAZE_MARGIN))
    centre = (low + high) * 0.5
    size = high - low

    bpy.ops.mesh.primitive_cube_add(size=1.0, location=centre)
    box = bpy.context.active_object
    box.name = "Atmosphere"
    box.scale = size
    # The box is the air, not an object in the picture: it must not appear as a surface, cast a
    # shadow, or be picked up as something to walk around.
    box.display_type = "WIRE"
    box.visible_shadow = False

    material = bpy.data.materials.new("Haze")
    if bpy.app.version < (6, 0):
        material.use_nodes = True
    material.node_tree.nodes.clear()
    volume = material.node_tree.nodes.new("ShaderNodeVolumePrincipled")
    volume.inputs["Density"].default_value = fog * HAZE_DENSITY_AT_FULL
    volume.inputs["Anisotropy"].default_value = HAZE_ANISOTROPY
    output = material.node_tree.nodes.new("ShaderNodeOutputMaterial")
    material.node_tree.links.new(volume.outputs["Volume"], output.inputs["Volume"])
    box.data.materials.append(material)

    # Keep it out of the rig collection so a designer can switch the air off in one click.
    for existing in list(box.users_collection):
        existing.objects.unlink(box)
    collection("Atmosphere").objects.link(box)
    return box


def add_camera(snapshot):
    """The camera the operator was looking through."""
    view = snapshot.get("camera")
    if not view:
        return None
    data = bpy.data.cameras.new("Viz Camera")
    if view.get("orthographic"):
        data.type = "ORTHO"
        data.ortho_scale = max(float(view.get("orthographic_size", 8.0)) * 2.0, 0.01)
    else:
        data.lens_unit = "FOV"
        data.angle_y = math.radians(float(view.get("fov_degrees", 45.0)))
    # A rig is metres across and lamps are centimetres deep; the default near clip hides both ends.
    data.clip_start = 0.05
    data.clip_end = 5000.0

    camera = bpy.data.objects.new("Viz Camera", data)
    camera.location = Vector(view["position"])
    aim = Vector(view["target"]) - Vector(view["position"])
    if aim.length > 1e-6:
        camera.rotation_euler = aim.to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.collection.objects.link(camera)
    bpy.context.scene.camera = camera
    return camera


def set_up_render(snapshot):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.render.resolution_x, scene.render.resolution_y = RESOLUTION
    scene.render.resolution_percentage = 100
    if hasattr(scene, "cycles"):
        scene.cycles.samples = SAMPLES
        # Beams are made of the volume, so it is worth marching properly through it.
        # Haze only scatters light if the renderer is allowed to scatter in it at all, and
        # Blender ships with that turned off.
        scene.cycles.volume_bounces = 2
        scene.cycles.volume_step_rate = 0.5
        scene.cycles.volume_max_steps = 1024
        scene.cycles.transparent_max_bounces = 16

    # A lighting rig is judged in a dark room: the world contributes nothing and every bit of
    # light in the picture comes from a lamp.
    world = bpy.data.worlds.new("Blackout")
    if bpy.app.version < (6, 0):
        world.use_nodes = True
    background = world.node_tree.nodes.get("Background")
    if background:
        background.inputs["Color"].default_value = (0.0, 0.0, 0.0, 1.0)
        background.inputs["Strength"].default_value = 0.0
    scene.world = world

    look = snapshot.get("look", {})
    exposure = float(look.get("exposure", 1.0))
    if exposure > 0.0:
        # The desk's trim is a multiplier; Blender's exposure is in stops.
        scene.view_settings.exposure = math.log2(exposure)
    for transform in ("AgX", "Filmic", "Standard"):
        try:
            scene.view_settings.view_transform = transform
            break
        except TypeError:
            continue


def main():
    folder, destination = arguments()
    snapshot, geometry = read_snapshot(folder)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.unit_settings.system = "METRIC"
    bpy.context.scene.unit_settings.length_unit = "METERS"

    import_geometry(geometry)
    lights = add_lights(snapshot)
    add_haze(snapshot)
    add_camera(snapshot)
    set_up_render(snapshot)

    directory = os.path.dirname(os.path.abspath(destination))
    if directory and not os.path.isdir(directory):
        os.makedirs(directory, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=destination)
    print(
        "Wrote {} — {} lights, {} objects".format(
            destination,
            len(lights.objects),
            len(bpy.context.scene.collection.all_objects),
        )
    )


main()
