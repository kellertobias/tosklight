#!/usr/bin/env python3
"""Derive the manual's product-icon assets from the approved branding artwork.

The help pages show each product's icon and a strip of all three. Those files are committed, but
nothing produced them: they were made by hand, so they drifted whenever the artwork was replaced
and nothing noticed. This regenerates them from the approved sources, so refreshing an icon is one
command rather than three exports somebody has to remember.

    python3 tools/generate_brand_doc_assets.py

The tile order is the order the Quick Start names them in.
"""

from __future__ import annotations

import pathlib
import sys

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the message is the whole point
    sys.exit("Pillow is required: python3 -m pip install pillow")

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRANDING = ROOT / "assets/branding"
OUT = ROOT / "docs/help/assets/branding"

# Product, approved source, and the manual's file name. The order is the strip's order.
PRODUCTS = [
    ("ToskLight Control", "tosklight-control.png"),
    ("ToskLight Architect", "tosklight-architect.png"),
    ("ToskLight Pixel", "tosklight-pixel.png"),
]

SINGLE = 256
TILE = 128


def load(name: str) -> Image.Image:
    source = BRANDING / f"{name}.png"
    if not source.is_file():
        sys.exit(f"no approved artwork at {source}")
    return Image.open(source).convert("RGBA")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    images = [(name, load(name), filename) for name, filename in PRODUCTS]

    for name, image, filename in images:
        image.resize((SINGLE, SINGLE), Image.LANCZOS).save(OUT / filename)
        print(f"wrote {filename} from {name}")

    strip = Image.new("RGBA", (TILE * len(images), TILE), (0, 0, 0, 0))
    for index, (_, image, _) in enumerate(images):
        strip.paste(image.resize((TILE, TILE), Image.LANCZOS), (index * TILE, 0))
    strip.save(OUT / "tosklight-suite.png")
    print(f"wrote tosklight-suite.png from {len(images)} icons")


if __name__ == "__main__":
    main()
