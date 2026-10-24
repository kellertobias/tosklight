"""Native frozen panel placements/crops and projector appearance, using package PNGs only."""
import math
import bpy
from mathutils import Matrix, Vector
from image_scene import checked_asset


def scalar(tree, operation, a, b):
    node = tree.nodes.new("ShaderNodeMath")
    node.operation = operation
    for index, value in enumerate((a, b)):
        if isinstance(value, (int, float)):
            node.inputs[index].default_value = value
        else:
            tree.links.new(value, node.inputs[index])
    return node.outputs[0]


def smooth(tree, value, low, high, start=0.0, end=1.0):
    node = tree.nodes.new("ShaderNodeMapRange")
    node.interpolation_type = "SMOOTHSTEP"
    node.clamp = True
    tree.links.new(value, node.inputs["Value"])
    for name, number in (("From Min", low), ("From Max", high), ("To Min", start), ("To Max", end)):
        node.inputs[name].default_value = number
    return node.outputs[0]


def multiply_colour(tree, colour, factor):
    node = tree.nodes.new("ShaderNodeMixRGB")
    node.blend_type = "MULTIPLY"
    node.inputs[0].default_value = 1.0
    tree.links.new(colour, node.inputs[1])
    tree.links.new(factor, node.inputs[2])
    return node.outputs[0]


def uv_node(tree, name):
    node = tree.nodes.new("ShaderNodeUVMap")
    node.uv_map = name
    return node.outputs["UV"]


def edge_feather(tree, feather):
    split = tree.nodes.new("ShaderNodeSeparateXYZ")
    tree.links.new(uv_node(tree, "FrozenPanelUV"), split.inputs[0])
    u, v = split.outputs["X"], split.outputs["Y"]
    horizontal = scalar(tree, "MINIMUM", u, scalar(tree, "SUBTRACT", 1.0, u))
    vertical = scalar(tree, "MINIMUM", v, scalar(tree, "SUBTRACT", 1.0, v))
    return smooth(tree, scalar(tree, "MINIMUM", horizontal, vertical), 0.0, max(feather, 0.0001))


def led_diode(tree, pitch):
    scale = tree.nodes.new("ShaderNodeVectorMath")
    scale.operation = "SCALE"
    tree.links.new(uv_node(tree, "FrozenNativeUV"), scale.inputs[0])
    scale.inputs[3].default_value = min(max(1000.0 / max(pitch, 1.0), 32.0), 300.0)
    fraction = tree.nodes.new("ShaderNodeVectorMath")
    fraction.operation = "FRACTION"
    tree.links.new(scale.outputs["Vector"], fraction.inputs[0])
    centre = tree.nodes.new("ShaderNodeVectorMath")
    centre.operation = "SUBTRACT"
    tree.links.new(fraction.outputs["Vector"], centre.inputs[0])
    centre.inputs[1].default_value = (0.5, 0.5, 0.0)
    length = tree.nodes.new("ShaderNodeVectorMath")
    length.operation = "LENGTH"
    tree.links.new(centre.outputs["Vector"], length.inputs[0])
    diode = smooth(tree, length.outputs["Value"], 0.16, 0.42, 1.0, 0.0)
    return scalar(tree, "ADD", 0.45, scalar(tree, "MULTIPLY", 0.55, diode))


def panel_material(panel, folder, images):
    material = bpy.data.materials.new("Frozen media " + str(panel["selected_file"]))
    if bpy.app.version < (6, 0):
        material.use_nodes = True
    tree = material.node_tree
    tree.nodes.clear()
    colour = tree.nodes.new("ShaderNodeRGB")
    colour.outputs[0].default_value = (0.0, 0.0, 0.0, 1.0)
    selected = panel["selected_file"]
    if selected is not None:
        if selected not in images:
            image = bpy.data.images.load(checked_asset(folder, selected), check_existing=False)
            image.colorspace_settings.name = "sRGB"
            image.pack()
            images[selected] = image
        texture = tree.nodes.new("ShaderNodeTexImage")
        texture.image = images[selected]
        texture.interpolation = "Linear"
        texture.extension = "EXTEND"
        tree.links.new(uv_node(tree, "FrozenTextureUV"), texture.inputs["Vector"])
        colour = texture
    colour_output = colour.outputs[0]
    kind, gain, roughness, detail = panel["material"]
    if kind < 0.5:
        colour_output = multiply_colour(tree, colour_output, edge_feather(tree, detail))
        shader = tree.nodes.new("ShaderNodeBsdfDiffuse")
        shader.inputs["Roughness"].default_value = roughness
        # Native projected image is reflective, unlike self-emitting TV/LED surfaces.
        strength = tree.nodes.new("ShaderNodeValue")
        strength.outputs[0].default_value = gain
        tree.links.new(multiply_colour(tree, colour_output, strength.outputs[0]), shader.inputs["Color"])
    else:
        if kind >= 1.5:
            colour_output = multiply_colour(tree, colour_output, led_diode(tree, detail))
        shader = tree.nodes.new("ShaderNodeEmission")
        shader.inputs["Strength"].default_value = gain
        tree.links.new(colour_output, shader.inputs["Color"])
    output = tree.nodes.new("ShaderNodeOutputMaterial")
    tree.links.new(shader.outputs[0], output.inputs["Surface"])
    return material


def add_panels(manifest, folder):
    images, materials = {}, {}
    for index, panel in enumerate(manifest["media"]["panels"]):
        mesh = bpy.data.meshes.new("Frozen panel " + str(index))
        # Same unit_plane vertices/winding and UVs as the native MediaPanel pass.
        mesh.from_pydata([(-0.5, 0.0, 0.5), (0.5, 0.0, 0.5), (0.5, 0.0, -0.5), (-0.5, 0.0, -0.5)], [], [(0, 1, 2, 3)])
        mesh.update()
        left, top, width, height = panel["crop"]
        native_uv = [(left + u * width, top + v * height) for u, v in [(0, 0), (1, 0), (1, 1), (0, 1)]]
        maps = {"FrozenNativeUV": native_uv, "FrozenTextureUV": [(u, 1.0 - v) for u, v in native_uv],
                "FrozenPanelUV": [(0, 0), (1, 0), (1, 1), (0, 1)]}
        for name, coordinates in maps.items():
            layer = mesh.uv_layers.new(name=name)
            for loop in mesh.loops:
                layer.data[loop.index].uv = coordinates[loop.vertex_index]
        key = (panel["selected_file"], tuple(panel["material"]))
        if key not in materials:
            materials[key] = panel_material(panel, folder, images)
        mesh.materials.append(materials[key])
        placed = bpy.data.objects.new("Frozen media panel " + str(index), mesh)
        placed.matrix_world = Matrix(panel["world_from_panel"]).transposed()
        bpy.context.scene.collection.objects.link(placed)


def add_projectors(manifest):
    panels = [obj for obj in bpy.context.scene.objects if obj.name.startswith("Frozen media panel ")]
    for projector in manifest["media"]["projectors"]:
        # Native preview separates fixed haze brightness from the operator's spill setting.
        for volume in (False, True):
            data = bpy.data.lights.new(projector["name"] + (" haze" if volume else " spill"), "SPOT")
            data.color = projector["colour"]
            data.energy = 400.0 * projector["flicker_gain"] * (0.32 if volume else projector["spill"])
            data.spot_size, data.spot_blend = 0.48, 0.25
            data.diffuse_factor = data.specular_factor = data.transmission_factor = 0.0 if volume else 1.0
            data.volume_factor = 1.0 if volume else 0.0
            projector_range_shader(data, projector["reach"])
            light = bpy.data.objects.new(data.name, data)
            light.location = Vector(projector["position"])
            light.rotation_euler = Vector(projector["direction"]).to_track_quat("-Z", "Y").to_euler()
            light["tosklight_frozen_flicker_gain"] = projector["flicker_gain"]
            bpy.context.scene.collection.objects.link(light)
            exclude_projector_lens(light, projector)
            if panels:
                # Native media_fragment deliberately excludes projector-class light from its
                # reflected image. Keep those panels as real shadow blockers, but never repaint
                # them with their own linked cone. Blender 4.5 supports these Cycles receivers.
                bpy.ops.object.select_all(action="DESELECT")
                for panel in panels:
                    panel.select_set(True)
                bpy.context.view_layer.objects.active = light
                light.select_set(True)
                bpy.ops.object.light_linking_receivers_link(link_state="EXCLUDE")


def exclude_projector_lens(light, projector):
    # The native emission origin is inside its closed procedural lens. That exact lens may
    # transmit its own source while every other object (including other lenses) blocks normally.
    name = projector["lens_node"]
    if name != "tosklight-projector-lens-" + projector["id"]:
        raise RuntimeError("Invalid frozen projector lens identity")
    lens = bpy.data.objects.get(name)
    if lens is None or lens.type != "MESH":
        raise RuntimeError("Frozen projector lens was not imported with its stable placement identity")
    bpy.ops.object.select_all(action="DESELECT")
    lens.select_set(True)
    light.select_set(True)
    bpy.context.view_layer.objects.active = light
    bpy.ops.object.light_linking_blockers_link(link_state="EXCLUDE")
    light["tosklight_transmitting_own_lens"] = name


def projector_range_shader(data, reach):
    """Cycles finite reach: exactly native range_envelope, independent of Eevee cutoff distance.

    Ray Length in a Cycles light shader is the sampled light-to-receiver/volume distance. The
    renderer's physical inverse-square transport remains Cycles-owned; this node adds only the
    native smooth finite work bound, with value and derivative zero at the authored reach.
    """
    if not math.isfinite(reach) or reach <= 0:
        raise RuntimeError("Frozen projector reach must be finite and positive")
    if bpy.app.version < (6, 0):
        data.use_nodes = True
    tree = data.node_tree
    if tree is None:
        raise RuntimeError("Installed Blender exposes no Cycles projector light-node tree")
    tree.nodes.clear()
    path = tree.nodes.new("ShaderNodeLightPath")
    fade = smooth(tree, path.outputs["Ray Length"], reach * 0.8, reach, 1.0, 0.0)
    emission = tree.nodes.new("ShaderNodeEmission")
    emission.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    tree.links.new(fade, emission.inputs["Strength"])
    output = tree.nodes.new("ShaderNodeOutputLight")
    tree.links.new(emission.outputs[0], output.inputs["Surface"])
