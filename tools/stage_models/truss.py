"""Truss and rigging of part 4 of the fixture and stage model brief.

The visualizer draws trusses procedurally from a venue object's size, so these have to
match those proportions: 50 chords, 20 braces, a bay every 340 mm, chords 290 apart.
Every section lies along ``+X`` centred on the origin, so a show rotates it into place.
"""

from __future__ import annotations

import math

from .kit import BASE, FABRIC, HOUSING_BLACK, METAL, Model, PAINT, Part, SILVER

MOTOR_BLACK = "#25272B"
WARNING_YELLOW = "#E8C21A"

CHORD = 50.0
BRACE = 20.0
# 290 across the outside of the chords, so their centres sit a chord diameter closer.
# Every section type uses the same spacing; only the number of chords and where they
# sit differ, which is what makes the family read as one product.
ENVELOPE = 290.0
SPACING = ENVELOPE - CHORD
BAY = 340.0
LENGTHS = (500, 1000, 1500, 2000, 2500, 3000, 4000)

# Chord positions in the section plane, as (across, up) in millimetres.
SECTIONS: dict[str, tuple[tuple[float, float], ...]] = {
    "2-point": ((0.0, SPACING / 2), (0.0, -SPACING / 2)),
    "3-point": (
        (0.0, SPACING / math.sqrt(3.0)),
        (-SPACING / 2, -SPACING / (2 * math.sqrt(3.0))),
        (SPACING / 2, -SPACING / (2 * math.sqrt(3.0))),
    ),
    "4-point": (
        (-SPACING / 2, SPACING / 2),
        (SPACING / 2, SPACING / 2),
        (SPACING / 2, -SPACING / 2),
        (-SPACING / 2, -SPACING / 2),
    ),
}

# The corner blocks a rental stock actually carries, as the arm directions each one has.
# +X and -X run along the truss, +Y and -Y turn the corner, -Z drops a leg.
CORNERS: dict[str, tuple[tuple[int, int, int], ...]] = {
    "corner-2-way": ((-1, 0, 0), (0, 1, 0)),
    "corner-3-way-t": ((-1, 0, 0), (1, 0, 0), (0, 1, 0)),
    "corner-3-way-down": ((-1, 0, 0), (0, 1, 0), (0, 0, -1)),
    "corner-4-way-cross": ((-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0)),
    "corner-4-way-down": ((-1, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, -1)),
    "corner-5-way": ((-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1)),
    "corner-6-way": ((-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)),
}
CORNER_ARM = 500.0
# Half the node a corner block is built round: the arms stop at its faces and their chords
# mitre to each other across it. It is the chord spacing, so on a 4-point the node's edges
# and the arms' chords are the same lines and the joint comes out flush.
NODE = SPACING / 2


def _faces(count: int) -> list[tuple[int, int]]:
    if count == 2:
        return [(0, 1)]
    return [(index, (index + 1) % count) for index in range(count)]


def _lace(braces: Part, first: tuple[float, float], second: tuple[float, float], length: float) -> None:
    """One face of the ladder: an upright at every node and diagonals between them."""

    bays = max(1, round(length / BAY))
    pitch = length / bays
    nodes = [-length / 2 + index * pitch for index in range(bays + 1)]
    for x in nodes:
        braces.strut(BRACE, (x, first[0], first[1]), (x, second[0], second[1]))
    for index in range(bays):
        near, far = nodes[index], nodes[index + 1]
        if index % 2 == 0:
            braces.strut(BRACE, (near, first[0], first[1]), (far, second[0], second[1]))
        else:
            braces.strut(BRACE, (near, second[0], second[1]), (far, first[0], first[1]))


def _run(model: Model, offsets: tuple[tuple[float, float], ...], length: float, chord: float = CHORD) -> None:
    chords = model.part("truss-chords", SILVER)
    connectors = model.part("end-connectors", SILVER)
    for y, z in offsets:
        chords.cylinder(chord, length, (0, y, z), segments=12, rotation=(0, 90, 0))
        for side in (-1, 1):
            connectors.cone(
                chord,
                chord * 0.72,
                34,
                (side * (length / 2 + 17), y, z),
                segments=12,
                rotation=(0, side * 90, 0),
            )
            connectors.cylinder(chord + 14, 12, (side * (length / 2 - 6), y, z), segments=12, rotation=(0, 90, 0))
    if len(offsets) > 1:
        braces = model.part("truss-braces", SILVER)
        for first, second in _faces(len(offsets)):
            _lace(braces, offsets[first], offsets[second], length)


def sections() -> list[Model]:
    """Every section type at every length, plus the single pipe."""

    built: list[Model] = []
    for name, offsets in SECTIONS.items():
        for length in LENGTHS:
            model = Model(
                f"truss-{name}-{length:04d}",
                "truss",
                f"{name} truss, {length} mm: {CHORD:.0f} chords {SPACING:.0f} apart, "
                f"{BRACE:.0f} braces, bay every {BAY:.0f}",
                origin="centre of the section, lying along +X",
            )
            _run(model, offsets, float(length))
            built.append(model)
    for length in LENGTHS:
        model = Model(
            f"pipe-{length:04d}",
            "truss",
            f"Scaffold pipe, {length} mm: single 48.3 tube",
            origin="centre of the pipe, lying along +X",
        )
        _run(model, ((0.0, 0.0),), float(length), chord=48.3)
        built.append(model)
    return built


def _perpendicular(direction: tuple[int, int, int]) -> tuple[tuple[int, int, int], tuple[int, int, int]]:
    """The two axes a section's chords spread along, for an arm running ``direction``.

    The second one is kept as vertical as possible so a three-point corner still has its
    apex up on every horizontal arm.
    """

    if direction[0]:
        return (0, 1, 0), (0, 0, 1)
    if direction[1]:
        return (1, 0, 0), (0, 0, 1)
    return (1, 0, 0), (0, 1, 0)


def _corner_arm(
    chords: Part,
    connectors: Part,
    braces: Part,
    direction: tuple[int, int, int],
    offsets: tuple[tuple[float, float], ...],
) -> list[tuple[float, float, float]]:
    """One arm of a corner block. Returns where its chords stop at the node.

    An arm starts at the face of the node, not on the far side of it. Run back through the
    middle — which is what a negative start does — every chord bores through every other
    arm and comes out stopping in mid-air behind it.
    """

    across, up = _perpendicular(direction)
    reach = CORNER_ARM

    def point(distance: float, offset: tuple[float, float]) -> tuple[float, float, float]:
        return tuple(
            direction[axis] * distance + across[axis] * offset[0] + up[axis] * offset[1]
            for axis in range(3)
        )

    ends: list[tuple[float, float, float]] = []
    for offset in offsets:
        inner = point(NODE, offset)
        ends.append(inner)
        chords.strut(CHORD, inner, point(reach, offset), segments=12)
        connectors.cone(
            CHORD,
            CHORD * 0.72,
            34,
            point(reach + 17, offset),
            segments=12,
            rotation=_facing(direction),
        )
    if len(offsets) > 1:
        for first, second in _faces(len(offsets)):
            for distance in (reach - 330.0, reach - 40.0):
                braces.strut(BRACE, point(distance, offsets[first]), point(distance, offsets[second]))
            braces.strut(BRACE, point(reach - 330.0, offsets[first]), point(reach - 40.0, offsets[second]))
    return ends


def _distance(first: tuple[float, float, float], second: tuple[float, float, float]) -> float:
    return math.sqrt(sum((first[axis] - second[axis]) ** 2 for axis in range(3)))


def _mitre(chords: Part, arms: list[list[tuple[float, float, float]]]) -> None:
    """Weld the arms into a block, joining each chord end to the nearest end beside it.

    A corner is where chords *turn*, not where they cross. Every chord that arrives at the
    node either lands on the end of a neighbouring arm's chord — in which case the two are
    already mitred and nothing is needed — or has to be carried across the node to it. Done
    for every pair of arms this closes the node whatever it is: two arms get the missing
    edges of their corner, a T gets its chords running straight through, and a six-way gets
    the full cage. Members are deduplicated, because opposite pairs of arms would otherwise
    each ask for the same one.
    """

    drawn: set[tuple[tuple[float, ...], tuple[float, ...]]] = set()
    for index, ends in enumerate(arms):
        for other in arms[index + 1 :]:
            for start in ends:
                nearest = min(other, key=lambda point, origin=start: _distance(origin, point))
                if _distance(start, nearest) < 1.0:
                    continue
                key = tuple(
                    sorted(
                        (
                            tuple(round(value, 3) for value in start),
                            tuple(round(value, 3) for value in nearest),
                        )
                    )
                )
                if key in drawn:
                    continue
                drawn.add(key)
                chords.strut(CHORD, start, nearest, segments=12)

    # A rounded boss on every junction. Two chords meeting at a right angle leave their own
    # flat end caps showing as a notch in the corner; a ball the width of the chord fills it
    # and the joint reads as the welded mitre it is meant to be.
    for point in {tuple(round(value, 3) for value in end) for ends in arms for end in ends}:
        chords.sphere(CHORD, point, segments=10, rings=5)


def _facing(direction: tuple[int, int, int]) -> tuple[float, float, float]:
    """Euler angles that point a +Z primitive along ``direction``."""

    if direction[0]:
        return (0.0, 90.0 * direction[0], 0.0)
    if direction[1]:
        return (-90.0 * direction[1], 0.0, 0.0)
    return (0.0, 0.0, 0.0) if direction[2] > 0 else (180.0, 0.0, 0.0)


def corner_blocks() -> list[Model]:
    """Every corner a rental stock carries, for the 3-point and the 4-point.

    Each arm is 500 mm from the block centre, so two of them make the 500 x 500 corner
    the brief asks for and the rest are the same block with more arms welded on.
    """

    built: list[Model] = []
    for section in ("3-point", "4-point"):
        offsets = SECTIONS[section]
        for name, directions in CORNERS.items():
            model = Model(
                f"truss-{section}-{name}",
                "truss",
                f"{section} {name.replace('-', ' ')} block, {CORNER_ARM:.0f} mm arms",
                origin="the corner node itself",
            )
            chords = model.part("truss-chords", SILVER)
            connectors = model.part("end-connectors", SILVER)
            braces = model.part("truss-braces", SILVER)
            arms = [
                _corner_arm(chords, connectors, braces, direction, offsets)
                for direction in directions
            ]
            _mitre(chords, arms)
            built.append(model)
    return built


def ground_support() -> Model:
    """The tower foot: a 600 x 600 x 30 base plate and a sleeve block."""

    model = Model(
        "ground-support-base",
        "truss",
        "Ground support base: 600 x 600 x 30 plate with a sleeve block for a 4-point tower",
        origin="floor level, centred on the plate",
    )
    plate = model.part("base-plate", SILVER)
    plate.box((600, 600, 30), (0, 0, 15))
    plate.box((520, 520, 12), (0, 0, 36))
    sleeve = model.part("sleeve-block", SILVER)
    sleeve.box((420, 420, 300), (0, 0, 192))
    sleeve.box((470, 470, 24), (0, 0, 354))
    spigots = model.part("sleeve-spigots", SILVER)
    for y, z in SECTIONS["4-point"]:
        spigots.cone(CHORD, CHORD * 0.72, 60, (y, z, 396), segments=12)
    return model


LINK_WIDTH = 34.0
LINK_WIRE = 10.0
# How much longer a link is than it is wide. A load chain link is a stretched ring, and
# drawn round it reads as a bead necklace.
LINK_STRETCH = 1.45


def _chain_run(part: Part, length: float, at: tuple[float, float, float]) -> None:
    """A run of load chain of ``length``, hanging down ``-Z`` from ``at``.

    Every second link lies in the other plane, which is the only thing that makes a chain
    read as a chain: a stack of rings all facing one way is a spring. Links are spaced so
    consecutive ones overlap by about the wire, the way real links sit inside each other.
    """

    half = (LINK_WIDTH + LINK_WIRE) / 2 * LINK_STRETCH
    reach = max(1.0, length - 2 * half)
    links = max(2, round(reach / (2 * half - 2 * LINK_WIRE)) + 1)
    pitch = reach / (links - 1)
    for index in range(links):
        part.torus(
            LINK_WIDTH,
            LINK_WIRE,
            (at[0], at[1], at[2] - half - index * pitch),
            segments=10,
            sides=6,
            rotation=(90, 0, 90 * (index % 2)),
            scale=(1.0, LINK_STRETCH, 1.0),
        )


def load_chain() -> Model:
    """A half-metre of simplified load chain, for dropping a point off a beam.

    Shipped as its own model because a chain is never the right length in a plan: a show
    stacks as many of these as the drop needs rather than carrying a model per height.
    """

    model = Model(
        "chain-0500",
        "truss",
        "Load chain, 500 mm: simplified links in alternating planes",
        origin="mounting point at the top link, chain below",
    )
    chain = model.part("chain-links", SILVER, BASE, None, METAL)
    _chain_run(chain, 500.0, (0.0, 0.0, 0.0))
    return model


def chain_motor() -> Model:
    """An electric chain hoist: shackle, body, chain bag, and the hook on its fall.

    A one-tonne touring hoist, hung the way a rig hangs it — motor up, hook down — with
    the bag of slack chain under the body. The bag is what makes the model read as a
    hoist rather than as another black box on the truss.
    """

    model = Model(
        "chain-motor",
        "truss",
        "Electric chain hoist, 1 t: 430 x 200 x 250 body on a shackle, chain bag and load hook",
    )
    top = model.part("motor-shackle", SILVER, BASE, None, METAL)
    top.torus(96, 22, (0, 0, -22), segments=14, sides=8, rotation=(0, 90, 0))
    top.cylinder(38, 46, (0, 0, -58), segments=12)
    top.box((44, 74, 40), (0, 0, -96))
    top.cylinder(24, 78, (0, 0, -96), segments=10, rotation=(0, 90, 0))

    body_top = -140.0
    body = model.part("motor-body", MOTOR_BLACK, BASE, None, PAINT)
    body.box((430, 200, 250), (0, 0, body_top - 125))
    body.box((160, 210, 190), (-195, 0, body_top - 120))
    body.cylinder(210, 150, (150, 0, body_top - 125), segments=18, rotation=(0, 90, 0))
    handle = model.part("motor-handle", SILVER, BASE, body, METAL)
    for side in (-1, 1):
        handle.box((24, 24, 46), (side * 120, 0, body_top - 22))
    handle.box((264, 30, 26), (0, 0, body_top + 1))
    plate = model.part("motor-plate", WARNING_YELLOW, BASE, body)
    plate.box((150, 8, 44), (30, -104, body_top - 96))

    # The bag of slack chain, hung off the body on its own strap.
    bag_top = body_top - 250
    bag = model.part("chain-bag", HOUSING_BLACK, BASE, None, FABRIC)
    bag.taper((200, 190, 330), (170, 160), (-60, 0, bag_top - 165), rotation=(180, 0, 0))
    bag.box((214, 200, 26), (-60, 0, bag_top - 13))
    straps = model.part("bag-straps", SILVER, BASE, bag, METAL)
    for side in (-1, 1):
        straps.box((14, 20, 40), (-60 + side * 90, 0, bag_top + 10))

    # The fall: the load chain out of the other end of the body, and the hook on it.
    fall = model.part("load-chain", SILVER, BASE, None, METAL)
    drop = 220.0
    _chain_run(fall, drop, (150.0, 0.0, bag_top))
    hook = model.part("load-hook", SILVER, BASE, None, METAL)
    hook_top = bag_top - drop
    hook.box((60, 46, 56), (150, 0, hook_top - 28))
    hook.torus(96, 30, (150, 0, hook_top - 104), segments=14, sides=8, arc=250.0, rotation=(0, 90, 0))
    return model


def truss_lift() -> Model:
    """A wind-up tower: telescopic mast, winch, and a cross base on outriggers.

    Drawn part way up, at 4.8 m, because a lift shown collapsed reads as a stand and a
    lift shown at full extension does not fit next to anything else in the review file.
    """

    model = Model(
        "truss-lift",
        "truss",
        "Truss lift: 4800 telescopic mast on a 1800 outrigger base with a winch",
        origin="floor level, centred under the mast",
    )
    base = model.part("lift-base", MOTOR_BLACK, BASE, None, PAINT)
    base.box((240, 240, 200), (0, 0, 160))
    for axis in range(2):
        for side in (-1, 1):
            centre = (side * 470.0, 0.0) if axis == 0 else (0.0, side * 470.0)
            size = (940.0, 70.0) if axis == 0 else (70.0, 940.0)
            base.box((size[0], size[1], 70), (centre[0], centre[1], 95))
    braces = model.part("lift-braces", MOTOR_BLACK, BASE, None, PAINT)
    for axis in range(2):
        for side in (-1, 1):
            end = (side * 800.0, 0.0, 130.0) if axis == 0 else (0.0, side * 800.0, 130.0)
            braces.strut(46, (0.0, 0.0, 560.0), end, segments=8)
    feet = model.part("lift-feet", SILVER, BASE, None, METAL)
    for axis in range(2):
        for side in (-1, 1):
            x, y = (side * 880.0, 0.0) if axis == 0 else (0.0, side * 880.0)
            feet.cylinder(46, 130, (x, y, 95), segments=10)
            feet.cylinder(150, 30, (x, y, 15), segments=12)

    # Three telescopic sections, each a little slimmer than the one it slides out of.
    mast = model.part("lift-mast", SILVER, BASE, None, METAL)
    for section, (across, bottom, top) in enumerate(
        (((120.0, 200.0, 2100.0)), ((102.0, 1900.0, 3550.0)), ((84.0, 3350.0, 4780.0)))
    ):
        mast.box((across, across, top - bottom), (0, 0, (top + bottom) / 2))
        if section:
            collar = across + 22
            mast.box((collar, collar, 60), (0, 0, bottom + 30))
    stripes = model.part("mast-stripes", WARNING_YELLOW, BASE, mast)
    for step in range(3):
        stripes.box((124, 124, 40), (0, 0, 300 + step * 90))

    winch = model.part("lift-winch", MOTOR_BLACK, BASE, None, PAINT)
    winch.cylinder(180, 130, (0, -110, 900), segments=16, rotation=(90, 0, 0))
    winch.box((200, 90, 160), (0, -100, 900))
    crank = model.part("winch-crank", SILVER, BASE, winch, METAL)
    crank.cylinder(26, 150, (0, -210, 900), segments=10, rotation=(90, 0, 0))
    crank.box((26, 26, 190), (0, -240, 960))
    crank.cylinder(34, 90, (0, -290, 1040), segments=10, rotation=(90, 0, 0))

    fork = model.part("lift-fork", SILVER, BASE, None, METAL)
    fork.box((300, 120, 30), (0, 0, 4795))
    for side in (-1, 1):
        fork.box((40, 120, 160), (side * 130, 0, 4890))
        fork.cylinder(28, 130, (side * 130, 0, 4940), segments=10, rotation=(90, 0, 0))
    return model


def models() -> list[Model]:
    """Every truss item of part 4, plus the rigging a truss is flown and stood on."""

    return [
        *sections(),
        *corner_blocks(),
        ground_support(),
        chain_motor(),
        load_chain(),
        truss_lift(),
    ]
