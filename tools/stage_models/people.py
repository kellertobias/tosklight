"""Figures, for scale and for aim.

A rig is plotted around people: where the singer stands is where the key light points,
and a truss height means nothing next to an empty stage. These are deliberately plain
mannequins — a lighting plot wants a silhouette at the right height in the right place,
not a likeness — but what each one is *doing* has to read, because that is the whole
reason a figure is placed rather than a cylinder.

Each stands on the floor facing the audience at ``-Y``, about 1.75 m tall. Facing the
audience means the figure's own left hand is at ``+X``. No node may be called a head:
the reader would tilt it.
"""

from __future__ import annotations

import math

from .kit import BASE, FABRIC, HOUSING_BLACK, METAL, Model, PAINT, Part, SILVER, SKIN as SKIN_FINISH

SKIN = "#C69A78"
SHIRT = "#3C4654"
TROUSERS = "#22262E"
SHOE = "#141416"
GUITAR_WOOD = "#8A5A2B"
GUITAR_NECK = "#3A2A1C"
CHROME = "#C6CACE"
FLOOR_ORIGIN = "floor level, under the figure, facing -Y"

HIP = 900.0
SHOULDER = 1400.0


def _legs(model: Model, stance: float = 130.0, seated: bool = False, height: float = 0.0) -> Part:
    legs = model.part("figure-legs", TROUSERS, BASE, None, FABRIC)
    shoes = model.part("figure-shoes", SHOE, BASE, None, PAINT)
    for side in (-1, 1):
        if seated:
            legs.strut(120, (side * stance, 0, height), (side * stance, -260, height), segments=8)
            legs.strut(110, (side * stance, -260, height), (side * stance, -300, 60), segments=8)
            shoes.box((120, 220, 60), (side * stance, -350, 30))
        else:
            legs.strut(130, (side * stance, 0, HIP), (side * stance * 1.15, 0, 80), segments=8)
            shoes.box((120, 240, 70), (side * stance * 1.15, -40, 35))
    return legs


def _torso(model: Model, top: float = SHOULDER, bottom: float = HIP) -> Part:
    """Hips, chest, neck and skull as one solid, because none of it moves separately."""

    torso = model.part("figure-torso", SHIRT, BASE, None, FABRIC)
    torso.revolve(
        [
            (0.0, 0.0),
            (170.0, 0.0),
            (185.0, 90.0),
            (180.0, 240.0),
            (245.0, 360.0),
            (235.0, 440.0),
            (70.0, 470.0),
            (0.0, 470.0),
        ],
        (0, 0, bottom),
        segments=16,
        scale=(1.0, 0.55, (top - bottom) / 480.0),
    )
    skin = model.part("figure-skin", SKIN, BASE, None, SKIN_FINISH)
    skin.cylinder(90, 70, (0, 0, top + 30), segments=12)
    skin.sphere(220, (0, 0, top + 150), segments=16, rings=10, scale=(1.0, 1.08, 1.15))
    return torso


def _sleeve(part: Part, side: int, elbow: tuple[float, float, float], hand: tuple[float, float, float]) -> None:
    shoulder = (side * 195.0, 0.0, SHOULDER - 40.0)
    part.strut(115, shoulder, elbow, segments=8)
    part.strut(98, elbow, hand, segments=8)


def _curve(
    part: Part,
    diameter: float,
    points: list[tuple[float, float, float]],
    segments: int = 8,
) -> None:
    """A run of struts through a list of points, for anything that bends.

    Cheaper to reason about than a swept torus, and it is the only way to bend a shape
    through a plane the kit's primitives do not already lie in.
    """

    for start, end in zip(points, points[1:]):
        part.strut(diameter, start, end, segments=segments)


def _handheld_microphone(model: Model, hand: tuple[float, float, float], tilt: float) -> None:
    """A vocal mic: a slim tapered barrel with a ball grille, held in the fist.

    Sized off a real SM58 — 50 across the ball, 23 across the handle, 160 long. Drawn any
    fatter it stops reading as a microphone, which is the one thing it has to do.
    """

    reach = math.radians(tilt)
    axis = (0.0, -math.sin(reach), math.cos(reach))
    heel = tuple(hand[index] - axis[index] * 70.0 for index in range(3))
    ball = tuple(hand[index] + axis[index] * 74.0 for index in range(3))
    body = model.part("handheld-microphone", HOUSING_BLACK, BASE, None, PAINT)
    body.strut(23, heel, tuple(hand[index] + axis[index] * 56.0 for index in range(3)), segments=10)
    body.strut(31, tuple(hand[index] + axis[index] * 48.0 for index in range(3)), ball, segments=10)
    grille = model.part("microphone-grille", SILVER, BASE, None, METAL)
    grille.sphere(50, ball, segments=12, rings=6)


def singer() -> Model:
    """Standing, one arm up with a handheld on it."""

    model = Model("figure-singer", "people", "Singer, 1750 tall, microphone in hand", origin=FLOOR_ORIGIN)
    _legs(model)
    _torso(model)
    arms = model.part("figure-sleeves", SHIRT, BASE, None, FABRIC)
    hand = (-210.0, -300.0, 1500.0)
    _sleeve(arms, -1, (-330, -120, 1180), hand)
    _sleeve(arms, 1, (360, -40, 1150), (430, -180, 900))
    _handheld_microphone(model, hand, tilt=54.0)
    return model


def _slung_guitar(model: Model, body_at: tuple[float, float, float], nut: tuple[float, float, float]) -> None:
    """An electric guitar hung across the chest: body, neck, headstock, strap.

    The body is two overlapping discs — the double cutaway everyone draws from memory —
    laid flat against the player rather than spun about the wrong axis, and the neck runs
    from the body to the nut in one piece so the fretting hand has something to be on.
    """

    x, y, z = body_at
    body = model.part("guitar-body", GUITAR_WOOD, BASE, None, PAINT)
    for offset, radius in ((-90.0, 175.0), (90.0, 148.0)):
        body.revolve(
            [(0.0, 0.0), (radius, 6.0), (radius, 46.0), (0.0, 52.0)],
            (x + offset * 0.94, y, z + offset * 0.34),
            segments=18,
            rotation=(90, 0, 0),
        )
    neck = model.part("guitar-neck", GUITAR_NECK, BASE, None, PAINT)
    joint = (x + 130.0, y - 6.0, z + 46.0)
    neck.strut(56, joint, nut, segments=10)
    span = tuple(nut[index] - joint[index] for index in range(3))
    head = tuple(nut[index] + span[index] * 0.13 for index in range(3))
    neck.strut(78, nut, head, segments=8)
    # In front of the body slab and the neck, or the strings are inside the instrument.
    strings = model.part("guitar-strings", CHROME, BASE, None, METAL)
    strings.strut(11, (x - 40.0, y - 62.0, z), (nut[0], nut[1] - 36.0, nut[2]), segments=6)
    strap = model.part("guitar-strap", HOUSING_BLACK, BASE, None, FABRIC)
    _curve(
        strap,
        34,
        [(x - 150.0, y - 20.0, z - 60.0), (-150.0, -40.0, 1380.0), (150.0, 60.0, 1360.0), joint],
        segments=6,
    )


def guitarist() -> Model:
    """Standing, guitar slung across the body, both hands where they should be."""

    model = Model("figure-guitarist", "people", "Guitar player, 1750 tall, guitar in hand", origin=FLOOR_ORIGIN)
    _legs(model, stance=150.0)
    _torso(model)
    body_at = (-70.0, -150.0, 1000.0)
    nut = (600.0, -160.0, 1330.0)
    _slung_guitar(model, body_at, nut)
    arms = model.part("figure-sleeves", SHIRT, BASE, None, FABRIC)
    # Right hand over the bridge, left hand on the neck a third of the way up it.
    _sleeve(arms, -1, (-340, -150, 1160), (-70, -240, 1010))
    _sleeve(arms, 1, (390, -180, 1240), (330, -186, 1188))
    return model


def pianist() -> Model:
    """Seated at a keyboard, arms forward onto it."""

    model = Model("figure-pianist", "people", "Pianist seated at a keyboard, 1300 seated height", origin=FLOOR_ORIGIN)
    stool = model.part("stool", HOUSING_BLACK, BASE, None, PAINT)
    stool.cylinder(360, 70, (0, 120, 590), segments=14)
    for index in range(4):
        angle = math.tau * index / 4 + math.pi / 4
        stool.strut(
            26,
            (150 * math.cos(angle), 120 + 150 * math.sin(angle), 560),
            (190 * math.cos(angle), 120 + 190 * math.sin(angle), 0),
            segments=6,
        )
    _legs(model, stance=120.0, seated=True, height=660.0)
    _torso(model, top=1180.0, bottom=700.0)
    arms = model.part("figure-sleeves", SHIRT, BASE, None, FABRIC)
    arms.strut(100, (-195, 0, 1140), (-260, -220, 1000), segments=8)
    arms.strut(85, (-260, -220, 1000), (-210, -430, 960), segments=8)
    arms.strut(100, (195, 0, 1140), (260, -220, 1000), segments=8)
    arms.strut(85, (260, -220, 1000), (210, -430, 960), segments=8)
    return model


def deejay() -> Model:
    """Standing at the booth: one hand on the deck, the other holding a can to one ear.

    That asymmetry is the pose. Both hands forward is a figure typing, and a deejay with
    headphones round the neck and nothing else is a figure standing still.
    """

    model = Model("figure-deejay", "people", "Deejay at the booth, 1750 tall, cueing one ear", origin=FLOOR_ORIGIN)
    _legs(model, stance=140.0)
    _torso(model)
    arms = model.part("figure-sleeves", SHIRT, BASE, None, FABRIC)
    # Right arm down and forward onto the platter; left arm folded up to the ear cup.
    _sleeve(arms, -1, (-330, -230, 1140), (-250, -450, 1030))
    _sleeve(arms, 1, (330, -140, 1290), (185, -110, 1520))
    cans = model.part("dj-cans", HOUSING_BLACK, BASE, None, PAINT)
    # Worn one-eared, the way a deejay cues: the band sits over the crown at a slant and
    # only the left cup is on the ear, with the right cup swung clear behind the jaw.
    cans.torus(280, 34, (0, 6, 1570), segments=14, sides=6, arc=170.0, rotation=(90, 14, 0))
    cans.cylinder(150, 60, (118, -8, 1528), segments=12, rotation=(0, 90, 0))
    cans.cylinder(150, 60, (-118, 60, 1600), segments=12, rotation=(0, 76, 0))
    return model


def models() -> list[Model]:
    """Four figures, one per role a plot is built around."""

    return [singer(), guitarist(), pianist(), deejay()]
