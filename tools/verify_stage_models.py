#!/usr/bin/env python3
"""Check the generated stage models against the import contract.

This reads the ``.glb`` files with nothing but the standard library, so it checks what
the visualizer's reader will actually see rather than what Blender believed it wrote.
The rules come from part 1 and part 7 of
``docs/engineering/fixture-and-stage-model-brief.md``.

    python3 tools/verify_stage_models.py [--models assets/models]
"""

from __future__ import annotations

import argparse
import json
import math
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MODELS = ROOT / "assets" / "models"

MAX_TRIANGLES = 120_000
MAX_ENTRY_BYTES = 64 * 1024 * 1024
FLOAT = 5126
INDEX_TYPES = {5121: "B", 5123: "H", 5125: "I"}
COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}

BASE, YOKE, HEAD = "base", "yoke", "head"


def classify(name: str, inherited: str = BASE) -> str:
    folded = name.lower()
    if "head" in folded or "lamp" in folded or "tilt" in folded:
        return HEAD
    if "yoke" in folded or "arm" in folded or "pan" in folded:
        return YOKE
    return inherited


@dataclass
class NodeGeometry:
    """One node's triangles, already in model space and in glTF axes."""

    name: str
    kind: str
    positions: list[tuple[float, float, float]]
    triangles: list[tuple[int, int, int]]

    def bounds(self) -> tuple[list[float], list[float]]:
        low = [min(point[axis] for point in self.positions) for axis in range(3)]
        high = [max(point[axis] for point in self.positions) for axis in range(3)]
        return low, high

    def signed_volume(self) -> float:
        total = 0.0
        for first, second, third in self.triangles:
            a, b, c = self.positions[first], self.positions[second], self.positions[third]
            cross = (
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            )
            total += a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]
        return total / 6.0


@dataclass
class ModelReport:
    path: Path
    nodes: list[NodeGeometry] = field(default_factory=list)
    problems: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def fail(self, message: str) -> None:
        self.problems.append(message)

    def triangle_count(self) -> int:
        return sum(len(node.triangles) for node in self.nodes)

    def bounds(self) -> tuple[list[float], list[float]]:
        low = [math.inf] * 3
        high = [-math.inf] * 3
        for node in self.nodes:
            node_low, node_high = node.bounds()
            low = [min(low[axis], node_low[axis]) for axis in range(3)]
            high = [max(high[axis], node_high[axis]) for axis in range(3)]
        return low, high

    def kind_bounds(self, kinds: set[str]) -> tuple[list[float], list[float]] | None:
        selected = [node for node in self.nodes if node.kind in kinds and node.positions]
        if not selected:
            return None
        low = [min(node.bounds()[0][axis] for node in selected) for axis in range(3)]
        high = [max(node.bounds()[1][axis] for node in selected) for axis in range(3)]
        return low, high


def split_chunks(data: bytes, report: ModelReport) -> tuple[dict, bytes] | None:
    if len(data) < 20:
        report.fail("file is too short to be a GLB")
        return None
    magic, version, declared = struct.unpack_from("<III", data, 0)
    if magic != 0x46546C67:
        report.fail("file is not a GLB")
        return None
    if version != 2:
        report.fail(f"GLB version is {version}, not 2")
    if declared != len(data):
        report.fail(f"GLB declares {declared} bytes but holds {len(data)}")
    if len(data) > MAX_ENTRY_BYTES:
        report.fail(f"file is {len(data)} bytes, over the 64 MiB package entry limit")
    cursor, document, binary = 12, None, b""
    while cursor + 8 <= len(data):
        length, kind = struct.unpack_from("<II", data, cursor)
        cursor += 8
        chunk = data[cursor : cursor + length]
        if kind == 0x4E4F534A and document is None:
            document = json.loads(chunk)
        elif kind == 0x004E4942 and not binary:
            binary = chunk
        cursor += length + (-length % 4)
    if document is None:
        report.fail("GLB has no JSON chunk")
        return None
    return document, binary


def read_accessor(document: dict, binary: bytes, index: int) -> list[tuple[float, ...]]:
    accessor = document["accessors"][index]
    count = accessor["count"]
    width = COMPONENTS[accessor["type"]]
    view = document["bufferViews"][accessor["bufferView"]]
    offset = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    component = accessor["componentType"]
    code = {FLOAT: "f", **{key: value for key, value in INDEX_TYPES.items()}}[component]
    size = struct.calcsize(code)
    stride = view.get("byteStride") or size * width
    values = []
    for item in range(count):
        start = offset + item * stride
        values.append(struct.unpack_from(f"<{width}{code}", binary, start))
    return values


def node_matrix(node: dict) -> list[float]:
    if "matrix" in node:
        return list(node["matrix"])
    translation = node.get("translation", [0.0, 0.0, 0.0])
    scale = node.get("scale", [1.0, 1.0, 1.0])
    x, y, z, w = node.get("rotation", [0.0, 0.0, 0.0, 1.0])
    rotation = [
        1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w),
        2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w),
        2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y),
    ]
    columns = []
    for column in range(3):
        for row in range(3):
            columns.append(rotation[column * 3 + row] * scale[column])
        columns.append(0.0)
    columns.extend([*translation, 1.0])
    return columns


def multiply(left: list[float], right: list[float]) -> list[float]:
    result = [0.0] * 16
    for column in range(4):
        for row in range(4):
            result[column * 4 + row] = sum(
                left[step * 4 + row] * right[column * 4 + step] for step in range(4)
            )
    return result


def transform(matrix: list[float], point: tuple[float, ...]) -> tuple[float, float, float]:
    x, y, z = point[0], point[1], point[2]
    return (
        matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12],
        matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13],
        matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14],
    )


def collect_nodes(document: dict, binary: bytes, report: ModelReport) -> None:
    nodes = document.get("nodes", [])
    meshes = document.get("meshes", [])
    scene = document.get("scenes", [{}])[document.get("scene", 0)]
    roots = scene.get("nodes", list(range(len(nodes))))
    identity = [1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, 1.0]
    seen: set[int] = set()
    names: set[str] = set()
    stack = [(index, identity, BASE) for index in roots]
    while stack:
        index, parent, inherited = stack.pop()
        if index in seen:
            report.fail(f"node {index} is referenced more than once; the graph must be a tree")
            continue
        seen.add(index)
        node = nodes[index]
        name = node.get("name", "")
        if not name:
            report.fail(f"node {index} has no name, so it cannot be classified")
        if name in names:
            report.fail(f"node name {name!r} is used twice")
        names.add(name)
        matrix = multiply(parent, node_matrix(node))
        kind = classify(name, inherited)
        if "mesh" in node:
            positions: list[tuple[float, float, float]] = []
            triangles: list[tuple[int, int, int]] = []
            for primitive in meshes[node["mesh"]].get("primitives", []):
                if primitive.get("mode", 4) != 4:
                    report.fail(f"{name}: primitive is not drawn as triangles")
                    continue
                attributes = primitive.get("attributes", {})
                for required in ("POSITION", "NORMAL"):
                    if required not in attributes:
                        report.fail(f"{name}: primitive has no {required}")
                for attribute in ("POSITION", "NORMAL"):
                    if attribute in attributes:
                        accessor = document["accessors"][attributes[attribute]]
                        if accessor["componentType"] != FLOAT or accessor["type"] != "VEC3":
                            report.fail(f"{name}: {attribute} is not a float VEC3 accessor")
                        if "sparse" in accessor:
                            report.fail(f"{name}: {attribute} uses a sparse accessor")
                if "POSITION" not in attributes:
                    continue
                offset = len(positions)
                positions.extend(
                    transform(matrix, point)
                    for point in read_accessor(document, binary, attributes["POSITION"])
                )
                if "indices" in primitive:
                    accessor = document["accessors"][primitive["indices"]]
                    if accessor["componentType"] not in INDEX_TYPES:
                        report.fail(f"{name}: indices are not an unsigned integer type")
                    flat = [value[0] for value in read_accessor(document, binary, primitive["indices"])]
                else:
                    flat = list(range(len(positions) - offset))
                triangles.extend(
                    (flat[step] + offset, flat[step + 1] + offset, flat[step + 2] + offset)
                    for step in range(0, len(flat) - 2, 3)
                )
            report.nodes.append(NodeGeometry(name, kind, positions, triangles))
        for child in node.get("children", []):
            stack.append((child, matrix, kind))


def check_document(document: dict, report: ModelReport) -> None:
    if document.get("asset", {}).get("version") != "2.0":
        report.fail("asset version is not 2.0")
    for buffer in document.get("buffers", []):
        if "uri" in buffer:
            report.fail("a buffer has a uri; the file must be self-contained")
    for image in document.get("images", []):
        if "uri" in image:
            report.fail("an image has a uri; the file must be self-contained")
    if document.get("images"):
        report.notes.append("carries images, which the reader throws away")
    if document.get("animations"):
        report.notes.append("carries animations, which the reader throws away")


def check_geometry(report: ModelReport, hung: bool) -> None:
    if not report.nodes:
        report.fail("no drawable nodes")
        return
    if report.triangle_count() > MAX_TRIANGLES:
        report.fail(f"{report.triangle_count()} triangles, over the {MAX_TRIANGLES} limit")
    for node in report.nodes:
        if not node.triangles:
            report.fail(f"{node.name}: no triangles")
            continue
        volume = node.signed_volume()
        if volume <= 0.0:
            report.fail(f"{node.name}: encloses {volume:.6f} m³, so its faces are wound inside out")
    low, high = report.bounds()
    if max(high[axis] - low[axis] for axis in range(3)) > 12.0:
        report.fail("model is over 12 m across; check the units")
    moving = report.kind_bounds({YOKE, HEAD})
    if moving is not None:
        centre_x = (moving[0][0] + moving[1][0]) / 2
        centre_z = (moving[0][2] + moving[1][2]) / 2
        if abs(centre_x) > 0.006 or abs(centre_z) > 0.006:
            report.fail(
                f"parts that pan are centred at x={centre_x:.3f}, z={centre_z:.3f}; "
                "the pan axis is x=0, z=0"
            )
    head = report.kind_bounds({HEAD})
    yoke = report.kind_bounds({YOKE})
    if head is not None and yoke is not None:
        pivot = (head[0][1] + head[1][1]) / 2
        if not yoke[0][1] - 0.001 <= pivot <= yoke[1][1] + 0.001:
            report.fail(f"the tilt axis at y={pivot:.3f} is outside the yoke arms")
    if hung and high[1] > 0.16:
        report.fail(f"geometry reaches {high[1]:.3f} m above the mounting point")
    if hung and low[1] > -0.05:
        report.fail("nothing hangs below the mounting point")


def check_bracket_travel(report: ModelReport, swivel: dict) -> None:
    """Swing the body about its hinge and see whether it stays inside its own bracket.

    A hanging frame is only a bracket if the lantern in it can actually be aimed. The
    builder cuts the frame to clear the body it measured and records that clearance; this
    rotates every node that is not the frame or the rigging through the declared travel
    and fails if any of it reaches past. What that catches is geometry added *after* the
    frame was placed — an accessory the arms were never sized for.

    Working in glTF axes, so a bracket tilts about `x` and the height is `y`.
    """

    clearance = swivel.get("clearance_metres")
    if clearance is None:
        return
    body = [
        node
        for node in report.nodes
        if node.name != swivel["node"] and not node.name.startswith("truss-coupler")
    ]
    if not body:
        return

    _, pivot_height, pivot_depth = swivel["point_metres"]
    low, high = (math.radians(angle) for angle in swivel["range_degrees"])
    # Sampled across the sweep, not only at its ends: a box corner reaches highest part
    # way through the travel.
    steps = 24
    highest = -math.inf
    for step in range(steps + 1):
        angle = low + (high - low) * step / steps
        cos, sin = math.cos(angle), math.sin(angle)
        for node in body:
            for point in node.positions:
                depth = point[2] - pivot_depth
                height = point[1] - pivot_height
                highest = max(highest, height * cos - depth * sin)
    if highest > clearance + 1e-4:
        report.fail(
            f"{swivel['node']}: the body reaches {highest:.3f} m above the hinge at full "
            f"travel but the bracket was cut for {clearance:.3f} m, so it cannot turn"
        )


def verify(path: Path, entry: dict | None) -> ModelReport:
    report = ModelReport(path=path)
    data = path.read_bytes()
    split = split_chunks(data, report)
    if split is None:
        return report
    document, binary = split
    check_document(document, report)
    collect_nodes(document, binary, report)
    check_geometry(report, bool(entry) and entry["origin"].startswith("mounting point"))
    for swivel in (entry or {}).get("swivels", []):
        check_bracket_travel(report, swivel)
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--quiet", action="store_true")
    arguments = parser.parse_args()

    manifest_path = arguments.models / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {"models": []}
    declared = {entry["file"]: entry for entry in manifest["models"]}

    files = sorted(arguments.models.rglob("*.glb"))
    if not files:
        print(f"verify error: no models under {arguments.models}", file=sys.stderr)
        return 1

    failures = 0
    for path in files:
        relative = path.relative_to(arguments.models).as_posix()
        entry = declared.get(relative)
        report = verify(path, entry)
        if entry is None:
            report.fail("not listed in manifest.json")
        else:
            low, high = report.bounds()
            measured = {
                "width_millimetres": round((high[0] - low[0]) * 1000, 1),
                "height_millimetres": round((high[1] - low[1]) * 1000, 1),
                "depth_millimetres": round((high[2] - low[2]) * 1000, 1),
            }
            for key, value in measured.items():
                if abs(value - entry[key]) > 1.0:
                    report.fail(f"{key} is {value} but the manifest says {entry[key]}")
            if report.triangle_count() != entry["triangles"]:
                report.fail(
                    f"{report.triangle_count()} triangles but the manifest says {entry['triangles']}"
                )
        if report.problems:
            failures += 1
            for problem in report.problems:
                print(f"verify error: {relative}: {problem}", file=sys.stderr)
        elif not arguments.quiet:
            low, high = report.bounds()
            size = " × ".join(f"{(high[axis] - low[axis]) * 1000:.0f}" for axis in (0, 1, 2))
            print(f"ok {relative}: {size} mm, {report.triangle_count()} triangles")

    print(f"{len(files) - failures}/{len(files)} models satisfy the import contract")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
