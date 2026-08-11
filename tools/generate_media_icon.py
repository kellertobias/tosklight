#!/usr/bin/env python3
"""Compose the ToskLight Media icon from the base icon and LED-wall badge."""

from __future__ import annotations

import shutil
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BRANDING = ROOT / "assets" / "branding"
BASE = BRANDING / "tosklight-app-icon.svg"
BADGE = BRANDING / "tosklight-media-badge.svg"
SVG = BRANDING / "tosklight-media-icon.svg"
PNG = BRANDING / "tosklight-media-icon.png"
NS = "{http://www.w3.org/2000/svg}"
ET.register_namespace("", "http://www.w3.org/2000/svg")


def required(parent: ET.Element, name: str) -> ET.Element:
    result = parent.find(f"{NS}{name}")
    if result is None:
        raise ValueError(f"expected <{name}>")
    return result


def main() -> int:
    base = ET.parse(BASE)
    root = base.getroot()
    badge = ET.parse(BADGE).getroot()
    required(root, "title").text = "ToskLight Media application icon"
    required(root, "desc").text = "The ToskLight mark with a glowing LED wall in the bottom-right corner."
    required(root, "defs").extend(list(required(badge, "defs")))
    overlay = next((element for element in badge if element.get("id") == "media-badge"), None)
    if overlay is None:
        raise ValueError('expected <g id="media-badge">')
    root.append(overlay)
    base.write(SVG, encoding="utf-8", xml_declaration=True)

    renderer = shutil.which("rsvg-convert")
    if renderer is None:
        raise SystemExit("rsvg-convert is required (brew install librsvg)")
    subprocess.run([renderer, "-w", "1024", "-h", "1024", str(SVG), "-o", str(PNG)], check=True)
    print(f"Wrote {SVG.relative_to(ROOT)}")
    print(f"Wrote {PNG.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
