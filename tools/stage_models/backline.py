"""Backline and PA: what is actually on the stage the lamps are pointed at.

None of it takes DMX. It exists so a plot reads as a stage rather than as an empty box:
a rig looks right when the drum riser has a kit on it and the wedges are where the
performers are. Everything stands on the floor, so the origin is on the floor under the
middle of the object, and the front — the side an audience sees — faces ``-Y``.
"""

from __future__ import annotations

import math

from .kit import (
    BASE,
    HOUSING_BLACK,
    LENS_CLEAR,
    METAL,
    Model,
    Part,
    SILVER,
    WHITE_DIFFUSER,
)
from .lamps import truss_coupler

CHROME = "#C6CACE"
DRUM_SHELL = "#7A1F24"
DRUM_SKIN = "#EDE7DC"
CABINET = "#1C1C1F"
GRILLE = "#333338"
# A driver has to read against a black cabinet from any angle. Paper cones really are a
# shade up from the box they are screwed to, and the dust cap is lighter again; drawn all
# one near-black the whole speaker disappears into the baffle.
CONE = "#2E2F33"
DUST_CAP = "#45474C"
WOOD = "#8A5A2B"
BRASS_HORN = "#C9A227"
FLOOR_ORIGIN = "floor level, centred under the object, front facing -Y"


def _stand_tripod(part: Part, height: float, spread: float, at: tuple[float, float, float]) -> None:
    """Three legs and a column: mic stands, cymbal stands and speaker poles all share it."""

    x, y, _ = at
    for index in range(3):
        angle = math.tau * index / 3
        part.strut(
            18,
            (x, y, 30.0),
            (x + spread * math.cos(angle), y + spread * math.sin(angle), 0.0),
            segments=6,
        )
    part.cylinder(30, height, (x, y, height / 2), segments=10)


def drum_kit() -> Model:
    """Kick, snare, two rack toms, a floor tom, hi-hat and two cymbals."""

    model = Model("drum-kit", "backline", "Drum kit: 22 kick, snare, two toms, floor tom, three cymbals", origin=FLOOR_ORIGIN)
    shells = model.part("drum-shells", DRUM_SHELL)
    skins = model.part("drum-skins", DRUM_SKIN)
    hardware = model.part("drum-hardware", CHROME, BASE, None, METAL)

    shells.cylinder(560, 460, (0, 240, 290), segments=24, rotation=(90, 0, 0))
    skins.cylinder(544, 8, (0, 12, 290), segments=24, rotation=(90, 0, 0))
    skins.cylinder(200, 6, (0, 6, 290), segments=16, rotation=(90, 0, 0))
    for side in (-1, 1):
        hardware.strut(26, (side * 250, 130, 40), (side * 250, 400, 40), segments=6)

    # A rack tom's head is on the end of its shell, and the shell is tilted, so the head
    # has to be offset along the tilted axis. Sine and cosine the wrong way round put it
    # somewhere off the side of the drum, which is where these two used to sit.
    lean = math.radians(20.0)
    for offset, diameter, depth, height in ((-170.0, 300.0, 260.0, 700.0), (150.0, 330.0, 280.0, 690.0)):
        shells.cylinder(diameter, depth, (offset, 250, height), segments=20, rotation=(20, 0, 0))
        skins.cylinder(
            diameter - 12,
            8,
            (
                offset,
                250 - depth / 2 * math.sin(lean),
                height + depth / 2 * math.cos(lean),
            ),
            segments=20,
            rotation=(20, 0, 0),
        )
        hardware.strut(24, (offset, 250, height - depth / 2), (offset * 0.6, 260, 380), segments=6)

    shells.cylinder(400, 400, (430, 330, 340), segments=20)
    skins.cylinder(386, 8, (430, 330, 542), segments=20)
    for index in range(3):
        angle = math.tau * index / 3
        hardware.strut(20, (430 + 190 * math.cos(angle), 330 + 190 * math.sin(angle), 0), (430 + 190 * math.cos(angle), 330 + 190 * math.sin(angle), 540), segments=6)

    shells.cylinder(360, 150, (-330, 120, 620), segments=20)
    skins.cylinder(348, 8, (-330, 120, 698), segments=20)
    _stand_tripod(hardware, 620, 200, (-330, 120, 0))

    cymbals = model.part("cymbals", BRASS_HORN, BASE, None, METAL)
    for x, y, height, diameter in ((-560.0, 330.0, 1420.0, 420.0), (520.0, 60.0, 1330.0, 460.0), (-260.0, -60.0, 820.0, 360.0)):
        cymbals.revolve(
            [(0.0, 0.0), (diameter / 2 * 0.35, 6.0), (diameter / 2, 22.0), (diameter / 2, 26.0), (0.0, 14.0)],
            (x, y, height),
            segments=20,
        )
        _stand_tripod(hardware, height, 230, (x, y, 0))
    pedal = model.part("kick-pedal", CHROME, BASE, None, METAL)
    pedal.box((120, 260, 30), (0, -80, 20))
    pedal.strut(24, (0, -80, 30), (0, 40, 210), segments=6)
    return model


def guitar_amp() -> Model:
    """A 4x12 cabinet with a head on top: the silhouette everyone recognises."""

    model = Model("guitar-amp", "backline", "Guitar stack: 4x12 cabinet with a head", origin=FLOOR_ORIGIN)
    cabinet = model.part("cabinet", CABINET)
    cabinet.box((760, 380, 760), (0, 0, 400))
    # The front is a border round the baffle, not a slab across it. A 4x12's cloth really
    # does cover the whole face, and modelled that way the cabinet is a black box: the four
    # speakers behind it are the only thing that says which cabinet this is.
    for side in (-1, 1):
        cabinet.box((790, 30, 60), (0, -190, 400 + side * 365))
        cabinet.box((60, 30, 790), (side * 365, -190, 400))
    feet = model.part("cabinet-feet", HOUSING_BLACK, BASE, cabinet)
    for x in (-1, 1):
        for y in (-1, 1):
            feet.box((90, 90, 40), (x * 300, y * 140, 20))
    grille = model.part("cabinet-grille", GRILLE)
    cones = model.part("speaker-cones", CONE)
    caps = model.part("dust-caps", DUST_CAP, BASE, cones)
    for x in (-1, 1):
        for z in (1, -1):
            at = (x * 170, -200, 400 + z * 170)
            cones.revolve(
                [(0.0, 0.0), (60.0, 26.0), (150.0, 40.0), (150.0, 46.0), (0.0, 20.0)],
                at,
                segments=16,
                rotation=(-90, 0, 0),
            )
            grille.tube(330, 290, 16, at, segments=18, rotation=(-90, 0, 0))
            caps.dome(108, 30, at, segments=14, rotation=(90, 0, 0))
    top_box = model.part("amp-top", CABINET)
    top_box.box((740, 300, 250), (0, 0, 905))
    face = model.part("amp-top-face", SILVER, BASE, top_box)
    face.box((640, 20, 150), (0, -152, 920))
    knobs = model.part("amp-top-knobs", CHROME, BASE, top_box, METAL)
    for step in range(7):
        knobs.cylinder(34, 22, (-260 + step * 88, -164, 960), segments=8, rotation=(90, 0, 0))
    lamp_glow = model.part("amp-indicator", LENS_CLEAR, BASE, top_box)
    lamp_glow.cylinder(26, 18, (300, -160, 960), segments=8, rotation=(90, 0, 0))
    return model


def guitar_in_stand() -> Model:
    """An electric guitar standing in an A-frame, neck up and face to the audience.

    The whole instrument is laid out along one leaning axis and the parts are placed at
    distances along it, which is the only way the body, the neck and the headstock end up
    as one object: built independently they drift apart and read as a pile of firewood.
    """

    model = Model("guitar-in-stand", "backline", "Electric guitar on an A-frame stand", origin=FLOOR_ORIGIN)
    stand = model.part("guitar-stand", HOUSING_BLACK)
    for side in (-1, 1):
        stand.strut(26, (side * 210, 170, 0), (side * 74, 30, 660), segments=6)
        stand.strut(22, (side * 150, 120, 250), (side * 150, -180, 30), segments=6)
    stand.strut(22, (-150, 120, 250), (150, 120, 250), segments=6)
    stand.strut(22, (-74, 30, 640), (74, 30, 640), segments=6)

    lean = math.radians(8.0)
    base = (0.0, 20.0, 300.0)
    direction = (0.0, math.sin(lean), math.cos(lean))

    def along(distance: float, out: float = 0.0) -> tuple[float, float, float]:
        return (
            base[0],
            base[1] + direction[1] * distance - out,
            base[2] + direction[2] * distance,
        )

    body = model.part("guitar-body", WOOD)
    # A double cutaway: a big lower bout and a smaller upper one, overlapping. The face
    # is turned to the audience and tipped back with the instrument.
    for distance, radius in ((92.0, 175.0), (286.0, 148.0)):
        body.revolve(
            [(0.0, 0.0), (radius, 6.0), (radius, 46.0), (0.0, 52.0)],
            along(distance),
            segments=18,
            rotation=(82, 0, 0),
        )
    neck = model.part("guitar-neck", "#3A2A1C")
    neck.strut(58, along(390.0, 6.0), along(1010.0, 22.0), segments=10)
    neck.strut(80, along(1010.0, 22.0), along(1090.0, 26.0), segments=8)
    # Clear of the front of the body and the neck, not inside them: the body slab reaches
    # about 50 forward of its own centreline and the neck half that, so strings drawn at 30
    # are buried in the instrument and the guitar comes out unstrung.
    strings = model.part("guitar-strings", CHROME, BASE, None, METAL)
    strings.strut(11, along(90.0, 60.0), along(1012.0, 56.0), segments=6)
    return model


def dj_gear() -> list[Model]:
    """A club mixer and a media player, as separate units so a booth can be built."""

    mixer = Model("dj-mixer", "backline", "DJ mixer: 320 x 400 x 110 club unit", origin=FLOOR_ORIGIN)
    body = mixer.part("mixer-body", CABINET)
    body.box((320, 400, 110), (0, 0, 55))
    face = mixer.part("mixer-face", "#2A2D33", BASE, body)
    face.box((300, 380, 12), (0, 0, 112))
    controls = mixer.part("mixer-controls", SILVER, BASE, body)
    for channel in range(4):
        x = -105 + channel * 70
        for step in range(3):
            controls.cylinder(26, 14, (x, 130 - step * 60, 120), segments=8)
        controls.box((30, 130, 10), (x, -80, 120))
    controls.box((240, 26, 10), (0, -170, 120))

    player = Model("dj-player", "backline", "DJ media player: 320 x 420 x 110 with a jog wheel", origin=FLOOR_ORIGIN)
    deck = player.part("player-body", CABINET)
    deck.box((320, 420, 110), (0, 0, 55))
    jog = player.part("jog-wheel", SILVER)
    jog.revolve(
        [(0.0, 0.0), (100.0, 0.0), (104.0, 10.0), (100.0, 18.0), (0.0, 18.0)],
        (0, -70, 110),
        segments=24,
    )
    screen = player.part("player-screen", LENS_CLEAR)
    screen.box((240, 110, 10), (0, 140, 112))
    buttons = player.part("player-buttons", "#2A2D33")
    for step in range(4):
        buttons.box((60, 34, 10), (-105 + step * 70, 40, 112))
    return [mixer, player]


def _splayed(
    hinge: tuple[float, float, float],
    angle: float,
    across: float,
    forward: float,
    down: float,
) -> tuple[float, float, float]:
    """A point in a hung box's own frame, rotated by its splay and placed on its hinge.

    ``forward`` runs towards the audience and ``down`` below the hinge, both in the box's
    own axes, so one call places a cabinet, its drivers, and the hinge the next box takes.
    """

    radians = math.radians(angle)
    return (
        across,
        hinge[1] + forward * math.cos(radians) + down * math.sin(radians),
        hinge[2] + forward * math.sin(radians) - down * math.cos(radians),
    )


def _towards(
    point: tuple[float, float],
    target: tuple[float, float],
    distance: float,
) -> tuple[float, float]:
    """A point ``distance`` along the way from ``point`` to ``target``, never past halfway."""

    span = math.hypot(target[0] - point[0], target[1] - point[1])
    step = min(distance, span * 0.45)
    return (
        point[0] + (target[0] - point[0]) / span * step,
        point[1] + (target[1] - point[1]) / span * step,
    )


def _rounded(
    section: list[tuple[float, float]],
    radius: float,
    steps: int = 3,
) -> list[tuple[float, float]]:
    """Round every corner of a closed 2D outline, keeping it in the same winding.

    A moulded cabinet has no sharp arrises on it, and drawn with them a wedge reads as a
    packing crate rather than as a monitor. Three points a corner is enough at the size a
    plot draws one.
    """

    count = len(section)
    rounded: list[tuple[float, float]] = []
    for index, corner in enumerate(section):
        start = _towards(corner, section[index - 1], radius)
        end = _towards(corner, section[(index + 1) % count], radius)
        for step in range(steps + 1):
            along = step / steps
            weight = (1 - along) ** 2, 2 * along * (1 - along), along**2
            rounded.append(
                (
                    weight[0] * start[0] + weight[1] * corner[0] + weight[2] * end[0],
                    weight[0] * start[1] + weight[1] * corner[1] + weight[2] * end[1],
                )
            )
    return rounded


def _cabinet(
    name: str,
    summary: str,
    width: float,
    depth: float,
    height: float,
    drivers: tuple[tuple[float, float, float], ...],
    *,
    wedge: bool = False,
    pole_socket: bool = False,
) -> Model:
    model = Model(name, "backline", summary, origin=FLOOR_ORIGIN)
    box = model.part("cabinet", CABINET)
    if wedge:
        box.extrusion(
            _rounded(
                [
                    (-depth / 2, 0.0),
                    (depth / 2, 0.0),
                    (depth / 2, height * 0.42),
                    (-depth / 2, height),
                ],
                48.0,
            ),
            width,
            (0, 0, 0),
            rotation=(0, 0, 90),
        )
    else:
        box.box((width, depth, height), (0, 0, height / 2))
    # A wedge's driver is in the sloping top, so it has to sit on that surface and lie in
    # it. Placed at a height taken from the front face instead, it ends up inside the
    # cabinet: invisible from underneath, and an empty slope seen from where a monitor is
    # actually looked at, which is over it.
    slope = math.degrees(math.atan2(height * 0.58, depth))
    stand = 6.0
    grille = model.part("grille", GRILLE)
    cones = model.part("speaker-cones", CONE)
    caps = model.part("dust-caps", DUST_CAP, BASE, cones)
    for x, z, diameter in drivers:
        face = -depth / 2 - 6 if not wedge else 0.0
        # The dish is recessed into whatever face carries it. A wedge's sloping face rises
        # towards the front, so its outward normal leans *back* — turning the cone by the
        # slope the other way lays the driver across the face instead of into it.
        tilt = (-90.0, 0.0, 0.0) if not wedge else (180.0 - slope, 0.0, 0.0)
        # Both cases stand the driver six clear of the face it is in — straight out for a
        # flat front, along the face normal for the wedge. Level with the surface the
        # grille ring is buried in the cabinet and the driver reads as a scratch on it.
        lean = math.radians(slope)
        offset = (
            (x, face, z)
            if not wedge
            else (x, stand * math.sin(lean), height * 0.71 + stand * math.cos(lean))
        )
        cones.revolve(
            [(0.0, 0.0), (diameter * 0.2, diameter * 0.09), (diameter / 2, diameter * 0.14), (diameter / 2, diameter * 0.16), (0.0, diameter * 0.07)],
            offset,
            segments=18,
            rotation=tilt,
        )
        # A ring, not a disc. A solid grille plate covers the cone it is meant to protect,
        # and a flat plate lying in a flat baffle shades identically to it — which is how a
        # 300 driver on a wedge ends up invisible on a face it very nearly fills.
        grille.tube(diameter + 30, diameter - 10, 16, offset, segments=18, rotation=tilt)
        # The dust cap bulges back out of the cone's throat, so the driver has a highlight
        # of its own however the fixture is lit.
        outward = (90.0, 0.0, 0.0) if not wedge else (-slope, 0.0, 0.0)
        caps.dome(diameter * 0.36, diameter * 0.1, offset, segments=14, rotation=outward)
    handles = model.part("handles", SILVER)
    for side in (-1, 1):
        handles.box((30, 120, 40), (side * (width / 2 - 12), 0, height * 0.7))
    if pole_socket:
        socket = model.part("pole-socket", HOUSING_BLACK)
        socket.cylinder(60, 60, (0, 0, height - 30), segments=10)
    return model


def speakers() -> list[Model]:
    """Wedges, subs, tops and a flown line-array element."""

    built = [
        _cabinet(
            "stage-monitor",
            "Stage monitor wedge: 560 x 440 x 360",
            560.0,
            440.0,
            360.0,
            ((0.0, 190.0, 300.0),),
            wedge=True,
        ),
        _cabinet(
            "subwoofer",
            "Subwoofer: 600 x 700 x 600 with an 18 driver",
            600.0,
            700.0,
            600.0,
            ((0.0, 300.0, 460.0),),
            pole_socket=True,
        ),
        _cabinet(
            "speaker-top",
            "PA top: 380 x 380 x 620 two-way on a pole socket",
            380.0,
            380.0,
            620.0,
            ((0.0, 240.0, 300.0), (0.0, 480.0, 130.0)),
        ),
    ]
    pole = Model("speaker-on-pole", "backline", "PA top on a tripod pole stand", origin=FLOOR_ORIGIN)
    stand = pole.part("speaker-stand", HOUSING_BLACK)
    _stand_tripod(stand, 1500, 420, (0, 0, 0))
    cabinet = pole.part("cabinet", CABINET)
    cabinet.box((380, 380, 620), (0, 0, 1810))
    # Grille rings round both drivers, as the boxed cabinets get from `_cabinet`. Without
    # them a black cone sits on a black baffle and the top is a plain brick on a stick.
    grille = pole.part("grille", GRILLE)
    cones = pole.part("speaker-cones", CONE)
    caps = pole.part("dust-caps", DUST_CAP, BASE, cones)
    for height, rim, outline in (
        (1750.0, 150.0, [(0.0, 0.0), (60.0, 26.0), (150.0, 40.0), (150.0, 46.0), (0.0, 20.0)]),
        (1980.0, 70.0, [(0.0, 0.0), (30.0, 14.0), (70.0, 22.0), (70.0, 26.0), (0.0, 10.0)]),
    ):
        at = (0, -196, height)
        cones.revolve(outline, at, segments=16, rotation=(-90, 0, 0))
        grille.tube(rim * 2 + 30, rim * 2 - 10, 16, at, segments=18, rotation=(-90, 0, 0))
        caps.dome(rim * 0.72, rim * 0.2, at, segments=14, rotation=(90, 0, 0))
    built.append(pole)

    hang = Model(
        "line-array-hang",
        "backline",
        "Line array: six 900 x 420 elements flown from a frame",
        origin="mounting point at the top of the fly frame",
    )
    truss_coupler(hang, offsets=(-160.0, 160.0))
    frame = hang.part("fly-frame", HOUSING_BLACK)
    frame.box((900, 300, 40), (0, 0, -40))
    for side in (-1, 1):
        frame.strut(24, (side * 420, 0, -60), (side * 300, 0, -260), segments=6)
    # The banana. Each box hangs off the rear-bottom edge of the one above it and adds a
    # little more splay, so the angles accumulate down the hang and the bottom of the
    # array is aimed into the front rows. Boxes stacked square with a fixed tilt each are
    # a ladder, not an array, and they tell an operator the wrong thing about coverage.
    height, depth = 260.0, 420.0
    hinge = (0.0, depth / 2, -260.0)
    angle = 0.0
    for index, splay in enumerate((0.0, 1.0, 2.0, 3.5, 5.5, 8.0)):
        angle += splay
        element = hang.part(f"array-element-{index + 1}", CABINET)
        element.box(
            (900, depth, height),
            _splayed(hinge, angle, 0.0, -depth / 2, height / 2),
            rotation=(angle, 0, 0),
        )
        cones = hang.part(f"array-cones-{index + 1}", HOUSING_BLACK, BASE, element)
        for side in (-1, 1):
            cones.revolve(
                [(0.0, 0.0), (40.0, 18.0), (110.0, 30.0), (110.0, 34.0), (0.0, 14.0)],
                _splayed(hinge, angle, side * 250, -depth + 6, height / 2),
                segments=14,
                rotation=(-90 + angle, 0, 0),
            )
        hinge = _splayed(hinge, angle, 0.0, 0.0, height)
    built.append(hang)
    return built


def microphone_stand() -> Model:
    """A boom stand with a capsule on it, at singing height."""

    model = Model("microphone-stand", "backline", "Boom microphone stand at 1500", origin=FLOOR_ORIGIN)
    stand = model.part("mic-stand", HOUSING_BLACK)
    _stand_tripod(stand, 1420, 300, (0, 0, 0))
    stand.strut(22, (0, 0, 1400), (0, -320, 1500), segments=6)
    capsule = model.part("mic-capsule", SILVER)
    capsule.cylinder(46, 130, (0, -360, 1512), segments=12, rotation=(76, 0, 0))
    capsule.sphere(52, (0, -392, 1520), segments=12, rings=6)
    return model


def stage_piano() -> Model:
    """An 88-note stage piano on an X stand."""

    model = Model("stage-piano", "backline", "Stage piano 1320 x 340 on an X stand", origin=FLOOR_ORIGIN)
    stand = model.part("keyboard-stand", HOUSING_BLACK)
    for side in (-1, 1):
        stand.strut(30, (side * 300, -220, 0), (-side * 300, 220, 700), segments=6)
        stand.strut(30, (side * 300, 220, 0), (-side * 300, -220, 700), segments=6)
    body = model.part("piano-body", CABINET)
    body.box((1320, 340, 110), (0, 0, 755))
    keys = model.part("piano-keys", WHITE_DIFFUSER)
    keys.box((1240, 190, 26), (0, -70, 818))
    blacks = model.part("piano-sharps", HOUSING_BLACK)
    for index in range(35):
        if index % 7 in (2, 6):
            continue
        blacks.box((22, 120, 18), (-600 + index * 35, -100, 838))
    controls = model.part("piano-controls", SILVER)
    controls.box((1200, 90, 12), (0, 120, 816))
    return model


def saxophone_in_stand() -> Model:
    """A tenor sax on its stand: bow at the bottom, body up the back, bell up the front.

    A saxophone is four runs of tube, and which way each one goes is the whole shape: the
    U of the bow at the floor, the tapered body climbing at the back, the bell climbing
    and flaring at the front, and the crook doubling forward over the top to the
    mouthpiece. Spun as one revolve it comes out as a rocket, which is what the first
    pass of this model was.
    """

    model = Model("saxophone-in-stand", "backline", "Tenor saxophone on a stand", origin=FLOOR_ORIGIN)
    stand = model.part("sax-stand", HOUSING_BLACK)
    for index in range(3):
        angle = math.tau * index / 3
        stand.strut(18, (0, 0, 130), (240 * math.cos(angle), 240 * math.sin(angle), 0), segments=6)
    stand.cup(180, 120, 80, 8, (0, 20, 150), segments=14)

    horn = model.part("sax-horn", BRASS_HORN, BASE, None, METAL)
    # The bow, as a run of short segments round the bottom of the U. It has to be wide:
    # the whole instrument is a V of two tubes leaning apart, and drawn narrow the bell
    # and the body sit on top of each other and the sax comes out as one funnel.
    bow = [
        (0.0, 108.0 * math.cos(math.radians(step * 30)), 236.0 - 96.0 * math.sin(math.radians(step * 30)))
        for step in range(7)
    ]
    for start, end in zip(bow, bow[1:]):
        horn.strut(84, start, end, segments=10)
    horn.flare(88, 56, (0, 108, 236), (0, 66, 680), segments=14)
    horn.flare(66, 178, (0, -108, 236), (0, -256, 610), segments=16)
    horn.torus(180, 24, (0, -256, 610), segments=16, sides=6, rotation=(22, 0, 0))
    crook = [(0.0, 66.0, 680.0), (0.0, 16.0, 756.0), (0.0, -84.0, 784.0)]
    for start, end in zip(crook, crook[1:]):
        horn.strut(46, start, end, segments=10)
    mouthpiece = model.part("sax-mouthpiece", HOUSING_BLACK)
    mouthpiece.flare(44, 30, (0, -84, 784), (0, -176, 800), segments=10)

    keys = model.part("sax-keys", SILVER, BASE, None, METAL)
    for step in range(8):
        # On the front of the body tube, which leans back as it climbs.
        keys.cylinder(38, 16, (0, 60 - step * 5, 300 + step * 46), segments=8, rotation=(90, 0, 0))
    return model


def models() -> list[Model]:
    """Everything on the stage that is not a lamp."""

    return [
        drum_kit(),
        guitar_amp(),
        guitar_in_stand(),
        *dj_gear(),
        *speakers(),
        microphone_stand(),
        stage_piano(),
        saxophone_in_stand(),
    ]
