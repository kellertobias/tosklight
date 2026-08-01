"""Flight-case racks: the black boxes a rig is actually made of.

Nineteen-inch racks, open front and back with gear in them, because a closed lid is a
featureless slab and what an operator recognises is the row of front faces. Sizes follow
the U counts a touring stock carries; the tall ones roll and the short ones do not.

These stand on the floor, so the origin is on the floor under the middle of the case.
"""

from __future__ import annotations

from .kit import BASE, HOUSING_BLACK, LENS_CLEAR, Model, Part, SILVER

RACK_UNIT = 44.45
CASE_BLACK = "#232326"
EXTRUSION = "#8E9298"
RACK_WIDTH = 600.0
RACK_DEPTH = 640.0
WHEEL_HEIGHT = 100.0

# The gear that ends up in a rack, as (units, face colour, how the front is broken up).
# Indexing this list by position rather than drawing at random keeps a rebuild identical.
GEAR = (
    (2, "#2C2F34", "amplifier"),
    (1, "#1E2126", "patch"),
    (3, "#2A2D33", "amplifier"),
    (1, "#33373D", "drawer"),
    (2, "#212429", "processor"),
    (1, "#2C2F34", "patch"),
    (4, "#26292E", "amplifier"),
    (2, "#1E2126", "processor"),
    (1, "#33373D", "drawer"),
    (3, "#2A2D33", "amplifier"),
)


def _gear_face(face: Part, detail: Part, kind: str, width: float, height: float, at: tuple[float, float, float]) -> None:
    """Break a blank front up enough that a rack does not read as a stack of bricks."""

    x, y, z = at
    if kind == "amplifier":
        for side in (-1, 1):
            detail.cylinder(min(56.0, height * 0.6), 12, (x + side * width * 0.3, y - 6, z), segments=10, rotation=(90, 0, 0))
        detail.box((width * 0.3, 8, height * 0.35), (x, y - 4, z))
    elif kind == "processor":
        detail.box((width * 0.42, 8, height * 0.3), (x - width * 0.16, y - 4, z + height * 0.1))
        for step in range(4):
            detail.cylinder(22, 10, (x + width * 0.16 + step * 34, y - 5, z - height * 0.15), segments=8, rotation=(90, 0, 0))
    elif kind == "drawer":
        detail.box((width * 0.9, 8, height * 0.45), (x, y - 4, z))
    else:
        for step in range(8):
            detail.cylinder(20, 12, (x - width * 0.4 + step * width * 0.11, y - 6, z), segments=8, rotation=(90, 0, 0))
    face.box((width, 26, height - 4), (x, y + 9, z))


def _rack(units: int, wheels: bool) -> Model:
    inner_height = units * RACK_UNIT
    lid = 90.0
    base = WHEEL_HEIGHT if wheels else 30.0
    model = Model(
        f"rack-{units:02d}u{'-wheels' if wheels else ''}",
        "cases",
        f"Flight case rack, {units}U{' on castors' if wheels else ''}: "
        f"{RACK_WIDTH:.0f} x {RACK_DEPTH:.0f} shell, open front and back",
        origin="floor level, centred under the case",
    )
    top = base + lid + inner_height + lid

    shell = model.part("case-shell", CASE_BLACK)
    shell.box((RACK_WIDTH, RACK_DEPTH, lid), (0, 0, base + lid / 2))
    shell.box((RACK_WIDTH, RACK_DEPTH, lid), (0, 0, top - lid / 2))
    for side in (-1, 1):
        shell.box((26, RACK_DEPTH, inner_height), (side * (RACK_WIDTH / 2 - 13), 0, base + lid + inner_height / 2))

    extrusion = model.part("case-extrusions", EXTRUSION, BASE, shell)
    for level in (base + 4, top - 4):
        for side in (-1, 1):
            extrusion.box((RACK_WIDTH + 8, 22, 22), (0, side * (RACK_DEPTH / 2 - 11), level))
            extrusion.box((22, RACK_DEPTH + 8, 22), (side * (RACK_WIDTH / 2 - 11), 0, level))
    corners = model.part("case-corners", EXTRUSION, BASE, shell)
    for x in (-1, 1):
        for y in (-1, 1):
            for z in (base + 10, top - 10):
                corners.box((60, 60, 60), (x * (RACK_WIDTH / 2 - 20), y * (RACK_DEPTH / 2 - 20), z))
    fittings = model.part("case-fittings", SILVER, BASE, shell)
    for side in (-1, 1):
        fittings.box((120, 30, 60), (side * (RACK_WIDTH / 2 - 60), -RACK_DEPTH / 2 - 12, base + lid / 2))
        fittings.box((90, 26, 90), (side * 150, -RACK_DEPTH / 2 - 10, top - lid / 2))

    rails = model.part("rack-rails", SILVER)
    for side in (-1, 1):
        for face in (-1, 1):
            rails.box((16, 30, inner_height), (side * 232, face * (RACK_DEPTH / 2 - 90), base + lid + inner_height / 2))

    faces = model.part("rack-gear", CASE_BLACK)
    detail = model.part("gear-detail", SILVER, BASE, faces)
    lights = model.part("gear-lights", LENS_CLEAR, BASE, faces)
    filled = 0
    index = 0
    while filled < units:
        gear_units, colour, kind = GEAR[index % len(GEAR)]
        index += 1
        if filled + gear_units > units:
            gear_units = units - filled
        height = gear_units * RACK_UNIT
        centre = base + lid + filled * RACK_UNIT + height / 2
        _gear_face(faces, detail, kind, 464.0, height, (0, -RACK_DEPTH / 2 + 40, centre))
        lights.cylinder(12, 8, (-200, -RACK_DEPTH / 2 + 32, centre + height / 2 - 16), segments=6, rotation=(90, 0, 0))
        filled += gear_units

    if wheels:
        castors = model.part("castors", HOUSING_BLACK)
        for x in (-1, 1):
            for y in (-1, 1):
                position = (x * (RACK_WIDTH / 2 - 80), y * (RACK_DEPTH / 2 - 80), 0.0)
                castors.box((70, 70, 34), (position[0], position[1], base - 17))
                castors.cylinder(
                    72,
                    28,
                    (position[0], position[1], 36),
                    segments=12,
                    rotation=(0, 90, 0),
                )
    else:
        feet = model.part("case-feet", HOUSING_BLACK)
        for x in (-1, 1):
            for y in (-1, 1):
                feet.box((80, 80, base), (x * (RACK_WIDTH / 2 - 60), y * (RACK_DEPTH / 2 - 60), base / 2))
    return model


def models() -> list[Model]:
    """Rolling racks for the big ones, footed cases for the small ones."""

    return [
        _rack(8, True),
        _rack(14, True),
        _rack(18, True),
        _rack(2, False),
        _rack(4, False),
        _rack(6, False),
    ]
