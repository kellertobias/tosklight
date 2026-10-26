"""CPU evaluation of the actual projector node graph; no Blender process or imports required."""
import importlib.util
import math
import pathlib
import sys
import types
import unittest
from unittest.mock import patch


class Socket:
    def __init__(self):
        self.default_value = 0.0
        self.source = None
        self.owner = None


class Sockets(dict):
    def __missing__(self, key):
        self[key] = Socket()
        return self[key]


class Node:
    def __init__(self, kind):
        self.kind = kind
        self.inputs, self.outputs = Sockets(), Sockets()
        self.outputs[0].owner = self
        self.outputs["Ray Length"].owner = self


class Nodes(list):
    def new(self, kind):
        node = Node(kind)
        self.append(node)
        return node


class Tree:
    def __init__(self):
        self.nodes = Nodes()
        self.links = types.SimpleNamespace(new=lambda source, target: setattr(target, "source", source))


def evaluate(socket, distance):
    if socket.source is not None:
        return evaluate(socket.source, distance)
    node = socket.owner
    if node is None:
        return socket.default_value
    if node.kind == "ShaderNodeLightPath":
        return distance
    if node.kind == "ShaderNodeMapRange":
        value = evaluate(node.inputs["Value"], distance)
        low, high = (node.inputs[key].default_value for key in ("From Min", "From Max"))
        x = (value - low) / (high - low)
        if node.clamp:
            x = min(max(x, 0.0), 1.0)
        if node.interpolation_type == "SMOOTHSTEP":
            x = x * x * (3.0 - 2.0 * x)
        start, end = (node.inputs[key].default_value for key in ("To Min", "To Max"))
        return start + (end - start) * x
    raise AssertionError("Unexpected shader node " + node.kind)


def load_backend():
    bpy = types.SimpleNamespace(app=types.SimpleNamespace(version=(5, 1)))
    modules = {"bpy": bpy, "mathutils": types.SimpleNamespace(Matrix=None, Vector=None),
               "image_scene": types.SimpleNamespace(checked_asset=None)}
    path = pathlib.Path(__file__).with_name("image_media.py")
    spec = importlib.util.spec_from_file_location("test_image_media_backend", path)
    module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, modules):
        spec.loader.exec_module(module)
    return module


class ProjectorRangeTests(unittest.TestCase):
    def test_actual_shader_matches_native_finite_envelope_and_has_no_hard_step(self):
        backend = load_backend()
        data = types.SimpleNamespace(node_tree=Tree())
        backend.projector_range_shader(data, 10.0)
        emission = next(node for node in data.node_tree.nodes if node.kind == "ShaderNodeEmission")
        for distance in (0.0, 2.0, 7.999, 8.0, 8.5, 9.0, 9.999, 10.0, 10.001, 100.0):
            x = min(max((distance - 8.0) / 2.0, 0.0), 1.0)
            expected = 1.0 - x * x * (3.0 - 2.0 * x)
            self.assertAlmostEqual(evaluate(emission.inputs["Strength"], distance), expected)
        self.assertAlmostEqual(evaluate(emission.inputs["Strength"], 9.0), 0.5)
        self.assertLess(evaluate(emission.inputs["Strength"], 9.999), 0.000001)
        output = next(node for node in data.node_tree.nodes if node.kind == "ShaderNodeOutputLight")
        self.assertIs(output.inputs["Surface"].source.owner, emission)

    def test_invalid_reach_fails_before_creating_shader(self):
        backend = load_backend()
        for reach in (0.0, -1.0, math.inf, math.nan):
            data = types.SimpleNamespace(node_tree=Tree())
            with self.assertRaises(RuntimeError):
                backend.projector_range_shader(data, reach)
            self.assertEqual(data.node_tree.nodes, [])


class PanelOrientationTests(unittest.TestCase):
    def test_physical_top_left_gets_source_top_left_and_blender_v_up(self):
        maps = load_backend().panel_uv_maps((0.1, 0.05, 0.8, 0.7))
        # Actual unit_plane vertex order: bottom-left/right, top-right/left after rotation.
        self.assertEqual(maps["FrozenPanelUV"], [(0, 1), (1, 1), (1, 0), (0, 0)])
        expected = [(0.1, 0.75), (0.9, 0.75), (0.9, 0.05), (0.1, 0.05)]
        for native, texture, target in zip(maps["FrozenNativeUV"], maps["FrozenTextureUV"], expected):
            self.assertAlmostEqual(native[0], target[0])
            self.assertAlmostEqual(native[1], target[1])
            self.assertAlmostEqual(texture[0], target[0])
            self.assertAlmostEqual(texture[1], 1.0 - target[1])

    def test_led_rows_keep_their_own_crop_instead_of_flipping_across_source(self):
        backend = load_backend()
        upper = backend.panel_uv_maps((0.1, 0.05, 0.4, 0.35))["FrozenNativeUV"]
        lower = backend.panel_uv_maps((0.1, 0.4, 0.4, 0.35))["FrozenNativeUV"]
        self.assertAlmostEqual(upper[3][1], 0.05)
        self.assertAlmostEqual(upper[0][1], lower[3][1])
        self.assertAlmostEqual(lower[0][1], 0.75)


if __name__ == "__main__":
    unittest.main()
