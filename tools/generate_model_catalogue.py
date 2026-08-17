#!/usr/bin/env python3
"""Regenerate the Model Catalogue without invoking Blender."""

from __future__ import annotations

import argparse
from pathlib import Path

from model_catalogue import write_catalogue

ROOT = Path(__file__).resolve().parent.parent

parser = argparse.ArgumentParser()
parser.add_argument("--models", type=Path, default=ROOT / "assets" / "models")
parser.add_argument("--images", type=Path, default=ROOT / "docs" / "help" / "assets" / "models")
parser.add_argument(
    "--page", type=Path, default=ROOT / "docs" / "help" / "99-Appendix" / "01-model-catalogue.md"
)
arguments = parser.parse_args()
write_catalogue(arguments.models, arguments.images, arguments.page)
print(f"wrote {arguments.page.relative_to(ROOT)}")
