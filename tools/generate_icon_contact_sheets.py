#!/usr/bin/env python3
"""Generate deterministic PNG contact sheets for the ToskLight SVG icon groups."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
import shutil
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from artifact_paths import artifact_path
from generate_expanded_icons import (
    expanded_path,
    generate_expanded_icons,
    validate_expanded_svg,
)

ROOT = Path(__file__).resolve().parents[1]
ICON_ROOT = ROOT / "assets" / "icons"
OUTPUT_ROOT = artifact_path(
    "LIGHT_ICON_CONTACT_SHEETS_DIR",
    "ICON_CONTACT_SHEETS",
)
HELP_OUTPUT_ROOT = ROOT / "docs" / "help" / "assets" / "icon-contact-sheets"
MANIFEST = OUTPUT_ROOT / "manifest.json"
FORMAT_VERSION = 3

GROUP_NAMES = {
    "beam-size": "Beam size",
    "fixture-base": "Fixture base",
    "fixture-type": "Fixture type",
    "flash": "Flash",
    "functionality": "Functionality",
    "gobo": "Gobo",
    "laser-shape": "Laser shape",
    "misc": "Miscellaneous",
    "position": "Position arrows",
    "position-beam": "Position beams",
    "prism": "Prism",
}

BACKGROUND = (255, 255, 255, 255)
INK = (0, 0, 0, 255)
MUTED = (70, 70, 70, 255)
LABEL_WIDTH = 92
CONTENT_GAP = 20
CELL_WIDTH = 128
CELL_HEIGHT = 116
ICON_SIZE = 72
MAX_COLUMNS = 5
OUTER_MARGIN = 16


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def display_path(path: Path) -> str:
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return str(path)


def icon_groups() -> dict[str, list[Path]]:
    groups = {
        directory.name: sorted(
            (
                path
                for path in directory.glob("*.svg")
                if not path.name.endswith(".expanded.svg")
            ),
            key=lambda path: path.name,
        )
        for directory in sorted(ICON_ROOT.iterdir(), key=lambda path: path.name)
        if directory.is_dir()
        and any(
            not path.name.endswith(".expanded.svg")
            for path in directory.glob("*.svg")
        )
    }
    unexpected = sorted(set(groups) - set(GROUP_NAMES))
    missing = sorted(set(GROUP_NAMES) - set(groups))
    if unexpected or missing:
        raise ValueError(f"icon group mismatch; unexpected={unexpected}, missing={missing}")
    return groups


def validate_svg(path: Path) -> bytes:
    source = path.read_bytes()
    text = source.decode("utf-8")
    root = text.split(">", 1)[0]
    required = (
        'class="tosklight-icon"',
        'color="#000"',
        'stroke="currentColor"',
        'fill="none"',
        'viewBox="0 0 64 64"',
    )
    missing = [value for value in required if value not in root]
    if missing:
        raise ValueError(f"{path.relative_to(ROOT)} is missing {', '.join(missing)}")
    if "<title>" not in text:
        raise ValueError(f"{path.relative_to(ROOT)} is missing a title")
    document = ET.fromstring(source)

    def local_name(element: ET.Element) -> str:
        return element.tag.rsplit("}", 1)[-1]

    def walk(element: ET.Element, inside_mask: bool = False) -> None:
        masked = inside_mask or local_name(element) == "mask"
        if not masked:
            for attribute in ("fill", "stroke"):
                value = element.attrib.get(attribute, "").lower()
                if value in {"#fff", "#ffffff", "white", "#000", "#000000", "black"}:
                    raise ValueError(
                        f"{path.relative_to(ROOT)} has fixed visible {attribute}={value}"
                    )
            if (
                local_name(element) == "rect"
                and element.attrib.get("x", "0") == "0"
                and element.attrib.get("y", "0") == "0"
                and element.attrib.get("width") == "64"
                and element.attrib.get("height") == "64"
                and element.attrib.get("fill", "none") != "none"
            ):
                raise ValueError(
                    f"{path.relative_to(ROOT)} has a rendered full-canvas background"
                )
        for child in element:
            walk(child, masked)

    walk(document)
    return source


def source_manifest(groups: dict[str, list[Path]]) -> dict[str, dict[str, str]]:
    return {
        group: {path.name: sha256(validate_svg(path)) for path in paths}
        for group, paths in groups.items()
    }


def expanded_manifest(
    groups: dict[str, list[Path]],
    expanded: dict[Path, bytes],
) -> dict[str, dict[str, str]]:
    return {
        group: {
            expanded_path(path).name: sha256(expanded[expanded_path(path)])
            for path in paths
        }
        for group, paths in groups.items()
    }


def font(size: int):
    return ImageFont.load_default(size=size)


def centered_text(
    draw: ImageDraw.ImageDraw,
    box: tuple[int, int, int, int],
    text: str,
    text_font,
    fill=INK,
) -> None:
    bounds = draw.multiline_textbbox((0, 0), text, font=text_font, align="center", spacing=2)
    width = bounds[2] - bounds[0]
    height = bounds[3] - bounds[1]
    x = box[0] + (box[2] - box[0] - width) / 2 - bounds[0]
    y = box[1] + (box[3] - box[1] - height) / 2 - bounds[1]
    draw.multiline_text((x, y), text, font=text_font, fill=fill, align="center", spacing=2)


def filename_label(path: Path) -> str:
    words = path.stem.replace("-", " ").split()
    lines: list[str] = []
    current = ""
    for word in words:
        candidate = f"{current} {word}".strip()
        if current and len(candidate) > 18:
            lines.append(current)
            current = word
        else:
            current = candidate
    if current:
        lines.append(current)
    return "\n".join(lines[:2])


def render_svg(source: bytes) -> Image.Image:
    try:
        import cairosvg
    except ImportError as error:
        raise RuntimeError(
            "CairoSVG is required; run this command through ./build icon-contact-sheets"
        ) from error
    png = cairosvg.svg2png(
        bytestring=source,
        output_width=ICON_SIZE,
        output_height=ICON_SIZE,
    )
    return Image.open(io.BytesIO(png)).convert("RGBA")


def render_group(group: str, paths: list[Path]) -> Image.Image:
    columns = min(MAX_COLUMNS, len(paths))
    rows = math.ceil(len(paths) / columns)
    width = LABEL_WIDTH + CONTENT_GAP + columns * CELL_WIDTH + OUTER_MARGIN
    height = OUTER_MARGIN * 2 + rows * CELL_HEIGHT
    sheet = Image.new("RGBA", (width, height), BACKGROUND)
    draw = ImageDraw.Draw(sheet)
    draw.line(
        (LABEL_WIDTH, OUTER_MARGIN, LABEL_WIDTH, height - OUTER_MARGIN),
        fill=INK,
        width=2,
    )

    title_font = font(24)
    title_bounds = draw.textbbox((0, 0), GROUP_NAMES[group], font=title_font)
    title = Image.new(
        "RGBA",
        (title_bounds[2] - title_bounds[0] + 20, title_bounds[3] - title_bounds[1] + 20),
        (0, 0, 0, 0),
    )
    title_draw = ImageDraw.Draw(title)
    title_draw.text((10 - title_bounds[0], 10 - title_bounds[1]), GROUP_NAMES[group], font=title_font, fill=INK)
    title = title.rotate(90, expand=True)
    sheet.alpha_composite(
        title,
        (
            (LABEL_WIDTH - title.width) // 2,
            (height - title.height) // 2,
        ),
    )

    label_font = font(13)
    for index, path in enumerate(paths):
        row, column = divmod(index, columns)
        left = LABEL_WIDTH + CONTENT_GAP + column * CELL_WIDTH
        top = OUTER_MARGIN + row * CELL_HEIGHT
        source = expanded_path(path).read_bytes()
        validate_expanded_svg(expanded_path(path), source)
        icon = render_svg(source)
        sheet.alpha_composite(icon, (left + (CELL_WIDTH - ICON_SIZE) // 2, top + 5))
        centered_text(
            draw,
            (left + 2, top + ICON_SIZE + 9, left + CELL_WIDTH - 2, top + CELL_HEIGHT),
            filename_label(path),
            label_font,
            MUTED,
        )
    return sheet


def generated_manifest(
    sources: dict[str, dict[str, str]],
    expanded: dict[str, dict[str, str]],
    output_hashes: dict[str, str],
) -> dict[str, object]:
    return {
        "format": FORMAT_VERSION,
        "sources": sources,
        "expanded": expanded,
        "outputs": output_hashes,
    }


def generate() -> None:
    groups = icon_groups()
    sources = source_manifest(groups)
    expanded_files = generate_expanded_icons()
    expanded = expanded_manifest(groups, expanded_files)
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    expected = {f"{group}.png" for group in groups}
    HELP_OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    for directory in (OUTPUT_ROOT, HELP_OUTPUT_ROOT):
        for stale in directory.iterdir():
            if stale.is_file() and stale.name not in expected and stale != MANIFEST:
                stale.unlink()
    output_hashes: dict[str, str] = {}
    for group, paths in groups.items():
        destination = OUTPUT_ROOT / f"{group}.png"
        render_group(group, paths).convert("RGB").save(
            destination,
            format="PNG",
            optimize=True,
            compress_level=9,
        )
        output_hashes[destination.name] = sha256(destination.read_bytes())
        print(f"Rendered {display_path(destination)} from {len(paths)} icons")
    for name in sorted(expected):
        shutil.copy2(OUTPUT_ROOT / name, HELP_OUTPUT_ROOT / name)
    MANIFEST.write_text(
        json.dumps(
            generated_manifest(sources, expanded, output_hashes),
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def check() -> None:
    groups = icon_groups()
    sources = source_manifest(groups)
    expanded_files = generate_expanded_icons(check=True)
    expanded = expanded_manifest(groups, expanded_files)
    if not MANIFEST.is_file():
        raise ValueError(f"missing {MANIFEST.relative_to(ROOT)}")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    expected_names = {f"{group}.png" for group in groups}
    actual_names = {path.name for path in OUTPUT_ROOT.glob("*.png")}
    if actual_names != expected_names:
        raise ValueError(
            f"contact sheet mismatch; expected={sorted(expected_names)}, actual={sorted(actual_names)}"
        )
    output_hashes = {
        name: sha256((OUTPUT_ROOT / name).read_bytes())
        for name in sorted(expected_names)
    }
    expected = generated_manifest(sources, expanded, output_hashes)
    if manifest != expected:
        raise ValueError(
            "icon contact sheets are stale; run `npm run icons:contact-sheets`"
        )
    for name in sorted(expected_names):
        mirror = HELP_OUTPUT_ROOT / name
        if not mirror.is_file() or mirror.read_bytes() != (OUTPUT_ROOT / name).read_bytes():
            raise ValueError(
                f"Help contact-sheet mirror is stale: {mirror.relative_to(ROOT)}"
            )
        with Image.open(OUTPUT_ROOT / name) as image:
            if image.format != "PNG" or image.mode != "RGB":
                raise ValueError(f"{name} must be an RGB PNG")
    print(
        f"Verified {len(expected_names)} current icon contact sheets for "
        f"{sum(len(paths) for paths in groups.values())} icons"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        check() if args.check else generate()
    except Exception as error:
        print(f"icon contact sheet generation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
