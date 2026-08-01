#!/usr/bin/env python3
"""Compose the ToskLight Viz application icon from the ToskLight icon and the "3D" badge.

The Viz products are the same desk seen in three dimensions, so their icon is the approved
ToskLight application icon with a glowing "3D" in the bottom-right corner rather than a separate
piece of artwork. Keeping the badge as its own small overlay means a future change to the
ToskLight icon reaches the Viz icon by re-running this script.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BRANDING = ROOT / "assets" / "branding"
BASE_ICON = BRANDING / "tosklight-app-icon.svg"
BADGE = BRANDING / "tosklight-viz-badge.svg"
COMPOSED_SVG = BRANDING / "tosklight-viz-icon.svg"
COMPOSED_PNG = BRANDING / "tosklight-viz-icon.png"

SVG_NS = "http://www.w3.org/2000/svg"
XLINK_NS = "http://www.w3.org/1999/xlink"
NS = f"{{{SVG_NS}}}"

ET.register_namespace("", SVG_NS)
ET.register_namespace("xlink", XLINK_NS)

TITLE = "ToskLight Viz application icon"
DESCRIPTION = (
    "The ToskLight spotlight and beam mark on its dark application tile, "
    'with a glowing "3D" in the bottom-right corner.'
)


def child(parent: ET.Element, name: str) -> ET.Element:
    element = parent.find(f"{NS}{name}")
    if element is None:
        raise ValueError(f"expected a <{name}> in {BASE_ICON.name}")
    return element


def compose() -> ET.ElementTree:
    base = ET.parse(BASE_ICON)
    root = base.getroot()
    badge_root = ET.parse(BADGE).getroot()

    child(root, "title").text = TITLE
    child(root, "desc").text = DESCRIPTION

    # The badge carries its own filters and glyph geometry. Merging them into the base <defs>
    # keeps one definitions block; every badge id is already namespaced, so nothing collides.
    definitions = child(root, "defs")
    definitions.extend(list(child(badge_root, "defs")))

    overlay = next(
        (
            element
            for element in badge_root
            if element.get("id") == "viz-badge"
        ),
        None,
    )
    if overlay is None:
        raise ValueError(f'expected a <g id="viz-badge"> in {BADGE.name}')
    root.append(overlay)
    return base


def rasterize(source: Path, destination: Path, size: int) -> None:
    renderer = shutil.which("rsvg-convert")
    if renderer is None:
        raise SystemExit(
            "rsvg-convert is required to export the flattened icon.\n"
            "Install it with: brew install librsvg"
        )
    subprocess.run(
        [renderer, "-w", str(size), "-h", str(size), str(source), "-o", str(destination)],
        check=True,
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--size",
        type=int,
        default=1024,
        help="edge length of the flattened PNG export (default: 1024)",
    )
    arguments = parser.parse_args(argv)

    compose().write(COMPOSED_SVG, encoding="utf-8", xml_declaration=True)
    rasterize(COMPOSED_SVG, COMPOSED_PNG, arguments.size)

    print(f"Wrote {COMPOSED_SVG.relative_to(ROOT)}")
    print(f"Wrote {COMPOSED_PNG.relative_to(ROOT)}")
    print(
        "Regenerate the platform icon sets with:\n"
        "  npm run --prefix apps/viz-editor tauri icon "
        "../../assets/branding/tosklight-viz-icon.png"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
