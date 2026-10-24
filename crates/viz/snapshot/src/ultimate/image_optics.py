"""Project frozen native angular masks through rotation-aware Cycles spot-light coordinates.

Normal coordinates on Point/Spot lights are documented by Blender's Texture Coordinate node.
A gate is an angular distribution, not a polygon placed in the beam or a substitute gobo.
"""
import math
import bpy
from mathutils import Matrix, Vector
from image_scene import checked_asset
from image_media import smooth


def math_node(tree, operation, a, b=None):
    node = tree.nodes.new("ShaderNodeMath")
    node.operation = operation
    for index, value in enumerate((a, b)):
        if value is None:
            continue
        if isinstance(value, (int, float)):
            node.inputs[index].default_value = value
        else:
            tree.links.new(value, node.inputs[index])
    return node.outputs[0]


def angular_shader(data, entry, folder, peak):
    if bpy.app.version < (4, 5):
        raise RuntimeError("Frozen optical projection requires Blender 4.5 or newer; choose Ultra")
    if bpy.app.version < (6, 0):
        data.use_nodes = True
    tree = data.node_tree
    if tree is None:
        raise RuntimeError("Installed Blender exposes no Cycles light-node tree; choose Ultra")
    tree.nodes.clear()
    coords = tree.nodes.new("ShaderNodeTexCoord")
    separate = tree.nodes.new("ShaderNodeSeparateXYZ")
    tree.links.new(coords.outputs["Normal"], separate.inputs[0])
    # Local axes are X=tangent, Y=-native-bitangent, Z=-aim. Dividing both Normal
    # components by Z cancels its inward/outward convention. PNG rows start at native gate y=-1,
    # whereas Blender image V starts at the bottom, so V uses the opposite native gate sign.
    scale = 0.5 / math.tan(entry["cone_radians"] * 0.5)
    u = math_node(tree, "DIVIDE", separate.outputs["X"], separate.outputs["Z"])
    u = math_node(tree, "ADD", math_node(tree, "MULTIPLY", u, -scale), 0.5)
    v = math_node(tree, "DIVIDE", separate.outputs["Y"], separate.outputs["Z"])
    v = math_node(tree, "ADD", math_node(tree, "MULTIPLY", v, -scale), 0.5)
    combine = tree.nodes.new("ShaderNodeCombineXYZ")
    tree.links.new(u, combine.inputs["X"])
    tree.links.new(v, combine.inputs["Y"])
    texture = tree.nodes.new("ShaderNodeTexImage")
    texture.image = bpy.data.images.load(checked_asset(folder, entry["mask_file"]), check_existing=True)
    texture.image.colorspace_settings.name = "Non-Color"
    texture.interpolation = "Linear"
    texture.extension = "CLIP"
    tree.links.new(combine.outputs[0], texture.inputs["Vector"])
    emission = tree.nodes.new("ShaderNodeEmission")
    # Blender 4.5+ applies data.energy/color separately to the node emission. Do not apply either
    # twice: the node owns only normalized mask transmission and its native hotspot peak.
    tree.links.new(texture.outputs["Color"], emission.inputs["Color"])
    reach = float(entry["reach"])
    if not math.isfinite(reach) or reach <= 0:
        raise RuntimeError("Frozen optical reach must be finite and positive")
    path = tree.nodes.new("ShaderNodeLightPath")
    fade = smooth(tree, path.outputs["Ray Length"], reach * 0.8, reach, 1.0, 0.0)
    tree.links.new(math_node(tree, "MULTIPLY", fade, peak), emission.inputs["Strength"])
    output = tree.nodes.new("ShaderNodeOutputLight")
    tree.links.new(emission.outputs[0], output.inputs["Surface"])


def add_optics(manifest, folder):
    optics = manifest["optics"]
    peak = float(optics["angular_mask_peak"])
    if not math.isfinite(peak) or peak <= 0:
        raise RuntimeError("Invalid frozen angular mask peak")
    collection = bpy.data.collections.new("Frozen Rig Lights")
    bpy.context.scene.collection.children.link(collection)
    for entry in optics["lights"]:
        axis = Vector(entry["direction"]).normalized()
        tangent = Vector(entry["tangent"])
        tangent = (tangent - axis * tangent.dot(axis)).normalized()
        if axis.length < 0.99 or tangent.length < 0.99:
            raise RuntimeError("Invalid frozen light-local coordinate basis")
        bitangent = axis.cross(tangent).normalized()
        basis = Matrix((tangent, -bitangent, -axis)).transposed().to_4x4()
        basis.translation = Vector(entry["origin"])
        data = bpy.data.lights.new(name=entry["name"], type="SPOT")
        data.color = tuple(entry["colour"])
        data.energy = entry["power_watts"]
        data.spot_size = entry["cone_radians"]
        data.spot_blend = 0.0  # The frozen native mask already owns the full rim and core profile.
        data.shadow_soft_size = max(float(entry["source_radius"]), 0.0)
        angular_shader(data, entry, folder, peak)
        light = bpy.data.objects.new(name=entry["name"], object_data=data)
        light.matrix_world = basis
        light["tosklight_fixture"] = entry["fixture_id"]
        light["tosklight_instance"] = entry["fixture_instance_id"]
        light["tosklight_emitter"] = entry["emitter_index"]
        light["tosklight_cell"] = entry["cell_index"]
        collection.objects.link(light)
    return collection
