"""Geometry kit for the ToskLight fixture and stage model set.

Everything here exists to satisfy ``docs/engineering/fixture-and-stage-model-brief.md``:
one self-contained GLB per model, authored in metres, +Y up in glTF terms, a lamp
pointing straight down at rest, the origin at the mounting point, and node names that
tell the visualizer's reader which parts pan and which parts tilt.

Two conventions matter when reading the builders:

* **Millimetres in, metres out.** Every builder writes the numbers from the brief;
  this module divides by a thousand when it hands vertices to Blender.
* **Blender space, not glTF space.** Blender is Z-up, so models are authored with
  ``+X`` stage right, ``+Y`` upstage (away from the audience) and ``+Z`` up. The
  exporter's *+Y up* conversion maps ``(x, y, z)`` to glTF ``(x, z, -y)``, which puts
  the audience at glTF ``+Z`` and points a resting lamp along glTF ``-Y``. So a lamp
  is built pointing down ``-Z`` here, and the front of a body faces ``-Y``.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from pathlib import Path

import bpy

MM = 0.001

# Section 2 of the brief: the colours a model is built from.
HOUSING_BLACK = "#1A1A1C"
# Modern moving heads and LED gear are grey castings rather than true black, and a lamp that
# dark loses its own silhouette in a render. The cheap procedural proxies use this tone and it
# is why their shapes read where the first pass of these models did not.
HOUSING_DARK = "#2A313A"
SILVER = "#B8BCC0"
LENS_CLEAR = "#C8D2DC"
BRASS = "#8A7040"
CABLE_BLACK = "#0E0E10"
WHITE_DIFFUSER = "#E6E9EC"
MIRROR_TILE = "#EDF1F5"

BASE = "base"
YOKE = "yoke"
HEAD = "head"

# --------------------------------------------------------------------------------------
# Finishes. Colour alone cannot tell a wool drape from a polished half-coupler, and a rig in
# which the fabric catches the same highlight as the aluminium reads as painted cardboard.
# A finish is a (metallic, roughness) pair; the glTF exporter writes both, the desk's Three.js
# stage shades with them directly, and the standalone visualizer reads them back off the file.
# --------------------------------------------------------------------------------------

METAL = "metal"
PAINT = "paint"
PLASTIC = "plastic"
GLASS = "glass"
DIFFUSER = "diffuser"
FABRIC = "fabric"
SKIN = "skin"

FINISHES: dict[str, tuple[float, float]] = {
    METAL: (0.9, 0.32),
    PAINT: (0.35, 0.55),
    PLASTIC: (0.0, 0.65),
    GLASS: (0.0, 0.12),
    DIFFUSER: (0.0, 0.72),
    FABRIC: (0.0, 0.96),
    SKIN: (0.0, 0.8),
}

# What a colour is made of when a part does not say. Every metallic colour in the palette is
# bare metal, everything else is a painted or moulded housing.
DEFAULT_FINISH: dict[str, str] = {
    SILVER: METAL,
    BRASS: METAL,
    MIRROR_TILE: METAL,
    LENS_CLEAR: GLASS,
    WHITE_DIFFUSER: DIFFUSER,
    CABLE_BLACK: PLASTIC,
}

# The node every flown model puts its rigging under, so a desk can switch it off when
# the fixture is not on a bar. Named once here because the manifest reports it and the
# builders all have to spell it the same way.
COUPLER = "truss-coupler"
# What the same model is called once its clamps are taken off. Every flown model ships both.
UNRIGGED_SUFFIX = "-no-clamp"

# How wide a row of the review file gets before it wraps, in metres. Laid out in one line
# the set runs to 170 m and nothing is legible next to the six-metre drapes.
REVIEW_ROW_WIDTH = 12.0

Vector3 = tuple[float, float, float]
Face = tuple[int, ...]


# --------------------------------------------------------------------------------------
# Node classification — the reader's rules, mirrored so a mistake fails the build here.
# --------------------------------------------------------------------------------------


def classify(name: str, inherited: str = BASE) -> str:
    """Classify a node exactly as ``viz_scene::ModelPartKind::from_node_name`` does."""

    folded = name.lower()
    if "head" in folded or "lamp" in folded or "tilt" in folded:
        return HEAD
    if "yoke" in folded or "arm" in folded or "pan" in folded:
        return YOKE
    return inherited


# --------------------------------------------------------------------------------------
# Primitives. Every primitive is a closed shell wound counter-clockwise seen from outside,
# so exported normals point out without a fix-up pass.
# --------------------------------------------------------------------------------------


def _rotation_matrix(rotation: Vector3) -> tuple[Vector3, Vector3, Vector3]:
    rx, ry, rz = (math.radians(angle) for angle in rotation)
    cx, sx = math.cos(rx), math.sin(rx)
    cy, sy = math.cos(ry), math.sin(ry)
    cz, sz = math.cos(rz), math.sin(rz)
    return (
        (cy * cz, cz * sx * sy - cx * sz, cx * cz * sy + sx * sz),
        (cy * sz, cx * cz + sx * sy * sz, -cz * sx + cx * sy * sz),
        (-sy, cy * sx, cx * cy),
    )


def _place(
    vertices: list[Vector3],
    at: Vector3,
    rotation: Vector3 | None,
    scale: Vector3 | None,
) -> list[Vector3]:
    if scale is not None:
        vertices = [(x * scale[0], y * scale[1], z * scale[2]) for x, y, z in vertices]
    if rotation is not None:
        row_x, row_y, row_z = _rotation_matrix(rotation)
        vertices = [
            (
                row_x[0] * x + row_x[1] * y + row_x[2] * z,
                row_y[0] * x + row_y[1] * y + row_y[2] * z,
                row_z[0] * x + row_z[1] * y + row_z[2] * z,
            )
            for x, y, z in vertices
        ]
    return [(x + at[0], y + at[1], z + at[2]) for x, y, z in vertices]


def box_shell(size: Vector3, top: Vector3 | None = None) -> tuple[list[Vector3], list[Face]]:
    """A cuboid, or a tapered one when ``top`` gives a different width and depth."""

    hx, hy, hz = size[0] / 2, size[1] / 2, size[2] / 2
    tx, ty = (top[0] / 2, top[1] / 2) if top else (hx, hy)
    vertices = [
        (-hx, -hy, -hz),
        (hx, -hy, -hz),
        (hx, hy, -hz),
        (-hx, hy, -hz),
        (-tx, -ty, hz),
        (tx, -ty, hz),
        (tx, ty, hz),
        (-tx, ty, hz),
    ]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    return vertices, faces


def plate_shell(width: float, height: float) -> tuple[list[Vector3], list[Face]]:
    """A single flat quad facing +Z.

    The one open surface in the kit, for the mirror-ball facets: a tile is seen only from
    outside the ball it is stuck to, and giving each of five thousand of them a back and
    four edges costs six times the triangles for something nobody can see.
    """

    half_width, half_height = width / 2, height / 2
    vertices = [
        (-half_width, -half_height, 0.0),
        (half_width, -half_height, 0.0),
        (half_width, half_height, 0.0),
        (-half_width, half_height, 0.0),
    ]
    return vertices, [(0, 1, 2, 3)]


def _ring(radius: float, height: float, segments: int, phase: float = 0.0) -> list[Vector3]:
    step = math.tau / segments
    return [
        (radius * math.cos(phase + index * step), radius * math.sin(phase + index * step), height)
        for index in range(segments)
    ]


def cylinder_shell(
    radius: float,
    length: float,
    segments: int = 24,
    top_radius: float | None = None,
) -> tuple[list[Vector3], list[Face]]:
    """A capped cylinder or frustum along +Z, centred on the origin."""

    top_radius = radius if top_radius is None else top_radius
    bottom = _ring(radius, -length / 2, segments)
    top = _ring(top_radius, length / 2, segments)
    vertices = bottom + top + [(0.0, 0.0, -length / 2), (0.0, 0.0, length / 2)]
    bottom_centre, top_centre = 2 * segments, 2 * segments + 1
    faces: list[Face] = []
    for index in range(segments):
        following = (index + 1) % segments
        faces.append((index, following, segments + following, segments + index))
        faces.append((bottom_centre, following, index))
        faces.append((top_centre, segments + index, segments + following))
    return vertices, faces


def tube_shell(
    outer: float,
    inner: float,
    length: float,
    segments: int = 24,
    phase: float = 0.0,
) -> tuple[list[Vector3], list[Face]]:
    """An open-ended tube with wall thickness: the ring or bezel of section 2.

    At a low segment count it is a polygonal frame rather than a ring, which is how the
    octagonal colour-frame holders of the PAR family are made: eight segments with the
    phase set half a step round so a flat, not a corner, faces up.
    """

    rings = [
        _ring(outer, -length / 2, segments, phase),
        _ring(outer, length / 2, segments, phase),
        _ring(inner, length / 2, segments, phase),
        _ring(inner, -length / 2, segments, phase),
    ]
    vertices = [vertex for ring in rings for vertex in ring]
    faces: list[Face] = []
    for index in range(segments):
        following = (index + 1) % segments
        for level in range(4):
            lower = level * segments
            upper = ((level + 1) % 4) * segments
            faces.append((lower + index, lower + following, upper + following, upper + index))
    return vertices, faces


def sphere_shell(radius: float, segments: int = 16, rings: int = 8) -> tuple[list[Vector3], list[Face]]:
    """A latitude/longitude sphere, flat shaded like everything else here."""

    vertices: list[Vector3] = [(0.0, 0.0, -radius)]
    for ring in range(1, rings):
        polar = math.pi * ring / rings
        vertices.extend(_ring(radius * math.sin(polar), -radius * math.cos(polar), segments))
    vertices.append((0.0, 0.0, radius))
    top = len(vertices) - 1
    faces: list[Face] = []
    for index in range(segments):
        following = (index + 1) % segments
        faces.append((0, 1 + following, 1 + index))
        faces.append((top, top - segments + index, top - segments + following))
    for ring in range(rings - 2):
        lower = 1 + ring * segments
        upper = lower + segments
        for index in range(segments):
            following = (index + 1) % segments
            faces.append((lower + index, lower + following, upper + following, upper + index))
    return vertices, faces


def torus_shell(
    major: float,
    minor: float,
    segments: int = 20,
    sides: int = 8,
    arc: float = 360.0,
) -> tuple[list[Vector3], list[Face]]:
    """A ring around +Z, or an arc of one when ``arc`` is less than a full turn."""

    closed = arc >= 359.999
    rings = segments if closed else segments + 1
    span = math.radians(arc)
    vertices: list[Vector3] = []
    for ring in range(rings):
        angle = span * ring / segments
        cos_a, sin_a = math.cos(angle), math.sin(angle)
        for side in range(sides):
            theta = math.tau * side / sides
            radial = major + minor * math.cos(theta)
            vertices.append((radial * cos_a, radial * sin_a, minor * math.sin(theta)))
    faces: list[Face] = []
    for ring in range(segments):
        lower = (ring % rings) * sides
        upper = ((ring + 1) % rings) * sides
        for side in range(sides):
            following = (side + 1) % sides
            faces.append((lower + side, upper + side, upper + following, lower + following))
    if not closed:
        faces.append(tuple(range(sides)))
        faces.append(tuple(reversed(range(segments * sides, segments * sides + sides))))
    return vertices, faces


def revolve_shell(
    outline: list[tuple[float, float]],
    segments: int = 24,
) -> tuple[list[Vector3], list[Face]]:
    """Spin a ``(radius, height)`` outline around +Z.

    This is what makes a shape read as moulded rather than assembled: filleted rims,
    parabolic reflectors, half-ball backs and tapering heads are all one outline.

    Walk the outline the way a finger would travel over the surface, keeping the solid
    on your left — for a lamp pointing down that means starting at the centre of the
    front face, out across it, up the outside, and in across the back. Walked that way
    the faces come out wound outwards. A radius of zero at either end closes as a pole;
    anything else is capped flat.
    """

    rings: list[list[int]] = []
    vertices: list[Vector3] = []
    for radius, height in outline:
        if radius <= 0.0:
            rings.append([len(vertices)])
            vertices.append((0.0, 0.0, height))
            continue
        rings.append([len(vertices) + step for step in range(segments)])
        vertices.extend(_ring(radius, height, segments))

    faces: list[Face] = []
    for lower, upper in zip(rings, rings[1:]):
        for index in range(segments):
            following = (index + 1) % segments
            if len(lower) == 1:
                faces.append((lower[0], upper[following], upper[index]))
            elif len(upper) == 1:
                faces.append((upper[0], lower[index], lower[following]))
            else:
                faces.append((lower[index], lower[following], upper[following], upper[index]))
    for ring, closing in ((rings[0], False), (rings[-1], True)):
        if len(ring) == 1:
            continue
        centre = len(vertices)
        vertices.append((0.0, 0.0, outline[-1 if closing else 0][1]))
        for index in range(segments):
            following = (index + 1) % segments
            faces.append(
                (centre, ring[index], ring[following])
                if closing
                else (centre, ring[following], ring[index])
            )
    return vertices, faces


def arc_outline(
    centre: tuple[float, float],
    radius: float,
    start: float,
    end: float,
    steps: int = 4,
    rise: float | None = None,
) -> list[tuple[float, float]]:
    """Points along an arc, for filleting a rim inside a revolve outline.

    ``rise`` makes it elliptical: a head whose back is a hemisphere looks like a light
    bulb, where the same shoulder over a shallower rise looks like a moulded housing.
    """

    rise = radius if rise is None else rise
    return [
        (
            centre[0] + radius * math.cos(math.radians(start + (end - start) * step / steps)),
            centre[1] + rise * math.sin(math.radians(start + (end - start) * step / steps)),
        )
        for step in range(steps + 1)
    ]


def profile_shell(
    section: list[tuple[float, float]],
    length: float,
) -> tuple[list[Vector3], list[Face]]:
    """Extrude a closed 2D section in the XZ plane along Y, capped at both ends.

    The section is given counter-clockwise in ``(x, z)``; it is what the LED strip,
    the truss chord layout and the curtain folds are made of.
    """

    count = len(section)
    front = [(x, -length / 2, z) for x, z in section]
    back = [(x, length / 2, z) for x, z in section]
    faces: list[Face] = []
    for index in range(count):
        following = (index + 1) % count
        faces.append((index, count + index, count + following, following))
    faces.append(tuple(range(count)))
    faces.append(tuple(reversed(range(count, 2 * count))))
    return front + back, faces


# --------------------------------------------------------------------------------------
# Parts and models
# --------------------------------------------------------------------------------------


@dataclass
class Part:
    """One named node of a model, drawn in one colour.

    Geometry is accumulated in model space in millimetres; the object itself stays at
    the origin, because the reader bakes node transforms into the vertices anyway.
    """

    name: str
    colour: str
    kind: str
    parent: "Part | None" = None
    finish: str = ""
    vertices: list[Vector3] = field(default_factory=list)
    faces: list[Face] = field(default_factory=list)

    def surface(self) -> str:
        return self.finish or DEFAULT_FINISH.get(self.colour, PAINT)

    def add(
        self,
        shell: tuple[list[Vector3], list[Face]],
        at: Vector3 = (0.0, 0.0, 0.0),
        rotation: Vector3 | None = None,
        scale: Vector3 | None = None,
    ) -> "Part":
        vertices, faces = shell
        offset = len(self.vertices)
        self.vertices.extend(_place(vertices, at, rotation, scale))
        self.faces.extend(tuple(index + offset for index in face) for face in faces)
        return self

    def box(self, size: Vector3, at: Vector3, **placement) -> "Part":
        return self.add(box_shell(size), at, **placement)

    def taper(self, size: Vector3, top: Vector3, at: Vector3, **placement) -> "Part":
        return self.add(box_shell(size, top), at, **placement)

    def cylinder(self, diameter: float, length: float, at: Vector3, segments: int = 24, **placement) -> "Part":
        return self.add(cylinder_shell(diameter / 2, length, segments), at, **placement)

    def cone(
        self,
        diameter: float,
        top_diameter: float,
        length: float,
        at: Vector3,
        segments: int = 24,
        **placement,
    ) -> "Part":
        return self.add(cylinder_shell(diameter / 2, length, segments, top_diameter / 2), at, **placement)

    def tube(
        self,
        diameter: float,
        bore: float,
        length: float,
        at: Vector3,
        segments: int = 24,
        phase: float = 0.0,
        **placement,
    ) -> "Part":
        return self.add(tube_shell(diameter / 2, bore / 2, length, segments, phase), at, **placement)

    def polygon_frame(
        self,
        across: float,
        bore: float,
        length: float,
        at: Vector3,
        sides: int = 8,
        **placement,
    ) -> "Part":
        """A flat-topped polygonal frame: the octagonal colour-frame holder of a PAR.

        ``across`` and ``bore`` are corner to corner. Phased half a step so the frame sits on
        a flat side, which is what stops an octagon from reading as a badly drawn circle.
        """

        return self.tube(across, bore, length, at, segments=sides, phase=math.pi / sides, **placement)

    def sphere(self, diameter: float, at: Vector3, segments: int = 12, rings: int = 6, **placement) -> "Part":
        return self.add(sphere_shell(diameter / 2, segments, rings), at, **placement)

    def torus(
        self,
        diameter: float,
        thickness: float,
        at: Vector3,
        segments: int = 20,
        sides: int = 8,
        arc: float = 360.0,
        **placement,
    ) -> "Part":
        return self.add(torus_shell(diameter / 2, thickness / 2, segments, sides, arc), at, **placement)

    def strut(
        self,
        diameter: float,
        start: Vector3,
        end: Vector3,
        segments: int = 6,
    ) -> "Part":
        """A round member running between two points: a truss brace or a rail."""

        span = tuple(end[axis] - start[axis] for axis in range(3))
        length = math.sqrt(sum(value * value for value in span))
        if length <= 0.0:
            raise ValueError("a strut needs two different ends")
        middle = tuple((start[axis] + end[axis]) / 2 for axis in range(3))
        polar = math.degrees(math.acos(max(-1.0, min(1.0, span[2] / length))))
        azimuth = math.degrees(math.atan2(span[1], span[0]))
        return self.cylinder(diameter, length, middle, segments, rotation=(0.0, polar, azimuth))

    def flare(
        self,
        diameter: float,
        end_diameter: float,
        start: Vector3,
        end: Vector3,
        segments: int = 12,
    ) -> "Part":
        """A tapering member between two points: a saxophone bell, a stand column."""

        span = tuple(end[axis] - start[axis] for axis in range(3))
        length = math.sqrt(sum(value * value for value in span))
        if length <= 0.0:
            raise ValueError("a flare needs two different ends")
        middle = tuple((start[axis] + end[axis]) / 2 for axis in range(3))
        polar = math.degrees(math.acos(max(-1.0, min(1.0, span[2] / length))))
        azimuth = math.degrees(math.atan2(span[1], span[0]))
        return self.cone(
            diameter,
            end_diameter,
            length,
            middle,
            segments,
            rotation=(0.0, polar, azimuth),
        )

    def plate(self, width: float, height: float, at: Vector3, **placement) -> "Part":
        return self.add(plate_shell(width, height), at, **placement)

    def revolve(
        self,
        outline: list[tuple[float, float]],
        at: Vector3,
        segments: int = 24,
        **placement,
    ) -> "Part":
        return self.add(revolve_shell(outline, segments), at, **placement)

    def dome(self, diameter: float, height: float, at: Vector3, segments: int = 20, **placement) -> "Part":
        """Half a ball, flat side down, sitting on ``at``: a PAR's back, a bulb's crown."""

        radius = diameter / 2
        outline = [(0.0, 0.0), (radius, 0.0)]
        outline += [
            (radius * math.cos(math.radians(step * 90 / 6)), height * math.sin(math.radians(step * 90 / 6)))
            for step in range(1, 7)
        ]
        return self.add(revolve_shell(outline, segments), at, **placement)

    def hollow_can(
        self,
        diameter: float,
        bore: float,
        length: float,
        dome: float,
        at: Vector3,
        segments: int = 24,
        **placement,
    ) -> "Part":
        """An open-fronted can: outer wall, domed rear cap, and a visible inner wall.

        Sitting on ``at`` and rising ``length`` along +Z, so a lamp pointing down puts ``at``
        at its front rim. A PAR is a piece of rolled tube with a lamp inside it, and a solid
        cylinder gives an operator no way to see the reflector or tell the front from the back.
        """

        outer, inner = diameter / 2, bore / 2
        shoulder = length - dome
        outline = [
            *arc_outline((0.0, shoulder), inner, 90.0, 0.0, steps=4, rise=dome * inner / outer),
            (inner, 0.0),
            (outer, 0.0),
            *arc_outline((0.0, shoulder), outer, 0.0, 90.0, steps=4, rise=dome),
        ]
        return self.add(revolve_shell(outline, segments), at, **placement)

    def parabolic_cup(
        self,
        mouth: float,
        depth: float,
        wall: float,
        at: Vector3,
        segments: int = 16,
        steps: int = 5,
        **placement,
    ) -> "Part":
        """A reflector whose profile is a parabola, open at +Z and lined inside.

        A straight-sided cone reads as a funnel; the curve is what makes a blinder cell
        or a PAR reflector look like it was pressed rather than folded.
        """

        radius = mouth / 2
        outer = [
            (radius * step / steps, depth * (step / steps) ** 2)
            for step in range(steps + 1)
        ]
        inner = [
            ((radius - wall) * step / steps, depth * (step / steps) ** 2 + wall)
            for step in range(steps, -1, -1)
        ]
        return self.add(revolve_shell(outer + [(radius, depth + wall)] + inner, segments), at, **placement)

    def extrusion(self, section: list[tuple[float, float]], length: float, at: Vector3, **placement) -> "Part":
        return self.add(profile_shell(section, length), at, **placement)

    def cup(
        self,
        mouth: float,
        throat: float,
        depth: float,
        wall: float,
        at: Vector3,
        segments: int = 16,
        **placement,
    ) -> "Part":
        """A reflector cup: a shell open at the mouth, so it reads from both sides.

        Used by the blinder cells and the multi-chip LED PAR, where a single-sided
        cone would vanish when the operator looks into the lamp.
        """

        outer_mouth, outer_throat = mouth / 2, throat / 2
        inner_mouth, inner_throat = outer_mouth - wall, outer_throat - wall
        rings = [
            _ring(outer_throat, -depth / 2, segments),
            _ring(outer_mouth, depth / 2, segments),
            _ring(inner_mouth, depth / 2, segments),
            _ring(inner_throat, -depth / 2 + wall, segments),
        ]
        vertices = [vertex for ring in rings for vertex in ring]
        faces: list[Face] = []
        for index in range(segments):
            following = (index + 1) % segments
            for level in range(4):
                lower = level * segments
                upper = ((level + 1) % 4) * segments
                faces.append((lower + index, lower + following, upper + following, upper + index))
        return self.add((vertices, faces), at, **placement)

    def triangle_count(self) -> int:
        return sum(len(face) - 2 for face in self.faces)


@dataclass
class Swivel:
    """A hinge or bolt a node turns on, for whoever animates the model later.

    glTF keeps no extras the reader would preserve, so this travels in the manifest
    instead: the node's name, the axis it turns about and the point that axis runs
    through, both already converted to the renderer's metres and axes, and how far the
    real hardware goes each way.
    """

    node: str
    axis: Vector3
    point: Vector3
    range_degrees: tuple[float, float]
    description: str
    # How far above the hinge the moving part may reach before it fouls what carries it,
    # in the builders' millimetres. Set where a frame was cut to clear a body, so the
    # verifier can swing the exported geometry and see whether it still fits.
    clearance: float | None = None


@dataclass
class Model:
    """One exported ``.glb``: a tree of parts sharing one origin."""

    name: str
    group: str
    summary: str
    origin: str = "mounting point at the top, body below"
    parts: list[Part] = field(default_factory=list)
    swivels: list[Swivel] = field(default_factory=list)

    def swivel(
        self,
        node: str,
        axis: Vector3,
        point: Vector3,
        range_degrees: tuple[float, float],
        description: str,
        clearance: float | None = None,
    ) -> None:
        """Record a hinge, in the millimetres and Blender axes the builders work in."""

        self.swivels.append(Swivel(node, axis, point, range_degrees, description, clearance))

    def part(
        self,
        name: str,
        colour: str,
        kind: str = BASE,
        parent: Part | None = None,
        finish: str = "",
    ) -> Part:
        if any(existing.name == name for existing in self.parts):
            raise ValueError(f"{self.name}: duplicate node name {name!r}")
        if finish and finish not in FINISHES:
            raise ValueError(f"{self.name}: node {name!r} asks for unknown finish {finish!r}")
        inherited = parent.kind if parent else BASE
        actual = classify(name, inherited)
        if actual != kind:
            raise ValueError(
                f"{self.name}: node {name!r} reads as {actual} but the builder wants {kind}; "
                "rename it — 'clamp' and 'panel' contain 'lamp' and 'pan'"
            )
        created = Part(name=name, colour=colour, kind=kind, parent=parent, finish=finish)
        self.parts.append(created)
        return created

    def group_node(self, name: str, kind: str, parent: Part | None = None) -> Part:
        """A node that carries no geometry, only a classification for its children."""

        return self.part(name, HOUSING_BLACK, kind, parent)

    def shift(self, offset: Vector3, skip: tuple[str, ...] = ()) -> None:
        """Move every part whose name does not start with one of ``skip``, hinges included.

        Builders that have to size a bracket around a body build that body about its own tilt
        axis, measure it, and then drop the whole thing to where the bracket puts it. Moving
        the geometry afterwards keeps each builder writing one set of numbers instead of two.
        """

        for part in self.parts:
            if skip and part.name.startswith(skip):
                continue
            part.vertices = [
                (x + offset[0], y + offset[1], z + offset[2]) for x, y, z in part.vertices
            ]
        moved = {part.name for part in self.parts if not (skip and part.name.startswith(skip))}
        for index, swivel in enumerate(self.swivels):
            if swivel.node not in moved:
                continue
            self.swivels[index] = Swivel(
                swivel.node,
                swivel.axis,
                tuple(swivel.point[axis] + offset[axis] for axis in range(3)),  # type: ignore[arg-type]
                swivel.range_degrees,
                swivel.description,
                swivel.clearance,
            )

    def reach(self, skip: tuple[str, ...] = ()) -> tuple[float, float, float]:
        """How far the measured parts reach from ``z = 0``: radius above it, depth, and width.

        The three numbers a bracket has to clear. A point above the tilt bolts sweeps its whole
        radius over the crossbar; a point below only ever raises its own depth, because at
        ninety degrees of tilt the depth is what is pointing up; and the width is what the arms
        have to stand outside of.
        """

        points = [
            vertex
            for part in self.parts
            if not (skip and part.name.startswith(skip))
            for vertex in part.vertices
        ]
        if not points:
            raise ValueError(f"{self.name}: nothing to measure a bracket against")
        above = max(
            (math.sqrt(y * y + z * z) for _, y, z in points if z > 0.0),
            default=0.0,
        )
        return (
            above,
            max(abs(y) for _, y, _ in points),
            max(abs(x) for x, _, _ in points),
        )

    def girth(self, band: float, skip: tuple[str, ...] = ()) -> float:
        """Half-width across the measured parts within ``band`` of ``z = 0``.

        How far out a trunnion has to reach to land on the body. A bracket's arms stand
        outside the *widest* part of a lantern — often an accessory nowhere near the tilt
        bolts — so a bolt drawn at the arm alone floats in mid-air next to a narrower body.
        """

        widths = [
            abs(x)
            for part in self.parts
            if not (skip and part.name.startswith(skip))
            for x, _, z in part.vertices
            if abs(z) <= band
        ]
        return max(widths) if widths else 0.0

    def bounds(self) -> tuple[Vector3, Vector3]:
        points = [vertex for part in self.parts for vertex in part.vertices]
        if not points:
            raise ValueError(f"{self.name}: model has no geometry")
        low = tuple(min(point[axis] for point in points) for axis in range(3))
        high = tuple(max(point[axis] for point in points) for axis in range(3))
        return low, high  # type: ignore[return-value]

    def triangle_count(self) -> int:
        return sum(part.triangle_count() for part in self.parts)


def unrigged(model: "Model") -> "Model | None":
    """The same model with its mounting hardware taken off, or ``None`` if it has none.

    A fixture is not always on a clamp. It goes on a floor base, into a moving-light case, onto
    somebody else's bracket, or straight onto a bar with the hardware the venue owns — and a plot
    that only ever draws the shipped half-coupler is wrong every one of those times. Shipping both
    is cheaper than asking a show to reach into the node tree: the desk points a profile at the
    one it means.

    The origin does not move. It is the mounting point either way — the bolt hole the clamp goes
    through — so the two files drop into a rig interchangeably.
    """

    if not any(part.name.startswith(COUPLER) for part in model.parts):
        return None
    return Model(
        name=f"{model.name}{UNRIGGED_SUFFIX}",
        group=model.group,
        summary=f"{model.summary}; no mounting hardware",
        origin=model.origin,
        parts=[part for part in model.parts if not part.name.startswith(COUPLER)],
        swivels=[swivel for swivel in model.swivels if not swivel.node.startswith(COUPLER)],
    )


# --------------------------------------------------------------------------------------
# Blender scene, materials and export
# --------------------------------------------------------------------------------------


def _linear(channel: int) -> float:
    value = channel / 255.0
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def _material(colour: str, finish: str) -> bpy.types.Material:
    name = f"{finish}{colour.lower()}"
    existing = bpy.data.materials.get(name)
    if existing is not None:
        return existing
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    red, green, blue = (_linear(int(colour[index : index + 2], 16)) for index in (1, 3, 5))
    metallic, roughness = FINISHES[finish]
    principled = material.node_tree.nodes.get("Principled BSDF")
    principled.inputs["Base Color"].default_value = (red, green, blue, 1.0)
    principled.inputs["Metallic"].default_value = metallic
    principled.inputs["Roughness"].default_value = roughness
    material.diffuse_color = (red, green, blue, 1.0)
    return material


def reset_scene() -> None:
    """Empty the file so one Blender session can write every model in the set."""

    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0


def _object_for(part: Part) -> bpy.types.Object:
    mesh = bpy.data.meshes.new(part.name)
    if part.vertices:
        mesh.from_pydata([(x * MM, y * MM, z * MM) for x, y, z in part.vertices], [], part.faces)
        mesh.validate(verbose=False)
        mesh.update()
        mesh.materials.append(_material(part.colour, part.surface()))
        return bpy.data.objects.new(part.name, mesh)
    return bpy.data.objects.new(part.name, None)


def build_scene(
    model: Model,
    reset: bool = True,
    offset: Vector3 = (0.0, 0.0, 0.0),
    collection: "bpy.types.Collection | None" = None,
    strict_names: bool = True,
) -> None:
    """Realise a model as Blender objects.

    Exporting wants one model alone in the file; the review file wants all of them at
    once, each in its own collection and shifted clear of its neighbours. The shift goes
    on the root objects only — children already inherit it — so the mesh data, and
    therefore the exported geometry, is identical either way.

    Object names become node names, so an export refuses a name Blender had to change.
    A review file holds a hundred lamps that each carry a ``rig-hook``, and there the
    suffixes Blender adds are only labels in the outliner.
    """

    if reset:
        reset_scene()
    target = collection or bpy.context.collection
    created: dict[str, bpy.types.Object] = {}
    for part in model.parts:
        obj = _object_for(part)
        target.objects.link(obj)
        if strict_names and obj.name != part.name:
            raise ValueError(f"{model.name}: Blender renamed {part.name!r} to {obj.name!r}")
        created[part.name] = obj
    for part in model.parts:
        if part.parent is not None:
            created[part.name].parent = created[part.parent.name]
        else:
            created[part.name].location = offset


def export_glb(model: Model, path: Path) -> None:
    """Write the scene with the settings section 1.6 of the brief asks for."""

    path.parent.mkdir(parents=True, exist_ok=True)
    wanted = {
        "filepath": str(path),
        "export_format": "GLB",
        "export_yup": True,
        "export_apply": True,
        "export_materials": "EXPORT",
        "export_image_format": "NONE",
        "export_texcoords": False,
        "export_normals": True,
        "export_tangents": False,
        "export_vertex_color": "NONE",
        "export_attributes": False,
        "export_animations": False,
        "export_skins": False,
        "export_morph": False,
        "export_cameras": False,
        "export_lights": False,
        "export_extras": False,
        "export_draco_mesh_compression_enable": False,
        "use_selection": False,
        "export_hierarchy_flatten_objs": False,
    }
    # The exporter's option names drift between Blender releases; pass only what this one has.
    supported = set(bpy.ops.export_scene.gltf.get_rna_type().properties.keys())
    bpy.ops.export_scene.gltf(**{key: value for key, value in wanted.items() if key in supported})


def write(model: Model, directory: Path) -> dict[str, object]:
    """Build, export, and report what a fixture profile needs to declare."""

    build_scene(model)
    path = directory / model.group / f"{model.name}.glb"
    export_glb(model, path)
    low, high = model.bounds()
    # Blender X/Y/Z becomes glTF X/Z/Y: width, height and depth as the profile states them.
    return {
        "model": model.name,
        "group": model.group,
        "file": f"{model.group}/{model.name}.glb",
        "summary": model.summary,
        "origin": model.origin,
        "width_millimetres": round(high[0] - low[0], 1),
        "height_millimetres": round(high[2] - low[2], 1),
        "depth_millimetres": round(high[1] - low[1], 1),
        "triangles": model.triangle_count(),
        # The node to hide when the fixture is not flown. Hiding it takes its children —
        # the safety bond — with it, which is why the rigging is one subtree.
        "coupler_node": COUPLER if any(part.name == COUPLER for part in model.parts) else None,
        "nodes": [
            {"name": part.name, "kind": part.kind, "finish": part.surface()}
            for part in model.parts
        ],
        "swivels": [
            {
                "node": swivel.node,
                "description": swivel.description,
                # Blender X/Y/Z becomes glTF X/Z/-Y, and millimetres become metres.
                "axis": [
                    round(swivel.axis[0], 4),
                    round(swivel.axis[2], 4),
                    round(-swivel.axis[1], 4),
                ],
                "point_metres": [
                    round(swivel.point[0] * MM, 4),
                    round(swivel.point[2] * MM, 4),
                    round(-swivel.point[1] * MM, 4),
                ],
                "range_degrees": list(swivel.range_degrees),
                "clearance_metres": (
                    None if swivel.clearance is None else round(swivel.clearance * MM, 4)
                ),
            }
            for swivel in model.swivels
        ],
    }


def write_review_file(models: list[Model], path: Path) -> None:
    """Save one ``.blend`` holding the whole set, for looking at rather than exporting.

    Every model keeps its own origin at ``z = 0``, one collection each, laid out in a row
    per group. The grid floor therefore stands in for the truss: a lamp that hangs the
    right way up hangs below it, and a deck or a hazer stands on top of it.
    """

    reset_scene()
    scene = bpy.context.scene
    row = 0.0
    for group in dict.fromkeys(model.group for model in models):
        parent = bpy.data.collections.new(group)
        scene.collection.children.link(parent)
        cursor = 0.0
        depth = 0.0
        for model in [entry for entry in models if entry.group == group]:
            low, high = model.bounds()
            width = (high[0] - low[0]) * MM
            if cursor > 0.0 and cursor + width > REVIEW_ROW_WIDTH:
                row -= depth + 1.0
                cursor, depth = 0.0, 0.0
            collection = bpy.data.collections.new(model.name)
            parent.children.link(collection)
            build_scene(
                model,
                reset=False,
                offset=(cursor - low[0] * MM, row - (low[1] + high[1]) / 2 * MM, 0.0),
                collection=collection,
                strict_names=False,
            )
            cursor += width + 0.4
            depth = max(depth, (high[1] - low[1]) * MM)
        row -= depth + 2.0
    path.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(path))
