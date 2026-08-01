"""Projection and laser: the boxes that share a rig with the lamps.

They are not in the brief's lamp set, but they hang off the same truss and an operator
plotting a rig needs to see them. Each carries the same switchable ``truss-coupler`` as
a lamp, and each hangs in a bracket it can actually be aimed in.

Unlike a lantern these two do **not** shoot out of their underside. A projector's lens
and a laser's aperture are in the front face, level with the middle of the body, and a
model that puts them underneath reads as a downlight and tells an operator the wrong
thing about where the picture goes.
"""

from __future__ import annotations

from .kit import (
    BASE,
    HOUSING_BLACK,
    LENS_CLEAR,
    METAL,
    Model,
    PAINT,
    SILVER,
)
from .lamps import hang_from_bracket

CASE_GREY = "#4A4E54"
WARNING_YELLOW = "#E8C21A"
LASER_APERTURE = "#141416"


def _projector(name: str, summary: str, width: float, depth: float, height: float, lens: float) -> Model:
    model = Model(name, "av", summary)

    body = model.part("body", CASE_GREY, BASE, None, PAINT)
    body.box((width, depth, height), (0, 0, 0))
    body.box((width - 60, depth - 60, 16), (0, 0, height / 2 - 4))
    feet = model.part("feet", HOUSING_BLACK, BASE, body)
    for x in (-1, 1):
        for y in (-1, 1):
            feet.cylinder(
                46,
                18,
                (x * (width / 2 - 60), y * (depth / 2 - 60), height / 2 - 4),
                segments=8,
            )
    intake = model.part("intake-grille", HOUSING_BLACK, BASE, body)
    intake.box((width * 0.34, 10, height * 0.5), (width * 0.3, -depth / 2 - 4, 0))
    exhaust = model.part("exhaust-grille", HOUSING_BLACK, BASE, body)
    exhaust.box((width * 0.5, 10, height * 0.55), (0, depth / 2 + 4, 0))

    # The barrel stands out of the front face on the optical axis, offset towards one
    # side the way an install projector's lens actually sits.
    axis = (-width * 0.16, 0.0, 0.0)
    barrel = model.part("lens-barrel", HOUSING_BLACK)
    barrel.tube(
        lens + 52,
        lens,
        74,
        (axis[0], -depth / 2 - 37, 0),
        segments=24,
        rotation=(90, 0, 0),
    )
    barrel.tube(
        lens + 74,
        lens + 44,
        20,
        (axis[0], -depth / 2 - 10, 0),
        segments=24,
        rotation=(90, 0, 0),
    )
    glass = model.part("lens", LENS_CLEAR)
    glass.revolve(
        [(0.0, 0.0), (lens / 2, 14.0), (lens / 2, 26.0), (0.0, 40.0)],
        (axis[0], -depth / 2 - 30, 0),
        segments=24,
        rotation=(90, 0, 0),
    )
    hang_from_bracket(model, bar_depth=depth * 0.55, arm_thickness=12.0, colour=CASE_GREY)
    return model


def projectors() -> list[Model]:
    """A small install projector and a large venue one."""

    return [
        _projector(
            "projector-small",
            "Projector, small: 420 x 380 x 150 body with a 110 lens in the front face",
            420.0,
            380.0,
            150.0,
            110.0,
        ),
        _projector(
            "projector-large",
            "Projector, large: 620 x 560 x 260 body with a 190 lens in the front face",
            620.0,
            560.0,
            260.0,
            190.0,
        ),
    ]


def show_laser() -> Model:
    """A projector-shaped body with a square aperture in the front instead of a lens.

    The yellow triangle is the whole point: a laser has to be identifiable as a laser
    from across a plan, and it has no barrel to say so.
    """

    model = Model(
        "show-laser",
        "av",
        "Show laser: 340 x 300 x 170 body, square aperture in the front face, warning triangle",
    )
    width, depth, height = 340.0, 300.0, 170.0

    body = model.part("body", HOUSING_BLACK)
    body.box((width, depth, height), (0, 0, 0))
    body.box((width - 60, depth - 60, 16), (0, 0, height / 2 - 4))
    vents = model.part("exhaust-grille", SILVER, BASE, body, METAL)
    vents.box((250, 10, 90), (0, depth / 2 + 4, 0))

    surround = model.part("aperture-surround", SILVER)
    for side in (-1, 1):
        surround.box((10, 26, 120), (side * 55, -depth / 2 - 9, 0))
        surround.box((120, 26, 10), (0, -depth / 2 - 9, side * 55))
    aperture = model.part("aperture-window", LASER_APERTURE)
    aperture.box((100, 10, 100), (0, -depth / 2 - 4, 0))

    sticker = model.part("warning-sticker", WARNING_YELLOW)
    sticker.revolve(
        [(0.0, 0.0), (46.0, 0.0), (46.0, 3.0), (0.0, 3.0)],
        (110, -depth / 2 - 2, 0),
        segments=3,
        rotation=(90, 0, 0),
    )
    hang_from_bracket(model, bar_depth=depth * 0.55, arm_thickness=12.0, colour=CASE_GREY)
    return model


def models() -> list[Model]:
    """Projection and laser."""

    return [*projectors(), show_laser()]
