"""Frozen native lamp haze field. No live time, procedural replacement noise, or simulations.

The spatial R8/trilinear field and drift/shape match beam.wgsl. Cycles' participating-medium
transport deliberately remains the documented 0.06*density mapping, not native per-beam opacity.
Active native lasers remain unsupported: their independent sinusoidal field is not this medium.
"""
import math

EDGE = 32
PAD = EDGE + 2


def atlas_rgba(voxels):
    """Each Z slice has repeated XY guard texels, so bilinear sampling cannot leak across slices."""
    if len(voxels) != EDGE**3 or any(type(v) is not int or not 0 <= v <= 255 for v in voxels):
        raise RuntimeError("Frozen fog requires the exact native 32-cubed R8 volume")
    pixels = []
    for z in range(EDGE):
        for y in range(-1, EDGE + 1):
            for x in range(-1, EDGE + 1):
                value = voxels[(z * EDGE + y % EDGE) * EDGE + x % EDGE] / 255.0
                pixels.extend((value, value, value, 1.0))
    return pixels


def math_node(tree, operation, *inputs):
    node = tree.nodes.new("ShaderNodeMath")
    node.operation = operation
    for index, value in enumerate(inputs):
        if isinstance(value, (int, float)):
            node.inputs[index].default_value = value
        else:
            tree.links.new(value, node.inputs[index])
    return node.outputs[0]


def fraction(tree, value):
    return math_node(tree, "SUBTRACT", value, math_node(tree, "FLOOR", value))


def linear_sample(tree, image, xyz):
    """Two guarded bilinear slices exactly reproduce normalized Repeat+Linear 3D sampling."""
    u = math_node(tree, "DIVIDE", math_node(tree, "ADD", math_node(tree, "MULTIPLY", fraction(tree, xyz[0]), EDGE), 1), PAD)
    y = math_node(tree, "ADD", math_node(tree, "MULTIPLY", fraction(tree, xyz[1]), EDGE), 1)
    z = math_node(tree, "SUBTRACT", math_node(tree, "MULTIPLY", xyz[2], EDGE), 0.5)
    z0 = math_node(tree, "FLOOR", z)
    blend = fraction(tree, z)
    sampled = []
    for offset in (0, 1):
        layer = math_node(tree, "ADD", z0, offset)
        layer = math_node(tree, "MULTIPLY", fraction(tree, math_node(tree, "DIVIDE", layer, EDGE)), EDGE)
        v = math_node(tree, "DIVIDE", math_node(tree, "ADD", y, math_node(tree, "MULTIPLY", layer, PAD)), PAD * EDGE)
        combine = tree.nodes.new("ShaderNodeCombineXYZ")
        tree.links.new(u, combine.inputs["X"])
        tree.links.new(v, combine.inputs["Y"])
        texture = tree.nodes.new("ShaderNodeTexImage")
        texture.image = image
        texture.interpolation = "Linear"
        texture.extension = "EXTEND"
        tree.links.new(combine.outputs[0], texture.inputs["Vector"])
        sampled.append(texture.outputs["Color"])
    difference = math_node(tree, "SUBTRACT", sampled[1], sampled[0])
    return math_node(tree, "ADD", sampled[0], math_node(tree, "MULTIPLY", blend, difference))


def native_field(tree, fog):
    import bpy
    image = bpy.data.images.new("Frozen native haze R8 slices", width=PAD, height=PAD * EDGE, float_buffer=True)
    image.colorspace_settings.name = "Non-Color"
    image.pixels.foreach_set(atlas_rgba(fog["noise_voxels"]))
    image.update()
    geometry = tree.nodes.new("ShaderNodeNewGeometry")
    xyz = tree.nodes.new("ShaderNodeSeparateXYZ")
    tree.links.new(geometry.outputs["Position"], xyz.inputs[0])
    # Inverse of native Y-up -> Blender Z-up (x,-z,y).
    native = (xyz.outputs["X"], xyz.outputs["Z"], math_node(tree, "MULTIPLY", xyz.outputs["Y"], -1))
    moving_time = float(fog["time_seconds"]) * float(fog["lamp_turbulence"])
    drift = (moving_time * 0.03, moving_time * 0.017, moving_time * -0.023)
    combined = 0.0
    for frequency, drift_scale, weight in ((0.035, 0.4, 0.5), (0.11, 1.0, 0.33), (0.3, -1.6, 0.17)):
        coordinates = [math_node(tree, "ADD", math_node(tree, "MULTIPLY", axis, frequency), d * drift_scale)
                       for axis, d in zip(native, drift)]
        octave = linear_sample(tree, image, coordinates)
        combined = math_node(tree, "ADD", combined, math_node(tree, "MULTIPLY", octave, weight))
    shaped = math_node(tree, "MULTIPLY", combined, 1.75)
    shaped = math_node(tree, "MINIMUM", math_node(tree, "MAXIMUM", shaped, 0), 1.6)
    shaped = math_node(tree, "POWER", shaped, 1.7)
    return math_node(tree, "ADD", 1, math_node(tree, "MULTIPLY", float(fog["lamp_cloudiness"]), math_node(tree, "SUBTRACT", shaped, 1)))


def add_fog(manifest, folder):
    import bpy
    from mathutils import Vector
    fog = manifest["fog"]
    density, cloudiness, turbulence, time = (float(fog[key]) for key in
        ("density", "lamp_cloudiness", "lamp_turbulence", "time_seconds"))
    if (not all(math.isfinite(x) for x in (density, cloudiness, turbulence, time))
            or not all(0 <= x <= 1 for x in (density, cloudiness, turbulence)) or time < 0):
        raise RuntimeError("Invalid frozen fog parameters")
    if density <= 0.0005:
        return None
    low, high = Vector(fog["min"]), Vector(fog["max"])
    if not all(math.isfinite(x) for x in (*low, *high)) or any(a >= b for a, b in zip(low, high)):
        raise RuntimeError("Invalid finite frozen fog bounds")
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(low + high) * 0.5)
    box = bpy.context.active_object
    box.name = "Frozen native atmosphere"
    box.scale = high - low
    box.display_type = "WIRE"
    box.visible_shadow = False
    box["tosklight_fog_mapping"] = fog["mapping"]
    box["tosklight_frozen_fog_time_seconds"] = time
    material = bpy.data.materials.new("Frozen native haze")
    if bpy.app.version < (6, 0):
        material.use_nodes = True
    tree = material.node_tree
    tree.nodes.clear()
    volume = tree.nodes.new("ShaderNodeVolumePrincipled")
    volume.inputs["Anisotropy"].default_value = 0.35
    if cloudiness > 0.001:
        if fog["noise_edge"] != EDGE:
            raise RuntimeError("Unsupported frozen fog texture dimensions")
        field = native_field(tree, fog)
        tree.links.new(math_node(tree, "MULTIPLY", field, density * 0.06), volume.inputs["Density"])
    else:
        volume.inputs["Density"].default_value = density * 0.06
    output = tree.nodes.new("ShaderNodeOutputMaterial")
    tree.links.new(volume.outputs["Volume"], output.inputs["Volume"])
    box.data.materials.append(material)
    return box
