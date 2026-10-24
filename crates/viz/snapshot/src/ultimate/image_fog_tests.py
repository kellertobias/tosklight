"""CPU execution of the production fog-node graph, without importing/running Blender."""
import importlib.util
import math
from pathlib import Path
import sys
import types
import unittest

spec = importlib.util.spec_from_file_location("frozen_fog", Path(__file__).with_name("image_fog.py"))
fog = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fog)


def voxels():
    state = 0x2545F4914F6CDD1D
    result = []
    mask = (1 << 64) - 1
    for _ in range(32**3):
        state ^= (state << 13) & mask
        state ^= state >> 7
        state ^= (state << 17) & mask
        state &= mask
        result.append((state >> 33) & 255)
    return result


class Socket:
    def __init__(self, node, key, output=False):
        self.node, self.key, self.output = node, key, output
        self.source = None
        self.default_value = 0.0

    def evaluate(self, point, memo):
        if id(self) not in memo:
            if not self.output:
                value = self.source.evaluate(point, memo) if self.source else self.default_value
            else:
                value = self.node.evaluate(self.key, point, memo)
            memo[id(self)] = value
        return memo[id(self)]


class Sockets(dict):
    def __init__(self, node, output=False):
        super().__init__()
        self.node, self.output = node, output

    def __missing__(self, key):
        self[key] = Socket(self.node, key, self.output)
        return self[key]


class Node:
    def __init__(self, kind):
        self.kind = kind
        self.inputs, self.outputs = Sockets(self), Sockets(self, True)

    def evaluate(self, key, point, memo):
        read = lambda name: self.inputs[name].evaluate(point, memo)
        if self.kind == "ShaderNodeNewGeometry":
            return point
        if self.kind == "ShaderNodeSeparateXYZ":
            return read(0)["XYZ".index(key)]
        if self.kind == "ShaderNodeCombineXYZ":
            return tuple(read(axis) for axis in "XYZ")
        if self.kind == "ShaderNodeTexImage":
            return bilinear(self.image, read("Vector"))
        if self.kind != "ShaderNodeMath":
            raise AssertionError(self.kind)
        a, b = read(0), read(1)
        operations = {
            "ADD": lambda: a + b, "SUBTRACT": lambda: a - b,
            "MULTIPLY": lambda: a * b, "DIVIDE": lambda: a / b,
            "FLOOR": lambda: math.floor(a), "MINIMUM": lambda: min(a, b),
            "MAXIMUM": lambda: max(a, b), "POWER": lambda: a**b,
        }
        return operations[self.operation]()


class Pixels:
    def foreach_set(self, values):
        self.values = values


class Image:
    def __init__(self, width, height):
        self.width, self.height = width, height
        self.pixels = Pixels()
        self.colorspace_settings = types.SimpleNamespace(name="")

    def update(self):
        pass


def bilinear(image, coordinate):
    x, y = coordinate[0] * image.width - 0.5, coordinate[1] * image.height - 0.5
    ix, iy = math.floor(x), math.floor(y)
    value = 0.0
    for dy in (0, 1):
        for dx in (0, 1):
            sx = min(max(ix + dx, 0), image.width - 1)
            sy = min(max(iy + dy, 0), image.height - 1)
            weight = (x - ix if dx else 1 - x + ix) * (y - iy if dy else 1 - y + iy)
            value += image.pixels.values[(sy * image.width + sx) * 4] * weight
    return value


def native_sample(data, coordinate):
    base = [math.floor(c * 32 - 0.5) for c in coordinate]
    fraction = [c * 32 - 0.5 - b for c, b in zip(coordinate, base)]
    value = 0.0
    for dz in (0, 1):
        for dy in (0, 1):
            for dx in (0, 1):
                xyz = [(b + d) % 32 for b, d in zip(base, (dx, dy, dz))]
                weight = math.prod(f if d else 1 - f for f, d in zip(fraction, (dx, dy, dz)))
                value += data[(xyz[2] * 32 + xyz[1]) * 32 + xyz[0]] / 255.0 * weight
    return value


def tree():
    return types.SimpleNamespace(
        nodes=types.SimpleNamespace(new=Node),
        links=types.SimpleNamespace(new=lambda source, target: setattr(target, "source", source)),
    )


class FrozenFogTests(unittest.TestCase):
    def test_guarded_atlas_matches_native_trilinear_repeat_including_negative_seams(self):
        data = voxels()
        image = Image(fog.PAD, fog.PAD * fog.EDGE)
        image.pixels.foreach_set(fog.atlas_rgba(data))
        for coordinate in ((0, 0, 0), (1, 1, 1), (-0.001, 1.001, -1.999),
                           (0.5 / 32, 31.5 / 32, 16 / 32), (2.125, -0.375, 7.456)):
            graph = tree()
            result = fog.linear_sample(graph, image, coordinate).evaluate((0, 0, 0), {})
            self.assertAlmostEqual(result, native_sample(data, coordinate), places=11)

    def test_production_octave_graph_preserves_frozen_time_turbulence_and_world_axes(self):
        data = voxels()
        fake = types.SimpleNamespace(data=types.SimpleNamespace(images=types.SimpleNamespace(
            new=lambda name, width, height, float_buffer: Image(width, height))))
        previous = sys.modules.get("bpy")
        sys.modules["bpy"] = fake
        try:
            for time, turbulence in ((0.0, 1.0), (37.25, 0.0), (37.25, 0.8)):
                settings = {"noise_voxels": data, "time_seconds": time,
                            "lamp_turbulence": turbulence, "lamp_cloudiness": 0.7}
                result = fog.native_field(tree(), settings)
                for native in ((1.2, -4.5, 9.1), (-21.7, 0.03, 2.0), (0, 0, 0)):
                    drift = [time * turbulence * speed for speed in (0.03, 0.017, -0.023)]
                    combined = sum(native_sample(data, [p * scale + d * shift for p, d in zip(native, drift)]) * weight
                                   for scale, shift, weight in ((0.035, 0.4, 0.5), (0.11, 1.0, 0.33), (0.3, -1.6, 0.17)))
                    expected = 1 + 0.7 * (min(max(combined * 1.75, 0), 1.6)**1.7 - 1)
                    blender = (native[0], -native[2], native[1])
                    self.assertAlmostEqual(result.evaluate(blender, {}), expected, places=10)
        finally:
            if previous is None:
                sys.modules.pop("bpy", None)
            else:
                sys.modules["bpy"] = previous

    def test_atlas_rejects_wrong_dimensions_and_non_r8_values(self):
        with self.assertRaises(RuntimeError):
            fog.atlas_rgba([0])
        invalid = [0] * 32**3
        invalid[0] = 256
        with self.assertRaises(RuntimeError):
            fog.atlas_rgba(invalid)


if __name__ == "__main__":
    unittest.main()
