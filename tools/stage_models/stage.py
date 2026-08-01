"""Stage and venue elements of part 5 of the fixture and stage model brief.

Decks, railings, curtains and mirror balls. None of these hangs from a clamp, so the
origin is stated per model: floor level for decks, railings and the ground support, the
hanging point for a curtain or a mirror ball.
"""

from __future__ import annotations

import math

from .kit import (
    BASE,
    CABLE_BLACK,
    COUPLER,
    FABRIC,
    HOUSING_BLACK,
    MIRROR_TILE,
    Model,
    SILVER,
)
from .lamps import truss_coupler

DECK_SIZES = ((1000, 500), (1000, 1000), (2000, 1000))
LEG_HEIGHTS = (200, 400, 600, 800, 1000)
CURTAIN_WIDTHS = (2000, 3000, 4000, 6000)
CURTAIN_HEIGHTS = (3000, 4000, 6000, 8000)
CURTAIN_FINISHES = (("black", "#141416"), ("grey", "#9AA0A6"))
MIRROR_BALL_DIAMETERS = (200, 300, 400, 500)

DECK_TOP = "#232326"
FOLD_PITCH = 225.0
# Fabric does not gather evenly. Six depths that do not repeat on any short cycle, so a
# six-metre drape never shows the same pair of folds twice running; the back face reads
# the sequence three folds out of step, which is what stops a drape modelled from both
# sides from looking like corrugated sheet.
FOLD_DEPTHS = (120.0, 55.0, 96.0, 38.0, 140.0, 72.0)
# Folds are not the same width either. These weights average one, so the finished width
# is unchanged and only the spacing wanders.
FOLD_WIDTHS = (1.14, 0.82, 1.02, 0.88, 1.2, 0.94)
BACK_FULLNESS = 0.45
# Bigger facets than a real ball's 25 mm tiles. A mirror ball is scenery, and at the
# size a desk draws it the flash comes from having flat faces at all, not from their
# count; 25 mm tiles cost six times the triangles for a difference nobody sees.
TILE = 40.0


def decks() -> list[Model]:
    """5.1 — a 40 mm top with an aluminium edge, on four legs with adjustable feet.

    Top and legs are separate objects so a show can stack a deck without its legs.
    """

    built: list[Model] = []
    for width, depth in DECK_SIZES:
        for height in LEG_HEIGHTS:
            model = Model(
                f"deck-{width}x{depth}-legs-{height:04d}",
                "stage",
                f"Stage deck {width} x {depth} at {height} mm leg height",
                origin="floor level, centred under the deck",
            )
            top = model.part("deck-top", DECK_TOP)
            top.box((width - 60, depth - 60, 34), (0, 0, height + 20))
            edge = model.part("deck-edge", SILVER)
            for side in (-1, 1):
                edge.box((width, 30, 40), (0, side * (depth / 2 - 15), height + 20))
                edge.box((30, depth - 60, 40), (side * (width / 2 - 15), 0, height + 20))
            legs = model.part("deck-legs", SILVER)
            feet = model.part("deck-feet", CABLE_BLACK)
            for x in (-(width / 2 - 70), width / 2 - 70):
                for y in (-(depth / 2 - 70), depth / 2 - 70):
                    legs.cylinder(48, height - 24, (x, y, (height - 24) / 2 + 24), segments=10)
                    feet.cylinder(70, 24, (x, y, 12), segments=10)
            built.append(model)
    return built


def railings() -> list[Model]:
    """5.2 — 1000 high, uprights every 1200, a knee rail and a toe board."""

    built: list[Model] = []
    for length in (500, 1000, 2000):
        model = Model(
            f"railing-{length:04d}",
            "stage",
            f"Guardrail {length} mm: 1000 high, 40 top rail, 32 knee rail, 100 toe board",
            origin="floor level, centred along the run",
        )
        posts = model.part("railing-posts", HOUSING_BLACK)
        count = max(2, math.ceil(length / 1200) + 1)
        positions = [-length / 2 + index * length / (count - 1) for index in range(count)]
        for x in positions:
            posts.cylinder(40, 1000, (x, 0, 500), segments=10)
        rails = model.part("railing-rails", HOUSING_BLACK)
        rails.cylinder(40, length, (0, 0, 980), segments=10, rotation=(0, 90, 0))
        rails.cylinder(32, length, (0, 0, 520), segments=10, rotation=(0, 90, 0))
        board = model.part("toe-board", HOUSING_BLACK)
        board.box((length, 12, 100), (0, 0, 50))
        built.append(model)
    return built


def _fold_section(width: float, thickness: float = 24.0) -> list[tuple[float, float]]:
    """The wavy cross-section of a drape, as ``(x, depth)`` pairs.

    Both faces are gathered. A drape hung at fullness is cloth pleated onto a webbing,
    not a board with a moulding on the front, and a flat back is the thing that gives
    that away the moment a rig is looked at from upstage or from the side. Fold depths
    come from an irregular sequence rather than alternating between two values, because
    two values repeating are as obviously mechanical as no folds at all.
    """

    folds = max(2, round(width / FOLD_PITCH))
    widths = [FOLD_WIDTHS[fold % len(FOLD_WIDTHS)] for fold in range(folds)]
    pitch = width / sum(widths)
    steps = 6
    front: list[tuple[float, float]] = []
    back: list[tuple[float, float]] = []
    cursor = -width / 2
    for fold in range(folds):
        depth = FOLD_DEPTHS[fold % len(FOLD_DEPTHS)]
        behind = FOLD_DEPTHS[(fold + 3) % len(FOLD_DEPTHS)] * BACK_FULLNESS
        span = pitch * widths[fold]
        for step in range(steps):
            offset = step / steps
            x = cursor + offset * span
            front.append((x, -depth * math.sin(math.pi * offset)))
            back.append((x, thickness + behind * math.sin(math.pi * offset)))
        cursor += span
    front.append((width / 2, 0.0))
    back.append((width / 2, thickness))
    return front + list(reversed(back))


def curtains() -> list[Model]:
    """5.3 — wool serge at 50% fullness, in every width, height and finish."""

    built: list[Model] = []
    for finish, colour in CURTAIN_FINISHES:
        for width in CURTAIN_WIDTHS:
            for height in CURTAIN_HEIGHTS:
                model = Model(
                    f"curtain-{width}x{height}-{finish}",
                    "stage",
                    f"Drape {width} x {height} mm, {finish}, 50% fullness gathered every 450",
                    origin="top centre of the drape, at the hanging bar",
                )
                # Wool serge, not sheet metal: the fabric finish keeps a drape matt while
                # the aluminium in the same picture still catches a highlight.
                drape = model.part("curtain-drape", colour, BASE, None, FABRIC)
                drape.extrusion(
                    _fold_section(float(width)),
                    float(height) - 90,
                    (0, 0, -(height - 90) / 2 - 60),
                    rotation=(-90, 0, 0),
                )
                webbing = model.part("curtain-webbing", colour, BASE, None, FABRIC)
                webbing.box((width, 34, 60), (0, 6, -30))
                ties = model.part("curtain-ties", colour, BASE, None, FABRIC)
                for index in range(max(2, round(width / 300) + 1)):
                    x = -width / 2 + index * width / max(1, round(width / 300))
                    ties.box((16, 8, 90), (x, -14, 30))
                hem = model.part("curtain-hem", colour, BASE, None, FABRIC)
                hem.box((width, 40, 70), (0, 0, -height + 35))
                built.append(model)
    return built


def _mirror_tiles(diameter: float) -> list[tuple[float, float, float]]:
    """Tile centres as ``(radius-independent)`` polar pairs plus their ring latitude."""

    radius = diameter / 2
    rings = max(3, round(math.pi * radius / TILE))
    placed: list[tuple[float, float, float]] = []
    for ring in range(rings):
        polar = math.pi * (ring + 0.5) / rings
        count = max(1, round(math.tau * radius * math.sin(polar) / TILE))
        for index in range(count):
            placed.append((polar, math.tau * index / count, radius))
    return placed


def mirror_balls() -> list[Model]:
    """5.4 — flat tiles in rings, because the flashes come from flat faces.

    Each tile is one quad on a dark core sphere. A tile is only ever seen from outside
    the ball, so the metre ball costs eleven thousand triangles instead of the sixty
    thousand that modelling five thousand thin slabs would take.
    """

    built: list[Model] = []
    for diameter in MIRROR_BALL_DIAMETERS:
        radius = diameter / 2
        drop = 250.0
        centre = -drop - radius
        model = Model(
            f"mirror-ball-{diameter:04d}",
            "stage",
            f"Mirror ball {diameter} mm on a 250 drop chain, faceted with {TILE:.0f} mm tiles",
            origin="mounting point at the hanging eye, ball below",
        )
        coupler = model.part(COUPLER, SILVER)
        coupler.torus(60, 10, (0, 0, -30), segments=12, sides=6, rotation=(0, 90, 0))
        coupler.cylinder(10, drop - 50, (0, 0, -50 - (drop - 50) / 2), segments=6)
        core = model.part("ball-core", HOUSING_BLACK)
        core.sphere(diameter - 3, (0, 0, centre), segments=20, rings=10)
        tiles = model.part("ball-tiles", MIRROR_TILE)
        for polar, azimuth, _ in _mirror_tiles(diameter):
            reach = radius + 1.0
            tiles.plate(
                TILE - 1.5,
                TILE - 1.5,
                (
                    reach * math.sin(polar) * math.cos(azimuth),
                    reach * math.sin(polar) * math.sin(azimuth),
                    centre + reach * math.cos(polar),
                ),
                rotation=(0.0, math.degrees(polar), math.degrees(azimuth)),
            )
        built.append(model)
    return built


def mirror_ball_motor() -> Model:
    """5.4 — the 120 x 120 x 90 box with a rotating hook."""

    model = Model(
        "mirror-ball-motor",
        "stage",
        "Mirror ball motor: 120 x 120 x 90 with a rotating hook",
    )
    truss_coupler(model)
    body = model.part("motor-body", HOUSING_BLACK)
    body.box((120, 120, 90), (0, 0, -75))
    body.box((90, 90, 16), (0, 0, -26))
    shaft = model.part("motor-shaft", SILVER)
    shaft.cylinder(26, 30, (0, 0, -134), segments=10)
    shaft.torus(52, 9, (0, 0, -168), segments=12, sides=6, rotation=(0, 90, 0))
    return model


def models() -> list[Model]:
    """Every stage element of part 5."""

    return [*decks(), *railings(), *curtains(), *mirror_balls(), mirror_ball_motor()]
