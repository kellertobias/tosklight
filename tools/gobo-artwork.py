#!/usr/bin/env python3
"""Draw the gobo artwork ToskLight ships, as greyscale masks.

A gobo is glass with metal on it: light passes where the image is white. These are the
repository's own patterns rather than any manufacturer's glass — the shapes every rig has, drawn
so a projected pattern reads as etched rather than as a procedural function. Each is rendered at
four times its final size and resampled, which is what gives the edges the softness a real gate
has.

    python3 tools/gobo-artwork.py            # write assets/gobos/*.png
    python3 tools/gobo-artwork.py --check    # fail if the checked-in artwork is out of date

The packages that use them are assembled by tools/gobo-package.py.
"""

from __future__ import annotations

import argparse
import math
import pathlib
import random
import sys

from PIL import Image, ImageDraw, ImageFilter

SIZE = 512
SUPERSAMPLE = 4
ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "assets" / "gobos"


def canvas() -> tuple[Image.Image, ImageDraw.ImageDraw]:
    """A black gate at working resolution: nothing passes until something is drawn."""
    image = Image.new("L", (SIZE * SUPERSAMPLE, SIZE * SUPERSAMPLE), 0)
    return image, ImageDraw.Draw(image)


def finish(image: Image.Image, blur: float = 0.0) -> Image.Image:
    """Resample to the shipped size, optionally softening the cut first."""
    if blur:
        image = image.filter(ImageFilter.GaussianBlur(blur * SUPERSAMPLE))
    return image.resize((SIZE, SIZE), Image.LANCZOS)


def clip_to_gate(image: Image.Image) -> Image.Image:
    """Mask everything outside the round gate, because glass is round."""
    gate = Image.new("L", image.size, 0)
    ImageDraw.Draw(gate).ellipse((0, 0, image.size[0] - 1, image.size[1] - 1), fill=255)
    return Image.composite(image, Image.new("L", image.size, 0), gate)


def breakup() -> Image.Image:
    """Foliage breakup: the pattern more rigs carry than any other."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    generator = random.Random(20260801)

    def scatter(count: int, fill: int, low: float, high: float) -> None:
        for _ in range(count):
            radius = generator.uniform(low, high) * edge
            angle = generator.uniform(0, math.tau)
            distance = math.sqrt(generator.random()) * 0.5 * edge
            x = edge / 2 + math.cos(angle) * distance
            y = edge / 2 + math.sin(angle) * distance
            stretch = generator.uniform(0.55, 1.8)
            turn = generator.uniform(0, math.tau)
            leaf = Image.new("L", (int(radius * 2 * stretch) + 2, int(radius * 2) + 2), 0)
            ImageDraw.Draw(leaf).ellipse((0, 0, leaf.size[0] - 1, leaf.size[1] - 1), fill=255)
            leaf = leaf.rotate(math.degrees(turn), expand=True)
            image.paste(
                Image.new("L", leaf.size, fill),
                (int(x - leaf.size[0] / 2), int(y - leaf.size[1] / 2)),
                leaf,
            )

    # Light through the canopy: a scatter of openings, then leaves closing some of them again.
    # A breakup that is one solid mass with a ragged edge projects as a blob, not as foliage.
    scatter(150, 255, 0.03, 0.10)
    scatter(120, 0, 0.02, 0.07)
    scatter(70, 255, 0.012, 0.035)
    return finish(image, blur=0.003)


def dots() -> Image.Image:
    """An even field of dots, the pattern a wash of texture is made from."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    step = edge / 9
    radius = step * 0.26
    for row in range(10):
        for column in range(10):
            offset = step / 2 if row % 2 else 0
            x = column * step + offset
            y = row * step
            draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=255)
    return finish(image, blur=0.002)


def spokes() -> Image.Image:
    """Radial spokes, for a beam that has to read as rays."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    centre = edge / 2
    for index in range(12):
        angle = index * math.tau / 12
        half = math.radians(7.5)
        points = [
            (centre, centre),
            (
                centre + math.cos(angle - half) * edge,
                centre + math.sin(angle - half) * edge,
            ),
            (
                centre + math.cos(angle + half) * edge,
                centre + math.sin(angle + half) * edge,
            ),
        ]
        draw.polygon(points, fill=255)
    return clip_to_gate(finish(image, blur=0.001))


def rings() -> Image.Image:
    """Concentric rings — a target, and the tunnel every projector ships with."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    count = 6
    for index in range(count):
        outer = edge / 2 * (1 - index / count)
        inner = outer - edge / (count * 4)
        draw.ellipse(
            (
                edge / 2 - outer,
                edge / 2 - outer,
                edge / 2 + outer,
                edge / 2 + outer,
            ),
            fill=255 if index % 2 == 0 else 0,
        )
        draw.ellipse(
            (
                edge / 2 - inner,
                edge / 2 - inner,
                edge / 2 + inner,
                edge / 2 + inner,
            ),
            fill=0 if index % 2 == 0 else 255,
        )
    return finish(image, blur=0.002)


def triangle() -> Image.Image:
    """One hard shape, for a projection that has to be a shape rather than a texture."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    centre = edge / 2
    radius = edge * 0.42
    points = [
        (centre + math.cos(angle) * radius, centre + math.sin(angle) * radius)
        for angle in (-math.pi / 2, math.pi / 6, math.pi * 5 / 6)
    ]
    draw.polygon(points, fill=255)
    return finish(image)


def bars() -> Image.Image:
    """Straight bars: a window, a blind, prison bars — whatever the scene calls them."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    step = edge / 7
    for index in range(8):
        top = index * step
        draw.rectangle((0, top, edge, top + step * 0.55), fill=255)
    return clip_to_gate(finish(image, blur=0.002))


def stars() -> Image.Image:
    """A scattered starfield, the one pattern that has to stay sparse to read."""
    image, draw = canvas()
    edge = SIZE * SUPERSAMPLE
    generator = random.Random(6060842)
    for _ in range(38):
        angle = generator.uniform(0, math.tau)
        distance = math.sqrt(generator.random()) * 0.46 * edge
        x = edge / 2 + math.cos(angle) * distance
        y = edge / 2 + math.sin(angle) * distance
        arm = generator.uniform(0.012, 0.045) * edge
        width = arm * 0.22
        draw.polygon(
            [
                (x, y - arm),
                (x + width, y - width),
                (x + arm, y),
                (x + width, y + width),
                (x, y + arm),
                (x - width, y + width),
                (x - arm, y),
                (x - width, y - width),
            ],
            fill=255,
        )
    return finish(image, blur=0.003)


PATTERNS = {
    "breakup": breakup,
    "dots": dots,
    "spokes": spokes,
    "rings": rings,
    "triangle": triangle,
    "bars": bars,
    "stars": stars,
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the checked-in artwork differs from what this would draw",
    )
    arguments = parser.parse_args()

    OUTPUT.mkdir(parents=True, exist_ok=True)
    stale = []
    for name, draw_pattern in PATTERNS.items():
        path = OUTPUT / f"{name}.png"
        image = draw_pattern()
        if arguments.check:
            if not path.exists() or Image.open(path).convert("L").tobytes() != image.tobytes():
                stale.append(path.name)
            continue
        image.save(path, optimize=True)
        print(f"{path.relative_to(ROOT)}  {image.size[0]}x{image.size[1]}")
    if stale:
        print(f"gobo artwork is out of date: {', '.join(stale)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
