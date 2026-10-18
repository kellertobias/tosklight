#!/usr/bin/env python3
"""Regenerate the bundled glyph atlas: python tools/generate-tracking-font.py FONT.ttf.

Use JetBrains Mono Regular (SIL OFL), 22 px, ASCII 32..127, 16 x 6 cells of 16 x 32.
The font's OFL notice lives beside the raster. Pillow is required only for regeneration.
"""
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont

font = ImageFont.truetype(sys.argv[1], 22)
image = Image.new('L', (256, 192))
draw = ImageDraw.Draw(image)
for code in range(32, 128):
    index = code - 32
    draw.text((index % 16 * 16 + 1, index // 16 * 32 + 1), chr(code), font=font, fill=255)
root = Path(__file__).resolve().parents[1]
(root / 'assets/media/tracking-glyphs.bin').write_bytes(image.tobytes())
