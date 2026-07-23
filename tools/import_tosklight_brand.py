#!/usr/bin/env python3
"""Import the approved ToskLight artwork and derive its reusable variants."""

from __future__ import annotations

import argparse
import copy
from pathlib import Path
import xml.etree.ElementTree as ET


SVG_NS = "http://www.w3.org/2000/svg"
XLINK_NS = "http://www.w3.org/1999/xlink"
NS = f"{{{SVG_NS}}}"

ET.register_namespace("", SVG_NS)
ET.register_namespace("xlink", XLINK_NS)


def local_name(element: ET.Element) -> str:
    return element.tag.rsplit("}", 1)[-1]


def find_artwork(root: ET.Element) -> tuple[ET.Element, ET.Element]:
    artboard = next(
        (
            element
            for element in root
            if local_name(element) == "g" and element.attrib.get("id") == "ArtBoard11"
        ),
        None,
    )
    if artboard is None or len(artboard) != 2:
        raise ValueError(
            "Expected ArtBoard11 to contain exactly the tile background and logo artwork"
        )
    return artboard[0], artboard[1]


def remove_use_references(element: ET.Element, references: set[str]) -> None:
    for child in list(element):
        remove_use_references(child, references)
        href = next(
            (
                value
                for key, value in child.attrib.items()
                if key.rsplit("}", 1)[-1] == "href"
            ),
            "",
        )
        if href in references or (local_name(child) == "g" and len(child) == 0):
            element.remove(child)


def referenced_ids(element: ET.Element) -> set[str]:
    references: set[str] = set()
    for node in element.iter():
        for value in node.attrib.values():
            if value.startswith("#"):
                references.add(value[1:])
            start = 0
            while (marker := value.find("url(#", start)) >= 0:
                end = value.find(")", marker)
                if end < 0:
                    break
                references.add(value[marker + 5 : end])
                start = end + 1
    return references


def filtered_defs(source_root: ET.Element, artwork: ET.Element) -> ET.Element | None:
    source_defs = source_root.find(f"{NS}defs")
    if source_defs is None:
        return None

    wanted = referenced_ids(artwork)
    selected: list[ET.Element] = []
    remaining = list(source_defs)
    while wanted:
        matches = [node for node in remaining if node.attrib.get("id") in wanted]
        if not matches:
            break
        for match in matches:
            remaining.remove(match)
            wanted.discard(match.attrib.get("id", ""))
            selected.append(copy.deepcopy(match))
            wanted.update(referenced_ids(match))

    if not selected:
        return None
    defs = ET.Element(f"{NS}defs")
    defs.extend(selected)
    return defs


def make_svg(
    source_root: ET.Element,
    artwork: list[ET.Element],
    title: str,
    description: str,
) -> ET.Element:
    root = ET.Element(
        f"{NS}svg",
        {
            "width": "1024",
            "height": "1024",
            "viewBox": "0 0 1024 1024",
            "role": "img",
            "aria-labelledby": "title description",
            "style": source_root.attrib.get(
                "style",
                "fill-rule:evenodd;clip-rule:evenodd;stroke-linejoin:round;stroke-miterlimit:2",
            ),
        },
    )
    ET.SubElement(root, f"{NS}title", {"id": "title"}).text = title
    ET.SubElement(root, f"{NS}desc", {"id": "description"}).text = description
    group = ET.SubElement(root, f"{NS}g")
    group.extend(copy.deepcopy(artwork))
    defs = filtered_defs(source_root, group)
    if defs is not None:
        root.insert(2, defs)
    return root


def write_svg(path: Path, root: ET.Element) -> None:
    ET.indent(root, space="  ")
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


def import_brand(source: Path, output: Path) -> None:
    source_root = ET.parse(source).getroot()
    background, logo = find_artwork(source_root)

    clean_logo = copy.deepcopy(logo)
    remove_use_references(clean_logo, {"#_Image1", "#_Image7"})

    variants = {
        "tosklight-app-icon.svg": make_svg(
            source_root,
            [background, logo],
            "ToskLight application icon",
            "The ToskLight spotlight and beam mark on its dark application tile.",
        ),
        "tosklight-icon-print.svg": make_svg(
            source_root,
            [background, clean_logo],
            "ToskLight print icon",
            "The ToskLight application icon without shadow effects.",
        ),
        "tosklight-mark-shadow.svg": make_svg(
            source_root,
            [logo],
            "ToskLight mark with presentation shadows",
            "The standalone ToskLight spotlight and beam mark with its original shadow effects.",
        ),
        "tosklight-mark.svg": make_svg(
            source_root,
            [clean_logo],
            "ToskLight mark",
            "The standalone ToskLight spotlight and beam mark without shadow effects.",
        ),
    }

    output.mkdir(parents=True, exist_ok=True)
    for name, root in variants.items():
        write_svg(output / name, root)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="Approved 1024x1024 SVG artwork")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("assets/branding"),
        help="Directory for the four derived SVG assets",
    )
    args = parser.parse_args()
    import_brand(args.source, args.output)


if __name__ == "__main__":
    main()
