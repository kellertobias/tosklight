"""Frozen camera/world transfer for the image backend. No live scene or network is consulted."""
import json
import math
import os
import bpy
from mathutils import Matrix


def checked_asset(folder, name):
    if not isinstance(name, str) or not name or name.startswith(".") or "/" in name or "\\" in name:
        raise RuntimeError("Frozen image assets require package-local filenames")
    root = os.path.realpath(folder)
    path = os.path.realpath(os.path.join(root, name))
    if os.path.commonpath((root, path)) != root or not os.path.isfile(path):
        raise RuntimeError("Frozen image asset escaped its package: " + name)
    return path


def read_manifest(folder):
    with open(checked_asset(folder, "image-manifest.json"), "rb") as handle:
        encoded = handle.read(16 * 1024 * 1024 + 1)
    if len(encoded) > 16 * 1024 * 1024:
        raise RuntimeError("Frozen image manifest exceeds 16 MiB")
    manifest = json.loads(encoded)
    if manifest.get("format") != 1 or manifest.get("unsupported"):
        raise RuntimeError("Frozen image manifest is unsupported")
    geometry = checked_asset(folder, manifest["geometry_file"])
    for asset in manifest["media"]["assets"]:
        checked_asset(folder, asset["file"])
    known = {asset["file"] for asset in manifest["media"]["assets"]}
    for panel in manifest["media"]["panels"]:
        if panel["selected_file"] is not None and panel["selected_file"] not in known:
            raise RuntimeError("Media panel references pixels absent from the frozen package")
    return manifest, geometry


def add_camera(manifest):
    captured = manifest["camera"]
    data = bpy.data.cameras.new("Frozen invocation camera")
    data.clip_start = max(float(captured["near"]), 0.000001)
    data.clip_end = max(float(captured["far"]), data.clip_start + 0.01)
    matrix = Matrix(captured["world_from_camera"]).transposed()
    if captured["orthographic"]:
        data.type = "ORTHO"
        data.sensor_fit = "HORIZONTAL"
        data.ortho_scale = float(captured["vertical_half_height"]) * 2.0 * 16.0 / 9.0
        matrix = matrix @ Matrix.Translation((0.0, 0.0, captured["orthographic_clip_shift"]))
    else:
        data.sensor_fit = "VERTICAL"
        data.lens_unit = "FOV"
        data.angle_y = math.radians(float(captured["vertical_fov_degrees"]))
    camera = bpy.data.objects.new("Frozen invocation camera", data)
    camera.matrix_world = matrix
    camera["tosklight_native_camera_columns"] = json.dumps(captured["world_from_camera"])
    bpy.context.scene.collection.objects.link(camera)
    bpy.context.scene.camera = camera


def add_world(manifest):
    look = manifest["look"]
    world = bpy.data.worlds.new("Frozen background and ambient")
    if bpy.app.version < (6, 0):
        world.use_nodes = True
    tree = world.node_tree
    tree.nodes.clear()
    light_path = tree.nodes.new("ShaderNodeLightPath")
    ambient = tree.nodes.new("ShaderNodeBackground")
    ambient.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    ambient.inputs["Strength"].default_value = max(float(look["ambient"]), 0.0)
    background = tree.nodes.new("ShaderNodeBackground")
    background.inputs["Color"].default_value = tuple(look["background"]) + (1.0,)
    background.inputs["Strength"].default_value = 1.0
    mix = tree.nodes.new("ShaderNodeMixShader")
    tree.links.new(light_path.outputs["Is Camera Ray"], mix.inputs[0])
    tree.links.new(ambient.outputs[0], mix.inputs[1])
    tree.links.new(background.outputs[0], mix.inputs[2])
    output = tree.nodes.new("ShaderNodeOutputWorld")
    tree.links.new(mix.outputs[0], output.inputs["Surface"])
    bpy.context.scene.world = world


def configure(manifest):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.render.resolution_x, scene.render.resolution_y = 3840, 2160
    scene.render.resolution_percentage = 100
    scene.render.pixel_aspect_x = scene.render.pixel_aspect_y = 1.0
    scene.render.film_transparent = False
    scene.view_settings.exposure = math.log2(max(float(manifest["look"]["exposure"]), 0.000001))
    scene["tosklight_frozen_scene_revision"] = str(manifest["scene_revision"])
    scene["tosklight_frozen_values_frame"] = str(manifest["values_frame"])
    scene["tosklight_frozen_time_seconds"] = manifest["time_seconds"]
    scene["tosklight_look_mapping"] = manifest["look"]["mapping"]
    add_camera(manifest)
    add_world(manifest)
