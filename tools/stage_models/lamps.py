"""The lamp set of part 3 of the fixture and stage model brief.

Every lamp hangs from the origin and points down ``-Z`` in Blender, which the exporter
turns into glTF ``-Y``. Sizes are the millimetres the brief gives; where it gives a
silhouette rather than a number, the number here is the one that keeps the silhouette
right next to its neighbours in a rig.

Two rules run through the whole file and decide most of the numbers:

* **A lantern is built about its own tilt axis and then hung.** The body goes together
  with the bracket bolts at ``z = 0``; :func:`hang_from_bracket` measures how far it
  reaches from there, cuts a frame that clears that reach, and drops the whole thing to
  where the frame puts it. A bracket sized any other way is drawn from the outside and
  the lantern in it cannot actually turn.
* **A moving head is built about its own trunnions** for the same reason, and the yoke
  is cut long enough for the head to go right over.
"""

from __future__ import annotations

import math

from .kit import (
    BASE,
    COUPLER,
    arc_outline,
    DIFFUSER,
    GLASS,
    HEAD,
    HOUSING_BLACK,
    HOUSING_DARK,
    LENS_CLEAR,
    METAL,
    Model,
    PAINT,
    Part,
    SILVER,
    WHITE_DIFFUSER,
    YOKE,
)

# A lens ring reads as the business end only if it is lighter than what it sits on.
RIM_ON_BLACK = "#3A3D42"
RIM_ON_DARK = "#4A5560"
RIM_ON_SILVER = "#D6DADE"
GEL_FRAME = "#2A2C30"
REFLECTOR = "#D2D6DA"
# A blinder cell is a sealed beam: warm tungsten glass in a bright chromed retaining ring.
CHROME_RING = "#DEE2E6"
LAMP_GLASS = "#E6B76A"

# Nothing but rigging hangs above this: the half-coupler needs the space, and a bracket
# that reached into it would be bolted through the clamp it hangs from.
FRAME_TOP = -40.0
# How much air a swinging body keeps between itself and its own frame.
SWING_MARGIN = 18.0
# The mounting face at which one clamp stops being enough and a fixture is hung on two.
TWIN_CLAMP_WIDTH = 380.0


def truss_coupler(
    model: Model,
    parent: Part | None = None,
    offsets: tuple[float, ...] = (0.0,),
) -> Part:
    """The half-couplers at the mounting point, as one switchable node.

    Everything that bolts a fixture to a bar lives under ``truss-coupler``, so a desk
    that knows a fixture is flown can show it and a desk that has it on a floor base can
    hide the whole node. The safety bond hangs off it as a child for the same reason, and
    :func:`kit.unrigged` writes the same model without any of it as its own file.

    ``offsets`` places a clamp at each position across the mounting face. A fixture heavy
    or wide enough for two mounting points is hung on two, a batten on one every metre, and
    the model's origin is the point between them — which is what makes a two-clamp model
    drop onto a bar in the same place a one-clamp model does. More than one clamp also means
    the fixture cannot be swivelled on the bar, so only a single one records that hinge.

    Nothing here may be called a clamp: ``clamp`` contains ``lamp``, and the reader would
    tilt the thing the fixture hangs from.
    """

    coupler = model.part(COUPLER, SILVER, BASE, parent)
    jaw = model.part(f"{COUPLER}-bolt", "#B0B4B8", BASE, coupler)
    bond = model.part(f"{COUPLER}-bond", HOUSING_BLACK, BASE, coupler, finish=METAL)
    for offset in offsets:
        # A half-coupler: a split collar round a 50 tube, a cheek that carries the M12 bolt
        # the fixture hangs on, and the wing bolt that closes the jaw. Cast aluminium, so it
        # is silver whatever colour the lamp under it is.
        coupler.torus(84, 22, (offset, 0, 34), segments=16, sides=8, arc=250.0, rotation=(0, 90, 0))
        coupler.box((30, 74, 46), (offset, 0, 8))
        coupler.box((30, 26, 30), (offset, -38, 30))
        coupler.cylinder(24, 30, (offset, 0, -10), segments=12)
        coupler.cylinder(16, 22, (offset, 0, -26), segments=8)
        jaw.cylinder(11, 62, (offset, 46, 34), segments=8, rotation=(0, 90, 0))
        for side in (-1, 1):
            jaw.box((10, 34, 9), (offset + side * 16, 46, 34), rotation=(0, 0, side * 24))
        bond.torus(34, 4, (offset, 26, 4), segments=12, sides=5, rotation=(0, 90, 0))
    if len(offsets) == 1:
        model.swivel(
            COUPLER,
            (0.0, 0.0, 1.0),
            (0.0, 0.0, 0.0),
            (-180.0, 180.0),
            "half-coupler rotation about the hanging bolt, for aiming the fixture on the bar",
        )
    return coupler


def rig(model: Model, mounting_width: float) -> None:
    """Clamp the fixture to the bar: one coupler for a light one, two for a wide or heavy one.

    A single half-coupler under a 400-wide moving-head base is not how anyone hangs one —
    it would spin on the bar and the base would sit on the clamp like a see-saw. Anything
    with a mounting face this wide gets a pair, spread across it but not out at its edges,
    and the origin stays the point between them.
    """

    if mounting_width < TWIN_CLAMP_WIDTH:
        truss_coupler(model)
        return
    spacing = min(mounting_width * 0.45, 320.0)
    truss_coupler(model, offsets=(-spacing / 2, spacing / 2))


def hang_from_bracket(
    model: Model,
    bar_depth: float,
    arm_thickness: float,
    colour: str = SILVER,
    finish: str = METAL,
    margin: float | None = None,
) -> float:
    """Hang the body built about ``z = 0`` in a U-frame it can actually turn in.

    Deliberately not called a yoke: a PAR, a Fresnel or a blinder never pans, and a node
    named ``yoke`` would turn under a pan attribute that the fixture does not have.

    The frame is cut from the body rather than guessed at. Everything already built is
    measured about the bracket bolts, the crossbar is put far enough above them that
    ninety degrees of tilt either way swings clear of it, the arms are set outside the
    widest part, and the body is then dropped so the crossbar sits under the coupler.
    Returns the height the bolts ended up at, for anything that has to line up with them.
    """

    above, depth, width = model.reach(skip=(COUPLER,))
    reach = max(above, depth)
    # Clearances in proportion, not in absolutes. Eighteen millimetres of swing room and a
    # twelve-millimetre gap either side are nothing on a PAR 64 and are half the fixture on
    # an ACL, which is what made the small can look like it was hung off scaffolding.
    swing = reach + (margin if margin is not None else max(6.0, min(SWING_MARGIN, reach * 0.12)))
    bar = arm_thickness * 1.6
    pivot = FRAME_TOP - bar - swing
    half_span = width + arm_thickness / 2 + max(6.0, min(14.0, width * 0.09))
    # What the bolts actually land on: the body's own width level with them, which on a can
    # with a colour frame is a good deal narrower than the arms standing outside that frame.
    trunnion = model.girth(arm_thickness * 2.0, skip=(COUPLER,))
    model.shift((0.0, 0.0, pivot), skip=(COUPLER,))

    frame = model.part("hanging-frame", colour, BASE, None, finish)
    frame.box((half_span * 2 + arm_thickness, bar_depth, bar), (0, 0, FRAME_TOP - bar / 2))
    foot = pivot - arm_thickness * 2.4
    outer = half_span + arm_thickness / 2
    inner = min(trunnion, outer - arm_thickness - 2.0)
    for side in (-1, 1):
        frame.box(
            (arm_thickness, bar_depth, FRAME_TOP - bar - foot),
            (side * half_span, 0, (FRAME_TOP - bar + foot) / 2),
        )
        # A trunnion spanning arm to body, so the bracket is bolted to the lantern rather
        # than floating a boss in the gap beside it.
        frame.cylinder(
            max(16.0, arm_thickness * 3.2),
            outer - inner,
            (side * (inner + outer) / 2, 0, pivot),
            segments=12,
            rotation=(0, 90, 0),
        )
    model.swivel(
        "hanging-frame",
        (1.0, 0.0, 0.0),
        (0.0, 0.0, pivot),
        (-90.0, 90.0),
        "tilt about the bracket bolts; the arms are cut so the body swings clear of the crossbar",
        clearance=swing,
    )
    rig(model, half_span * 2 + arm_thickness)
    return pivot


def _head_cheeks(model: Model, head: Part, half_width: float, length: float) -> Part:
    """The flats each side of a turned head where the tilt bolts go, at ``z = 0``.

    They have to stay inside the yoke arms, which is why the builder hands their outer
    face to :func:`_moving_head_frame` as the head's width.
    """

    cheeks = model.part("head-cheeks", HOUSING_DARK, HEAD, head)
    for side in (-1, 1):
        cheeks.box((20, length * 0.5, length * 0.62), (side * (half_width - 6), 0, 0))
        cheeks.cylinder(
            min(120.0, length * 0.6),
            22,
            (side * (half_width - 3), 0, 0),
            segments=16,
            rotation=(0, 90, 0),
        )
    return cheeks


def _ring_positions(radius: float, count: int, phase: float = 0.0) -> list[tuple[float, float]]:
    step = math.tau / count
    return [
        (radius * math.cos(phase + index * step), radius * math.sin(phase + index * step))
        for index in range(count)
    ]


# --------------------------------------------------------------------------------------
# 3.1 – 3.3 Moving heads
# --------------------------------------------------------------------------------------


def _moving_base(model: Model, size: tuple[float, float, float]) -> float:
    """The base plate under the coupler. Returns the height its underside sits at."""

    width, depth, height = size
    base = model.part("moving-base", HOUSING_DARK)
    base.box((width, depth, height), (0, 0, FRAME_TOP - height / 2))
    base.box((width - 40, depth - 30, 26), (0, 0, FRAME_TOP - height - 13))
    display = model.part("base-display", SILVER, BASE, base)
    display.box((96, 8, 62), (width / 4, -depth / 2 - 4, FRAME_TOP - height / 2))
    connectors = model.part("base-connectors", SILVER, BASE, base)
    connectors.box((220, 10, 78), (0, depth / 2 + 5, FRAME_TOP - height / 2 - 10))
    return FRAME_TOP - height - 26


def _moving_yoke(
    model: Model,
    base_bottom: float,
    half_span: float,
    arm_thickness: float,
    pivot: float,
) -> Part:
    yoke = model.group_node("moving-yoke", YOKE)
    crossbar = model.part("yoke-crossbar", HOUSING_DARK, YOKE, yoke)
    crossbar.box((half_span * 2 + arm_thickness, 150, 55), (0, 0, base_bottom - 27.5))
    top = base_bottom - 55
    bottom = pivot - arm_thickness * 0.9
    for side, name in ((-1, "yoke-left"), (1, "yoke-right")):
        arm = model.part(name, HOUSING_DARK, YOKE, yoke)
        arm.taper(
            (arm_thickness, 150, top - bottom),
            (arm_thickness, 118),
            (side * half_span, 0, (top + bottom) / 2),
        )
        arm.cylinder(
            92,
            22,
            (side * (half_span - arm_thickness / 2 - 11), 0, pivot),
            segments=12,
            rotation=(0, 90, 0),
        )
    return yoke


def _moving_head_frame(
    model: Model,
    base_size: tuple[float, float, float],
    head_half_width: float,
    arm_thickness: float = 70.0,
) -> float:
    """Hang a head built about ``z = 0`` in a yoke long enough for it to turn right over.

    A moving head's tilt is not the ninety degrees a lantern bracket gets: the head goes
    past vertical both ways, so the yoke has to clear the head's whole turning radius and
    not just its reach above the trunnions. Getting this wrong is what makes a modelled
    head sit in a stub of a yoke that the real casting would foul on its first move.
    """

    turning = max(
        math.sqrt(y * y + z * z)
        for part in model.parts
        if part.kind == HEAD
        for _, y, z in part.vertices
    )
    base_bottom = FRAME_TOP - base_size[2] - 26.0
    pivot = base_bottom - 55.0 - turning - 26.0
    model.shift((0.0, 0.0, pivot), skip=(COUPLER,))
    _moving_base(model, base_size)
    _moving_yoke(model, base_bottom, head_half_width + 48.0, arm_thickness, pivot)
    # Always two clamps, whatever the base measures. A moving head is the heavy case the
    # pair exists for: hung on one it swings on the bar, and the base sits on the clamp like
    # a see-saw with a turning head on the end of it.
    spacing = min(base_size[0] * 0.55, 320.0)
    truss_coupler(model, offsets=(-spacing / 2, spacing / 2))
    model.swivel(
        "moving-yoke",
        (0.0, 0.0, 1.0),
        (0.0, 0.0, 0.0),
        (-270.0, 270.0),
        "pan about the hanging axis, carrying the yoke and everything in it",
    )
    model.swivel(
        "moving-head",
        (1.0, 0.0, 0.0),
        (0.0, 0.0, pivot),
        (-135.0, 135.0),
        "tilt about the yoke trunnions, which the arms are cut long enough to allow",
    )
    return pivot


def moving_head_profile() -> Model:
    """3.1 — the long-nosed head, turned rather than boxed.

    The body is one outline spun about the optical axis and squeezed a little in depth:
    a domed back, a barrel, the raised step where the shutters sit, and a flare down to
    the nose. Real profile heads are mouldings over a round optical train, and the shape
    the procedural proxy draws — an ovoid, not a crate — is the one that reads as a lamp
    from the back of the room.
    """

    model = Model(
        "moving-head-profile",
        "lamps",
        "Moving head profile: 400 x 250 x 175 base, turned 300 x 480 head on a full-travel yoke",
    )
    head = model.group_node("moving-head", HEAD)
    bottom = -240.0
    body = model.part("head-body", HOUSING_DARK, HEAD, head)
    body.revolve(
        [
            (0.0, 0.0),
            (54.0, 0.0),
            (60.0, 16.0),
            (94.0, 60.0),
            (122.0, 104.0),
            (140.0, 150.0),
            (144.0, 196.0),
            (144.0, 292.0),
            (150.0, 308.0),
            (150.0, 336.0),
            (144.0, 352.0),
            (144.0, 384.0),
            *arc_outline((0.0, 384.0), 144.0, 0.0, 90.0, steps=5, rise=96.0),
        ],
        (0, 0, bottom),
        segments=28,
        scale=(1.0, 0.85, 1.0),
    )
    _head_cheeks(model, head, 150.0, 210.0)
    ring = model.part("lens-ring", RIM_ON_DARK, HEAD, head)
    ring.tube(132, 96, 26, (0, 0, bottom - 6), segments=24)
    lens = model.part("lens", LENS_CLEAR, HEAD, head)
    lens.cylinder(100, 14, (0, 0, bottom - 4), segments=24)
    _moving_head_frame(model, (400.0, 250.0, 175.0), head_half_width=158.0)
    return model


def moving_head_wash() -> Model:
    """3.2 — the same family, stubbier, with one big front element.

    The silhouette difference between a profile and a wash is the whole point: this one
    is as wide as the profile and two thirds of its length, and the lens fills the face
    instead of sitting in a nose.
    """

    model = Model(
        "moving-head-wash",
        "lamps",
        "Moving head wash: turned 320 x 330 head with a 210 front lens on a full-travel yoke",
    )
    head = model.group_node("moving-head", HEAD)
    bottom = -165.0
    body = model.part("head-body", HOUSING_DARK, HEAD, head)
    body.revolve(
        [
            (0.0, 0.0),
            (104.0, 0.0),
            (118.0, 20.0),
            (142.0, 54.0),
            (156.0, 92.0),
            (160.0, 168.0),
            (154.0, 200.0),
            *arc_outline((0.0, 220.0), 154.0, 0.0, 90.0, steps=5, rise=110.0),
        ],
        (0, 0, bottom),
        segments=28,
        scale=(1.0, 0.875, 1.0),
    )
    _head_cheeks(model, head, 160.0, 180.0)
    ring = model.part("lens-ring", RIM_ON_DARK, HEAD, head)
    ring.tube(238, 196, 36, (0, 0, bottom - 14), segments=28)
    lens = model.part("lens", LENS_CLEAR, HEAD, head)
    lens.revolve(
        [(0.0, 2.0), (105.0, 6.0), (105.0, 18.0), (0.0, 26.0)],
        (0, 0, bottom - 18),
        segments=28,
    )
    _moving_head_frame(model, (400.0, 250.0, 170.0), head_half_width=168.0)
    return model


def led_wash_heads() -> list[Model]:
    """3.3 — the LED wash family: one turned disc head in three radii.

    The head is a squat cylinder — call its height one unit and its radius sits between
    one and a half and two and a half of them. The back rim is filleted, the front face
    is sunk a fifth of the height inside a slightly smaller circle, and the emitter domes
    stand proud of the lip rather than hiding in the recess.

    Base and yoke are cut to the head rather than shared across the family: the smallest
    head under the largest head's base looks like a lamp bolted to somebody else's motor,
    and arms sized for a 500 are half the width of a 300.

    Ring radii are computed, not written down. The outermost ring plus a dome's own radius
    has to stay inside the sunk face, and the one place that went wrong — a 500 with its
    outer ring hanging over the rim — is exactly the arithmetic a table of literals invites.
    """

    built: list[Model] = []
    dome = 46.0
    for label, radius, height, counts, base_size, arm in (
        ("300", 150.0, 100.0, (1, 6, 12), (340.0, 210.0, 150.0), 52.0),
        ("400", 200.0, 110.0, (1, 6, 12, 18), (400.0, 250.0, 170.0), 66.0),
        ("500", 250.0, 120.0, (1, 6, 12, 18, 24), (460.0, 280.0, 180.0), 78.0),
    ):
        pitch = (radius - 26.0 - dome / 2) / (len(counts) - 1)
        rings = tuple((step * pitch, count) for step, count in enumerate(counts))
        model = Model(
            f"moving-head-led-wash-{label}",
            "lamps",
            f"Moving head LED wash {label}: turned head {radius * 2:.0f} across, "
            f"{height:.0f} deep, proud emitter domes on a sunk front face",
        )
        head = model.group_node("moving-head", HEAD)
        bottom = -height / 2
        recess = height / 5
        fillet = height * 0.35
        body = model.part("head-body", HOUSING_DARK, HEAD, head)
        body.revolve(
            [
                (0.0, recess),
                (radius - 26.0, recess),
                (radius - 26.0, 0.0),
                (radius, 0.0),
                (radius, height - fillet),
                *arc_outline((radius - fillet, height - fillet), fillet, 0.0, 90.0, steps=4),
                (0.0, height),
            ],
            (0, 0, bottom),
            segments=32,
        )
        cheeks = model.part("head-cheeks", HOUSING_DARK, HEAD, head)
        for side in (-1, 1):
            cheeks.cylinder(
                height * 0.8,
                26,
                (side * (radius - 3), 0, 0),
                segments=16,
                rotation=(0, 90, 0),
            )
        array = model.part("head-lens-array", WHITE_DIFFUSER, HEAD, head, finish=DIFFUSER)
        for ring_radius, count in rings:
            for x, y in _ring_positions(ring_radius, count) if count > 1 else [(0.0, 0.0)]:
                # Proud of the front lip, not sunk behind it: an emitter the housing hides
                # is the one thing on an LED wash an operator actually looks for.
                array.dome(
                    dome,
                    recess + 8,
                    (x, y, bottom + recess),
                    segments=10,
                    rotation=(180, 0, 0),
                )
        _moving_head_frame(
            model,
            base_size,
            head_half_width=radius + 12.0,
            arm_thickness=arm,
        )
        built.append(model)
    return built


# --------------------------------------------------------------------------------------
# 3.4 Fresnel and the house profile
# --------------------------------------------------------------------------------------


def fresnel() -> Model:
    """3.4 — rounded cowl, stepped lens, and four splayed barn-door leaves."""

    model = Model("fresnel-barn-doors", "lamps", "Fresnel 260 x 300 x 300 with four barn doors")
    top, bottom = 150.0, -150.0
    body = model.part("body", HOUSING_BLACK)
    body.box((260, 300, 300), (0, 0, 0))
    body.sphere(260, (0, 0, top - 6), segments=16, rings=6, scale=(1.0, 1.12, 0.34))
    body.box((90, 60, 26), (0, 130, top - 40))

    ring = model.part("lens-ring", RIM_ON_BLACK)
    ring.tube(228, 200, 30, (0, 0, bottom + 5))
    lens = model.part("lens", LENS_CLEAR)
    lens.cylinder(200, 14, (0, 0, bottom + 3))
    for step, diameter in enumerate((200.0, 146.0, 92.0)):
        lens.cone(diameter, diameter - 18, 9, (0, 0, bottom - 4 - step * 8), segments=20)

    runner = model.part("colour-frame-runner", GEL_FRAME)
    runner.polygon_frame(340, 296, 46, (0, 0, bottom - 34))
    barn_doors(model, aperture=200.0, at=bottom - 60, hinge_span=150.0)
    hang_from_bracket(model, bar_depth=90.0, arm_thickness=20.0)
    return model


def barn_doors(model: Model, aperture: float, at: float, hinge_span: float) -> None:
    """A four-leaf barn door: two long leaves and two short ones, on a round frame.

    Real barn doors are not four identical flaps. The pair that closes across the beam
    is nearly as wide as the frame and long enough to cut it right down; the other pair
    is shorter, and hinges outside the first so the four can fold flat over each other.
    The frame itself is a ring the size of the lens with a square carrier round it, which
    is what makes the accessory read as a barn door rather than as a box.

    Each leaf declares its hinge, so whoever animates them later swings them about the
    edge they are actually bolted to rather than about their middle.
    """

    ring = model.part("barn-door-frame", GEL_FRAME)
    ring.tube(aperture + 54, aperture + 8, 26, (0, 0, at), segments=28)
    for side in (-1, 1):
        ring.box((12, hinge_span * 2 + 24, 30), (side * (hinge_span + 6), 0, at))
        ring.box((hinge_span * 2 + 24, 12, 30), (0, side * (hinge_span + 6), at))
    hinges = model.part("barn-door-hinges", SILVER, BASE, ring)
    for side in (-1, 1):
        for offset in (-hinge_span * 0.55, hinge_span * 0.55):
            hinges.box((26, 20, 22), (offset, side * (hinge_span + 6), at - 16))
            hinges.box((20, 26, 22), (side * (hinge_span + 6), offset, at - 16))

    splay = 26.0
    for side, name, width, length in (
        (-1, "barn-door-bottom", hinge_span * 2 + 20, aperture * 1.15),
        (1, "barn-door-top", hinge_span * 2 + 20, aperture * 1.15),
    ):
        _barn_leaf(model, name, (0.0, side * (hinge_span + 6), at - 26), width, length, side * splay, True)
    for side, name in ((-1, "barn-door-left"), (1, "barn-door-right")):
        _barn_leaf(
            model,
            name,
            (side * (hinge_span + 6), 0.0, at - 26),
            hinge_span * 1.25,
            aperture * 0.78,
            -side * splay,
            False,
        )


def profile_spot() -> Model:
    """A static ellipsoidal: the house profile every rig has forty of.

    Four sections, back to front: a finned lamp housing, the shutter barrel with its
    handles, the lens tube with its focus knob, and a gel-frame holder at the nose. It
    hangs from a bracket bolted at the barrel rather than at the middle of the body,
    which is why a profile always looks nose-heavy on a bar.
    """

    model = Model(
        "profile-spot",
        "lamps",
        "Profile spot: 640 long ellipsoidal, shutter barrel, lens tube, octagonal gel holder",
    )
    rear, front = 290.0, -350.0

    housing = model.part("rear-housing", HOUSING_BLACK)
    housing.revolve(
        [
            (0.0, 0.0),
            (96.0, 0.0),
            (96.0, 150.0),
            *arc_outline((0.0, 150.0), 96.0, 0.0, 90.0, steps=4, rise=52.0),
        ],
        (0, 0, rear - 202),
        segments=22,
    )
    for step in range(5):
        housing.tube(204, 188, 10, (0, 0, rear - 60 - step * 22), segments=22)
    cap = model.part("rear-cap", SILVER, BASE, housing)
    cap.cylinder(120, 26, (0, 0, rear + 8), segments=14)

    barrel = model.part("shutter-barrel", HOUSING_BLACK)
    barrel.taper((188, 188, 190), (176, 176), (0, 0, rear - 295))
    barrel.box((206, 206, 22), (0, 0, rear - 202))
    handles = model.part("shutter-handles", SILVER, BASE, barrel)
    for index, (x, y) in enumerate(_ring_positions(118.0, 4, math.radians(45))):
        handles.strut(
            16,
            (x * 0.7, y * 0.7, rear - 240 - index * 26),
            (x, y, rear - 270 - index * 26),
            segments=6,
        )
        handles.sphere(30, (x, y, rear - 272 - index * 26), segments=8, rings=4)

    tube = model.part("focus-barrel", HOUSING_BLACK)
    tube.revolve(
        [
            (0.0, 0.0),
            (84.0, 0.0),
            (84.0, 190.0),
            (92.0, 200.0),
            (92.0, 228.0),
            (0.0, 240.0),
        ],
        (0, 0, front + 10),
        segments=22,
    )
    knob = model.part("focus-knob", SILVER, BASE, tube)
    knob.cylinder(44, 30, (0, 104, front + 160), segments=10, rotation=(90, 0, 0))
    lens = model.part("lens", LENS_CLEAR)
    lens.revolve(
        [(0.0, 0.0), (78.0, 22.0), (78.0, 40.0), (0.0, 58.0)],
        (0, 0, front + 8),
        segments=22,
    )
    ring = model.part("lens-ring", RIM_ON_BLACK)
    ring.tube(196, 168, 24, (0, 0, front + 12))

    runner = model.part("colour-frame-runner", GEL_FRAME)
    runner.polygon_frame(240, 202, 60, (0, 0, front - 18))
    clip = model.part("frame-clip", SILVER, BASE, runner)
    clip.box((120, 12, 16), (0, -104, front + 16))
    hang_from_bracket(model, bar_depth=120.0, arm_thickness=12.0)
    return model


def _barn_leaf(
    model: Model,
    name: str,
    hinge: tuple[float, float, float],
    width: float,
    length: float,
    splay: float,
    across: bool,
) -> None:
    """One leaf, hanging from ``hinge`` and splayed outwards by ``splay`` degrees."""

    angle = math.radians(abs(splay))
    reach = length / 2 * math.cos(angle)
    offset = length / 2 * math.sin(angle) * (1 if splay >= 0 else -1)
    leaf = model.part(name, HOUSING_BLACK)
    if across:
        leaf.box((width, 4, length), (hinge[0], hinge[1] + offset, hinge[2] - reach), rotation=(splay, 0, 0))
        axis = (1.0, 0.0, 0.0)
    else:
        leaf.box((4, width, length), (hinge[0] - offset, hinge[1], hinge[2] - reach), rotation=(0, splay, 0))
        axis = (0.0, 1.0, 0.0)
    model.swivel(name, axis, hinge, (0.0, 90.0), "barn-door leaf, hinged at the frame")


# --------------------------------------------------------------------------------------
# 3.5 – 3.8 The PAR family
# --------------------------------------------------------------------------------------


def _par_can(
    name: str,
    summary: str,
    diameter: float,
    length: float,
    body_colour: str,
    rim_colour: str,
    *,
    clips: bool = True,
    gel_frame: bool = True,
) -> Model:
    """A sealed-beam can: rolled tube, domed back, and an open front you can see into.

    Hollow is the point. A PAR is a piece of tube with a reflector lamp pushed into it,
    and the solid cylinder the first pass drew gave an operator no way to tell a lit lamp
    from a dead one, or the front of the can from the back.
    """

    model = Model(name, "lamps", summary)
    # The bracket bolts sit a third of the way back, where a PAR balances.
    front = -length * 0.66
    wall = max(4.0, diameter * 0.03)
    dome = diameter / 2

    can = model.part("par-can", body_colour)
    can.hollow_can(diameter, diameter - 2 * wall, length, dome, (0, 0, front), segments=24)

    # The sealed beam sits well back down the can, which is why a long-nose PAR is mostly
    # empty tube: the reflector and its capsule are visible through the open nose, and the
    # inner wall between them and the rim is what makes the lamp read as hollow at all.
    mount = front + length * 0.42
    reflector = model.part("reflector", REFLECTOR, BASE, can, finish=METAL)
    reflector.parabolic_cup(
        diameter - 2 * wall - 4,
        length * 0.34,
        3,
        (0, 0, mount + length * 0.34),
        segments=20,
        rotation=(180, 0, 0),
    )
    filament = model.part("filament", LENS_CLEAR, BASE, can, finish=GLASS)
    filament.sphere(
        diameter * 0.24,
        (0, 0, mount + length * 0.2),
        segments=10,
        rings=6,
        scale=(0.6, 0.6, 1.0),
    )
    lens = model.part("lens", LENS_CLEAR)
    lens.revolve(
        [(0.0, 0.0), (diameter / 2 - wall - 1, 10.0), (diameter / 2 - wall - 1, 22.0), (0.0, 30.0)],
        (0, 0, mount - 12),
        segments=24,
    )

    rim = model.part("lens-ring", rim_colour)
    rim.tube(diameter + 18, diameter - 2, 22, (0, 0, front + 11))

    if clips:
        clip = model.part("spring-clips", SILVER)
        for x, y in _ring_positions(diameter / 2 + 4, 4, math.radians(45)):
            clip.box((26, 16, 34), (x, y, front + 30), rotation=(0, 0, math.degrees(math.atan2(y, x))))
    if gel_frame:
        # A colour frame for a PAR is an octagon of folded sheet, and the runner that
        # holds it is the same shape. A square holder is the one detail that makes a
        # modelled PAR look like a torch.
        runner = model.part("colour-frame-runner", GEL_FRAME)
        runner.polygon_frame(diameter + 76, diameter + 30, 16, (0, 0, front - 6))
        runner.polygon_frame(diameter + 60, diameter + 24, 6, (0, 0, front - 17))
    hang_from_bracket(
        model,
        bar_depth=max(30.0, diameter * 0.34),
        arm_thickness=max(6.0, diameter * 0.045),
        colour=body_colour,
        finish=PAINT if body_colour != SILVER else METAL,
    )
    return model


def par_cans() -> list[Model]:
    """3.5 – 3.7 — PAR 64 long and short nose, and the PAR 56, black and silver."""

    built: list[Model] = []
    for finish, body, rim in (
        ("black", HOUSING_BLACK, RIM_ON_BLACK),
        ("silver", SILVER, RIM_ON_SILVER),
    ):
        built.append(
            _par_can(
                f"par-64-long-nose-{finish}",
                f"PAR 64 long nose, {finish}: hollow 205 can, 400 long, octagonal gel frame",
                205.0,
                400.0,
                body,
                rim,
            )
        )
        built.append(
            _par_can(
                f"par-64-short-nose-{finish}",
                f"PAR 64 short nose, {finish}: hollow 205 can, 280 long, octagonal gel frame",
                205.0,
                280.0,
                body,
                rim,
            )
        )
        built.append(
            _par_can(
                f"par-56-{finish}",
                f"PAR 56, {finish}: hollow 180 can, 260 long, octagonal gel frame",
                180.0,
                260.0,
                body,
                rim,
            )
        )
    return built


def acl() -> Model:
    """3.8 — the small silver can that gets rigged four to a bar."""

    return _par_can(
        "acl-par-16",
        "ACL / PAR 16 narrow: hollow 55 can, 130 long, single-cell frame",
        55.0,
        130.0,
        SILVER,
        RIM_ON_SILVER,
        clips=False,
        gel_frame=False,
    )


# --------------------------------------------------------------------------------------
# 3.9 – 3.11 LED PARs
# --------------------------------------------------------------------------------------


def _led_par_shell(name: str, summary: str, diameter: float, length: float) -> tuple[Model, float]:
    """The open can an LED PAR's source plate sits inside. Returns the front rim height."""

    model = Model(name, "lamps", summary)
    rear, front = length * 0.5, -length * 0.5
    can = model.part("par-can", HOUSING_DARK)
    can.hollow_can(diameter, diameter - 20, length, 44, (0, 0, front), segments=24)
    fins = model.part("heatsink", HOUSING_DARK, BASE, can)
    for step in range(3):
        fins.tube(diameter + 20, diameter - 2, 9, (0, 0, rear - 30 - step * 26), segments=24)
    rim = model.part("lens-ring", RIM_ON_DARK)
    rim.tube(diameter + 16, diameter - 6, 24, (0, 0, front + 12))
    return model, front


def led_par_pizza() -> Model:
    """3.9 — the front that looks like a pizza, because of 31 separate sources."""

    model, front = _led_par_shell(
        "led-par-pizza",
        "LED PAR pizza lamp: hollow 180 can, 230 long, 18 + 12 + 1 domed sources",
        180.0,
        230.0,
    )
    plate = model.part("source-plate", HOUSING_BLACK)
    plate.cylinder(158, 10, (0, 0, front + 18))
    sources = model.part("source-array", LENS_CLEAR, finish=GLASS)
    for radius, count in ((0.0, 1), (42.0, 12), (70.0, 18)):
        for x, y in _ring_positions(radius, count) if count > 1 else [(0.0, 0.0)]:
            # Domes standing clear of the plate and just proud of the rim, not spheres
            # half sunk into it: the separate sources are the whole point of this front.
            sources.dome(16, 22, (x, y, front + 14), segments=8, rotation=(180, 0, 0))
    hang_from_bracket(model, bar_depth=54.0, arm_thickness=18.0, colour=HOUSING_DARK, finish=PAINT)
    return model


def led_par_x_in_1() -> Model:
    """3.10 — 18 multi-chip emitters in shallow cups; one model for 4-in-1 to 7-in-1."""

    model, front = _led_par_shell(
        "led-par-x-in-1",
        "LED PAR x-in-1: hollow 180 can, 18 emitters of 20 in shallow cups",
        180.0,
        230.0,
    )
    plate = model.part("source-plate", HOUSING_BLACK)
    plate.cylinder(158, 12, (0, 0, front + 34))
    cups = model.part("source-cups", SILVER)
    emitters = model.part("source-array", LENS_CLEAR, finish=GLASS)
    # The cups stand proud of the plate and open downwards past the rim; sunk flush they
    # would be a black disc, because the reader keeps no transparency to see a recess through.
    for radius, count in ((36.0, 6), (68.0, 12)):
        for x, y in _ring_positions(radius, count):
            cups.cup(34, 22, 24, 2, (x, y, front + 20), segments=10, rotation=(180, 0, 0))
            emitters.cylinder(20, 10, (x, y, front + 26), segments=10)
    hang_from_bracket(model, bar_depth=54.0, arm_thickness=18.0, colour=HOUSING_DARK, finish=PAINT)
    return model


def flat_led_par() -> Model:
    """3.11 — the slim disc that hangs tight to the bar."""

    model = Model("flat-led-par", "lamps", "Flat LED PAR: 230 disc, 95 deep, 18 proud emitters")
    rear, front = 47.5, -47.5
    body = model.part("body", HOUSING_DARK)
    body.hollow_can(230, 206, 95, 30, (0, 0, front), segments=28)
    housing = model.part("driver-housing", HOUSING_DARK, BASE, body)
    housing.box((150, 110, 48), (0, 0, rear + 24))
    display = model.part("body-display", SILVER, BASE, body)
    display.box((70, 6, 34), (0, -58, rear + 24))
    bezel = model.part("lens-ring", RIM_ON_DARK)
    bezel.tube(232, 206, 22, (0, 0, front + 11))
    plate = model.part("source-plate", HOUSING_BLACK)
    plate.cylinder(200, 12, (0, 0, front + 34))
    emitters = model.part("source-array", LENS_CLEAR, finish=GLASS)
    for radius, count in ((36.0, 6), (72.0, 12)):
        for x, y in _ring_positions(radius, count):
            emitters.cylinder(24, 30, (x, y, front + 13), segments=10)
    hang_from_bracket(model, bar_depth=70.0, arm_thickness=16.0, colour=HOUSING_DARK, finish=PAINT)
    return model


# --------------------------------------------------------------------------------------
# 3.12 – 3.15 Bars, blinders and strobes
# --------------------------------------------------------------------------------------


def _reflector_cell(
    cups: Part,
    filaments: Part,
    at: tuple[float, float, float],
    mouth: float,
    depth: float,
) -> None:
    """One pressed parabola with a linear capsule at its focus, opening downwards.

    A blinder cell and a sunstrip cell are the same thing at two sizes: a straight cone
    and a plain rod read as a funnel with a pipe in it, and a bulb hung under the bar
    reads as a household lamp, which is what a sunstrip conspicuously is not.
    """

    x, y, z = at
    # Proud of the tray, not sunk into it: a cell flush with the frame is invisible from
    # anywhere but straight in front, which is the one angle an operator never looks from.
    cups.parabolic_cup(mouth, depth, mouth * 0.024, (x, y, z + depth - mouth * 0.14), segments=16, rotation=(180, 0, 0))
    filaments.sphere(
        mouth * 0.31,
        (x, y, z + depth * 0.62 - mouth * 0.14),
        segments=10,
        rings=6,
        scale=(0.42, 0.42, 1.0),
    )


def _sealed_beam_cell(
    ring: Part,
    lens: Part,
    at: tuple[float, float, float],
    mouth: float,
) -> None:
    """One PAR 64 sealed beam in a polished ring, opening downwards.

    What is actually bolted into a blinder: not a bare capsule in a pressed cup, but a
    sealed-beam PAR lamp — a warm ribbed glass face filling a chromed retaining ring, held
    by three spring clips. The ring and the ribbing are what make the cell read as a lamp
    the moment anyone looks at the fixture; a plain silver funnel reads as a downlight.
    """

    x, y, z = at
    face = z + 6.0
    glass_radius = mouth / 2 - 24.0

    ring.tube(mouth, mouth - 42, 44, (x, y, z + 22), segments=20)
    ring.tube(mouth - 6, mouth - 30, 10, (x, y, face), segments=20)
    for angle in _ring_positions(mouth / 2 - 12, 3, math.radians(90)):
        ring.box((22, 14, 26), (x + angle[0], y + angle[1], face - 4), rotation=(0, 0, 12))

    # A shallow domed face, and three concentric ridges standing on it for the linear
    # prisms a PAR lens is moulded with.
    lens.revolve(
        [(0.0, 0.0), (glass_radius, 14.0), (glass_radius, 30.0), (0.0, 42.0)],
        (x, y, face - 34),
        segments=20,
    )
    for step in (0.4, 0.66, 0.9):
        lens.tube(
            glass_radius * 2 * step,
            glass_radius * 2 * step - 12,
            10,
            (x, y, face - 30),
            segments=20,
        )


def sunstrip() -> Model:
    """3.12 — ten flat reflector cells on 100 centres; the whole bar stays a base part."""

    model = Model(
        "sunstrip",
        "lamps",
        "Sunstrip: 1000 x 90 x 90 bar, ten 84 reflector cells on 100 centres",
    )
    top, bottom = 45.0, -45.0
    bar = model.part("sunstrip-bar", HOUSING_BLACK)
    bar.box((1000, 90, 90), (0, 0, 0))
    bar.box((940, 60, 16), (0, 0, top + 8))
    cups = model.part("cell-cups", REFLECTOR, finish=METAL)
    filaments = model.part("cell-filaments", LENS_CLEAR, finish=GLASS)
    for index in range(10):
        _reflector_cell(cups, filaments, (-450 + index * 100, 0, bottom), mouth=84.0, depth=38.0)
    # The arms have to stand outside the bar's ends, which is where a sunstrip's
    # brackets really are; the frame takes its span from the widest part it measures.
    hang_from_bracket(model, bar_depth=80.0, arm_thickness=14.0)
    return model


def blinders() -> list[Model]:
    """3.13 — 2, 4 and 8 cell blinders sharing one 180 sealed-beam cell."""

    built: list[Model] = []
    for name, columns, rows, frame in (
        ("blinder-2-cell", 2, 1, (400.0, 200.0)),
        ("blinder-4-cell", 2, 2, (400.0, 400.0)),
        ("blinder-8-cell", 4, 2, (800.0, 400.0)),
    ):
        model = Model(
            name,
            "lamps",
            f"Blinder {columns * rows} cell: frame {frame[0]:.0f} x {frame[1]:.0f} x 155, "
            f"sealed beams in chromed rings",
        )
        top, bottom = 78.0, -78.0
        # A tray, not a solid block: the cells have to be visible from in front, and the
        # reader has no transparency that would let an operator see into a closed box.
        body = model.part("body", HOUSING_BLACK)
        body.box((frame[0], frame[1], 26), (0, 0, top - 13))
        body.box((frame[0] - 24, frame[1] - 24, 26), (0, 0, top + 12))
        for side in (-1, 1):
            body.box((18, frame[1], 130), (side * (frame[0] / 2 - 9), 0, bottom + 65))
            body.box((frame[0] - 36, 18, 130), (0, side * (frame[1] / 2 - 9), bottom + 65))
        lens = model.part("cell-lenses", LAMP_GLASS, finish=GLASS)
        for column in range(columns):
            for row in range(rows):
                x = (column - (columns - 1) / 2) * 190
                y = (row - (rows - 1) / 2) * 190
                index = column * rows + row + 1
                cell = model.part(f"cell-{index}", CHROME_RING, finish=METAL)
                _sealed_beam_cell(cell, lens, (x, y, bottom), mouth=178.0)
        hang_from_bracket(model, bar_depth=90.0, arm_thickness=20.0)
        built.append(model)
    return built


def strobe() -> Model:
    """3.14 — one straight xenon tube down the middle of a white reflector well."""

    model = Model(
        "strobe-xenon",
        "lamps",
        "Strobe: 420 x 200 x 260, straight xenon tube in a white reflector well",
    )
    top, bottom = 130.0, -130.0
    # The window is an opening, not a pane: the reader throws away alpha, so a sheet of
    # glass in front of the tube would simply hide the one feature that says "strobe".
    #
    # The surround is shallow and the optics sit close under it on purpose. A deep well
    # with the tube at the back of it is only ever seen from dead underneath; from the
    # three-quarter angle a rig is actually looked at, the front wall hides everything and
    # the fixture is a black box with nothing in it.
    body = model.part("body", HOUSING_BLACK)
    body.box((420, 200, 195), (0, 0, top - 97.5))
    for side in (-1, 1):
        body.box((24, 200, 65), (side * 198, 0, bottom + 32.5))
        body.box((420, 24, 65), (0, side * 88, bottom + 32.5))

    # The well is lined, not just backed. A strobe is white inside — that is what makes the
    # flash come out of the whole aperture instead of off one plate — and a black slot with
    # a silver floor at the bottom of it reads as a hole in a box.
    reflector = model.part("reflector-well", REFLECTOR, finish=METAL)
    reflector.box((372, 152, 8), (0, 0, bottom + 30))
    for side in (-1, 1):
        reflector.box((372, 6, 30), (0, side * 72, bottom + 13), rotation=(side * 12, 0, 0))
        reflector.box((6, 152, 30), (side * 182, 0, bottom + 13), rotation=(0, -side * 12, 0))

    # One straight tube on the axis of the well, level with the bezel. Real strobes of this
    # size are a single linear flashtube between two electrode blocks; the folded tube this
    # model used to carry belongs to a compact camera flash, not to a stage fixture.
    tube = model.part("xenon-source", LENS_CLEAR, finish=GLASS)
    tube.cylinder(16, 320, (0, 0, bottom + 8), segments=10, rotation=(0, 90, 0))
    electrodes = model.part("tube-electrodes", SILVER, BASE, tube)
    for side in (-1, 1):
        electrodes.cylinder(26, 30, (side * 166, 0, bottom + 8), segments=8, rotation=(0, 90, 0))

    bezel = model.part("lens-ring", RIM_ON_BLACK)
    for side in (-1, 1):
        bezel.box((14, 200, 26), (side * 203, 0, bottom + 13))
        bezel.box((420, 14, 26), (0, side * 93, bottom + 13))
    hang_from_bracket(model, bar_depth=90.0, arm_thickness=20.0)
    return model


def led_strobe() -> Model:
    """3.15 — a flat diffusion plane where 3.14 has a tube."""

    model = Model("led-strobe", "lamps", "LED strobe: 500 x 130 x 200, flat diffusion plane")
    top, bottom = 100.0, -100.0
    body = model.part("body", HOUSING_DARK)
    body.box((500, 130, 200), (0, 0, 0))
    housing = model.part("driver-housing", HOUSING_DARK, BASE, body)
    housing.box((320, 90, 46), (0, 0, top + 20))
    # Proud of the housing, not inside it. The flat white plane is the whole difference
    # between this and 3.14, and set flush into a solid body it is simply not there.
    diffuser = model.part("diffuser", WHITE_DIFFUSER, finish=DIFFUSER)
    diffuser.box((464, 96, 18), (0, 0, bottom - 5))
    bezel = model.part("lens-ring", RIM_ON_DARK)
    for side in (-1, 1):
        bezel.box((14, 120, 30), (side * 239, 0, bottom - 5))
        bezel.box((492, 14, 30), (0, side * 54, bottom - 5))
    hang_from_bracket(model, bar_depth=70.0, arm_thickness=16.0)
    return model


# --------------------------------------------------------------------------------------
# 3.16 – 3.17 Strips and atmosphere
# --------------------------------------------------------------------------------------


def led_strips() -> list[Model]:
    """3.16 — one section, six lengths, so the family reads as one product."""

    built: list[Model] = []
    for length in (500, 1000, 1500, 2000, 2500, 3000):
        model = Model(
            f"led-strip-rgbcct-{length:04d}",
            "lamps",
            f"LED strip RGBCCT {length} mm: 50 x 45 extrusion with a milky diffuser",
        )
        # No bracket. A batten is not hung in a yoke like a lantern — the clamps bolt through
        # its own extrusion, so the profile's top face *is* the mounting face and the origin
        # sits on the bolt line. Every other lamp here gets a frame because it has to be
        # aimable; a strip is fixed where it is bolted.
        top = -37.0
        body = model.part("strip-body", SILVER)
        body.box((length, 50, 33), (0, 0, top - 16.5))
        body.box((length, 34, 10), (0, 0, top - 38))
        diffuser = model.part("diffuser", WHITE_DIFFUSER, finish=DIFFUSER)
        diffuser.box((length - 8, 44, 14), (0, 0, top - 60))
        # A clamp about every metre, spread symmetrically, so the origin — the point between
        # them — stays the middle of the strip.
        count = max(2, round(length / 1000) + 1)
        span = length - 160.0
        mounts = model.part("mounting-plates", HOUSING_BLACK)
        places = tuple(-span / 2 + index * span / (count - 1) for index in range(count))
        for x in places:
            mounts.box((70, 62, 14), (x, 0, top + 7))
        truss_coupler(model, offsets=places)
        built.append(model)
    return built


def hazer() -> Model:
    """3.17 — a floor unit, so the origin sits on the floor and nothing hangs."""

    model = Model(
        "hazer",
        "lamps",
        "Hazer: 550 x 350 x 300 chassis on feet, nozzle at the front top",
        origin="floor level, centred on the chassis",
    )
    feet = model.part("feet", HOUSING_BLACK)
    for x in (-230, 230):
        for y in (-140, 140):
            feet.cylinder(56, 30, (x, y, 15), segments=10)
    chassis = model.part("chassis", HOUSING_BLACK)
    chassis.box((550, 350, 300), (0, 0, 180))
    chassis.box((470, 300, 24), (0, 0, 342))
    grille = model.part("fan-grille", SILVER, BASE, chassis)
    grille.cylinder(200, 12, (0, 178, 170), segments=20, rotation=(90, 0, 0))
    grille.box((210, 8, 210), (0, 182, 170))
    nozzle = model.part("nozzle-aperture", SILVER)
    nozzle.cone(90, 70, 80, (0, -215, 288), segments=16, rotation=(90, 0, 0))
    nozzle.cylinder(110, 20, (0, -178, 288), segments=16, rotation=(90, 0, 0))
    tank = model.part("fluid-tank", LENS_CLEAR, finish=GLASS)
    tank.box((36, 240, 190), (-283, 0, 170))
    tank.cylinder(60, 30, (-283, 0, 285), segments=12)
    controls = model.part("control-face", SILVER)
    controls.box((150, 8, 70), (170, 178, 300))
    return model


# --------------------------------------------------------------------------------------
# Mirror scanners
# --------------------------------------------------------------------------------------


def _scanner(
    name: str,
    summary: str,
    body: tuple[float, float, float],
    mirror: float,
) -> Model:
    """A mirror scanner: a fixed profile lantern shining down onto a gimbaled mirror.

    A scanner is not a moving mirror on its own. It is a whole lamp — lamp house, gate,
    lens tube — bolted rigid inside a chassis, with the only moving part a mirror hung in
    a gimbal under the nose: the fork turns about the optical axis and the mirror rocks in
    the fork, and between them they put the beam anywhere. Drawn as a plate on a stalk it
    reads as a moving mirror with no lamp behind it, which is the wrong fixture entirely.

    The optics sit on the pan axis with the chassis spread fore and aft around them,
    because the reader turns everything that pans about the model's own vertical through
    the origin. A scanner modelled with its mirror out at the nose would swing that mirror
    through an arc the size of the fixture.
    """

    width, depth, height = body
    model = Model(name, "lamps", summary)

    chassis = model.part("scanner-chassis", HOUSING_DARK)
    chassis.box((width, depth, height), (0, 0, 0))
    chassis.box((width - 40, depth - 60, 20), (0, 0, height / 2 + 10))
    vents = model.part("chassis-vents", SILVER, BASE, chassis)
    for step in range(3):
        vents.box((width - 70, 10, 16), (0, depth / 2 - 40 - step * 34, height * 0.1))
    display = model.part("chassis-display", SILVER, BASE, chassis)
    display.box((70, 10, 34), (width / 5, -depth / 2 - 5, -height * 0.2))

    # The lantern inside: a finned house on top, the gate in the chassis, the lens tube
    # out of the bottom. Named "optics" throughout — "lamp" would tilt it.
    house = model.part("optics-house", HOUSING_BLACK)
    house.revolve(
        [
            (0.0, 0.0),
            (width * 0.34, 0.0),
            (width * 0.34, height * 0.38),
            *arc_outline(
                (0.0, height * 0.38), width * 0.34, 0.0, 90.0, steps=4, rise=height * 0.22
            ),
        ],
        (0, 0, height / 2 + 20),
        segments=18,
    )
    for step in range(3):
        house.tube(
            width * 0.72,
            width * 0.62,
            9,
            (0, 0, height / 2 + 46 + step * 22),
            segments=18,
        )
    nose = model.part("optics-nose", HOUSING_BLACK)
    nose.revolve(
        [
            (0.0, 0.0),
            (width * 0.26, 0.0),
            (width * 0.26, 54.0),
            (width * 0.3, 62.0),
            (width * 0.3, 78.0),
            (0.0, 86.0),
        ],
        (0, 0, -height / 2 - 86),
        segments=18,
    )
    ring = model.part("lens-ring", RIM_ON_DARK)
    ring.tube(width * 0.66, width * 0.5, 20, (0, 0, -height / 2 - 82))
    lens = model.part("lens", LENS_CLEAR, finish=GLASS)
    lens.cylinder(width * 0.5, 14, (0, 0, -height / 2 - 80), segments=18)

    # The gimbal. The fork pans about the optical axis, the mirror rocks in the fork, and
    # the plate is hung at forty-five degrees where a scanner leaves it at rest.
    fork = model.group_node("mirror-yoke", YOKE)
    cheeks = model.part("yoke-cheeks", SILVER, YOKE, fork)
    pivot = -height / 2 - 96 - mirror * 0.72
    for side in (-1, 1):
        cheeks.box((13, mirror * 0.55, mirror * 0.8), (side * (mirror / 2 + 14), 0, pivot + mirror * 0.16))
    cheeks.cylinder(mirror * 0.44, 26, (0, 0, -height / 2 - 100), segments=14)
    backing = model.part("head-mirror-back", HOUSING_BLACK, HEAD, fork)
    backing.box((mirror + 16, mirror + 16, 10), (0, 0, pivot), rotation=(45, 0, 0))
    plate = model.part("head-mirror", "#E4EAF0", HEAD, fork, finish=METAL)
    plate.box((mirror, mirror, 6), (0, 0, pivot + 5), rotation=(45, 0, 0))

    model.swivel(
        "mirror-yoke",
        (0.0, 0.0, 1.0),
        (0.0, 0.0, pivot),
        (-180.0, 180.0),
        "the mirror fork turning about the optical axis",
    )
    model.swivel(
        "head-mirror",
        (1.0, 0.0, 0.0),
        (0.0, 0.0, pivot),
        (-45.0, 45.0),
        "the mirror rocking in its fork; half a turn of the plate is a whole turn of the beam",
    )
    hang_from_bracket(model, bar_depth=depth * 0.5, arm_thickness=10.0)
    return model


def scanners() -> list[Model]:
    """Two mirror scanners: a full-size one and a compact club fixture."""

    return [
        _scanner(
            "scanner-mirror-spot",
            "Mirror scanner: 240 x 420 x 230 chassis, fixed lantern over a 150 gimbaled mirror",
            (240.0, 420.0, 230.0),
            150.0,
        ),
        _scanner(
            "scanner-compact",
            "Compact mirror scanner: 190 x 330 x 180 chassis, 110 gimbaled mirror",
            (190.0, 330.0, 180.0),
            110.0,
        ),
    ]


def models() -> list[Model]:
    """Every lamp of part 3, in the brief's order, plus the mirror scanners."""

    built = [
        moving_head_profile(),
        moving_head_wash(),
        *led_wash_heads(),
        *scanners(),
        profile_spot(),
        fresnel(),
    ]
    built.extend(par_cans())
    built.append(acl())
    built.extend([led_par_pizza(), led_par_x_in_1(), flat_led_par(), sunstrip()])
    built.extend(blinders())
    built.extend([strobe(), led_strobe()])
    built.extend(led_strips())
    built.append(hazer())
    return built
