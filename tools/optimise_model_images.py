#!/usr/bin/env python3
"""Shrink the model renders before they are committed.

A flat-shaded render over transparency is a few dozen flat colours stored as full
truecolour, which costs about twelve times what it needs to. Quantising to a palette is
lossless to the eye here — there are no gradients to band — and takes the model
catalogue from twelve megabytes of tracked PNG to about one.

    python3 tools/optimise_model_images.py docs/help/assets/models
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - reported, not raised, so a build can say why
    Image = None

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_IMAGES = ROOT / "docs" / "help" / "assets" / "models"
COLOURS = 128


def optimise(path: Path) -> tuple[int, int]:
    before = path.stat().st_size
    with Image.open(path) as opened:
        image = opened.convert("RGBA")
    image.quantize(colors=COLOURS, method=Image.FASTOCTREE).save(path, optimize=True)
    return before, path.stat().st_size


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("images", nargs="?", type=Path, default=DEFAULT_IMAGES)
    arguments = parser.parse_args()
    if Image is None:
        print("optimise error: Pillow is not installed in this interpreter", file=sys.stderr)
        return 1
    paths = sorted(arguments.images.rglob("*.png"))
    if not paths:
        print(f"optimise error: no PNGs under {arguments.images}", file=sys.stderr)
        return 1
    before = after = 0
    for path in paths:
        was, now = optimise(path)
        before += was
        after += now
    print(f"Optimised {len(paths)} renders: {before // 1024} KiB to {after // 1024} KiB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
