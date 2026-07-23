#!/usr/bin/env python3
"""Generate filled-geometry SVG derivatives from editable ToskLight icon sources."""

from __future__ import annotations

import argparse
import copy
import math
import re
import sys
from pathlib import Path

from lxml import etree
from picosvg.svg import SVG
from picosvg.svg_pathops import difference, union
from picosvg.svg_types import SVGPath

ROOT = Path(__file__).resolve().parents[1]
ICON_ROOT = ROOT / "assets" / "icons"
SVG_NS = "http://www.w3.org/2000/svg"
URL_REFERENCE = re.compile(r"^url\(#([^)]+)\)$")
PAINTED_TAGS = {"path", "text"}


def local_name(element: etree._Element) -> str:
    return etree.QName(element).localname


def svg_tag(name: str) -> str:
    return f"{{{SVG_NS}}}{name}"


def is_expanded(path: Path) -> bool:
    return path.name.endswith(".expanded.svg")


def source_icon_paths() -> list[Path]:
    return sorted(
        (path for path in ICON_ROOT.glob("*/*.svg") if not is_expanded(path)),
        key=lambda path: path.relative_to(ICON_ROOT).as_posix(),
    )


def expanded_path(source: Path) -> Path:
    return source.with_name(f"{source.stem}.expanded.svg")


def _number(element: etree._Element, attribute: str, default: float = 0) -> float:
    raw = element.get(attribute)
    if raw is None:
        return default
    try:
        return float(raw)
    except ValueError as error:
        raise ValueError(
            f"<{local_name(element)}> has unsupported {attribute}={raw!r}"
        ) from error


def _reference_id(value: str | None, attribute: str) -> str:
    match = URL_REFERENCE.fullmatch(value or "")
    if not match:
        raise ValueError(f"unsupported {attribute} reference {value!r}")
    return match.group(1)


def _definition(root: etree._Element, identifier: str, tag: str) -> etree._Element:
    matches = root.xpath(
        f".//svg:{tag}[@id=$identifier]",
        namespaces={"svg": SVG_NS},
        identifier=identifier,
    )
    if len(matches) != 1:
        raise ValueError(f"expected one <{tag} id={identifier!r}>, found {len(matches)}")
    return matches[0]


def _ensure_defs(root: etree._Element) -> etree._Element:
    defs = root.find(svg_tag("defs"))
    if defs is None:
        defs = etree.Element(svg_tag("defs"))
        root.insert(0, defs)
    return defs


def _expand_patterns(root: etree._Element) -> None:
    """Materialize the simple user-space patterns used by the icon catalog."""

    patterned = [
        element
        for element in root.iter()
        if URL_REFERENCE.fullmatch(element.get("fill", ""))
    ]
    defs = _ensure_defs(root)
    for index, element in enumerate(patterned):
        if local_name(element) != "rect":
            raise ValueError("pattern fills are supported only on rectangles")
        pattern_id = _reference_id(element.get("fill"), "pattern")
        pattern = _definition(root, pattern_id, "pattern")
        if pattern.get("patternUnits") != "userSpaceOnUse":
            raise ValueError(f"pattern {pattern_id!r} must use userSpaceOnUse")
        if pattern.get("patternTransform"):
            raise ValueError(f"pattern {pattern_id!r} cannot use patternTransform")

        x = _number(element, "x")
        y = _number(element, "y")
        width = _number(element, "width")
        height = _number(element, "height")
        pattern_x = _number(pattern, "x")
        pattern_y = _number(pattern, "y")
        pattern_width = _number(pattern, "width")
        pattern_height = _number(pattern, "height")
        if min(width, height, pattern_width, pattern_height) <= 0:
            raise ValueError(f"pattern {pattern_id!r} has non-positive geometry")

        clip_id = f"expanded-pattern-clip-{index}"
        clip = etree.SubElement(defs, svg_tag("clipPath"), id=clip_id)
        clip.append(
            etree.Element(
                svg_tag("rect"),
                x=f"{x:g}",
                y=f"{y:g}",
                width=f"{width:g}",
                height=f"{height:g}",
            )
        )
        group = etree.Element(svg_tag("g"))
        group.set("clip-path", f"url(#{clip_id})")
        start_x = pattern_x + math.floor((x - pattern_x) / pattern_width) * pattern_width
        start_y = pattern_y + math.floor((y - pattern_y) / pattern_height) * pattern_height
        tile_x = start_x
        while tile_x < x + width:
            tile_y = start_y
            while tile_y < y + height:
                for pattern_child in pattern:
                    clone = copy.deepcopy(pattern_child)
                    existing = clone.get("transform")
                    translate = f"translate({tile_x:g} {tile_y:g})"
                    clone.set("transform", f"{translate} {existing}" if existing else translate)
                    group.append(clone)
                tile_y += pattern_height
            tile_x += pattern_width

        parent = element.getparent()
        parent.replace(element, group)


def _mini_svg(root: etree._Element, children: list[etree._Element]) -> etree._Element:
    mini = etree.Element(svg_tag("svg"), nsmap={None: SVG_NS})
    for name, value in root.attrib.items():
        if name not in {"class"}:
            mini.set(name, value)
    for child in children:
        mini.append(copy.deepcopy(child))
    return mini


def _expanded_cutout(root: etree._Element, mask: etree._Element) -> SVGPath:
    blockers: list[etree._Element] = []
    for child in mask:
        if not isinstance(child.tag, str):
            continue
        fill = child.get("fill", "none").lower()
        stroke = child.get("stroke", "none").lower()
        opacity = child.get("opacity", "1")
        if opacity != "1":
            raise ValueError(f"mask {mask.get('id')!r} uses opacity")
        if fill in {"#fff", "#ffffff", "white"}:
            if (
                local_name(child) != "rect"
                or _number(child, "x") != 0
                or _number(child, "y") != 0
                or _number(child, "width") != 64
                or _number(child, "height") != 64
                or stroke != "none"
            ):
                raise ValueError(f"mask {mask.get('id')!r} has unsupported white geometry")
            continue
        if fill not in {"none", "#000", "#000000", "black"}:
            raise ValueError(f"mask {mask.get('id')!r} has unsupported fill {fill!r}")
        if stroke not in {"none", "#000", "#000000", "black"}:
            raise ValueError(f"mask {mask.get('id')!r} has unsupported stroke {stroke!r}")
        if fill == "none" and stroke == "none":
            continue
        blockers.append(child)

    if not blockers:
        raise ValueError(f"mask {mask.get('id')!r} has no black cutout geometry")
    converted = SVG.fromstring(
        etree.tostring(_mini_svg(root, blockers), encoding="unicode")
    ).topicosvg(ndigits=6, allow_text=False)
    paths = [
        SVGPath(d=element.get("d", ""))
        for element in converted.svg_root.iter()
        if local_name(element) == "path"
    ]
    if not paths:
        raise ValueError(f"mask {mask.get('id')!r} produced no cutout paths")
    cutout = SVGPath.from_commands(
        union(
            [path.as_cmd_seq() for path in paths],
            [path.fill_rule for path in paths],
        )
    )
    canvas = SVGPath(d="M0 0H64V64H0Z")
    return SVGPath.from_commands(
        difference(
            [canvas.as_cmd_seq(), cutout.as_cmd_seq()],
            ["nonzero", "nonzero"],
        )
    )


def _resolve_binary_masks(root: etree._Element) -> None:
    masks = list(root.iter(svg_tag("mask")))
    for mask in masks:
        mask_id = mask.get("id")
        if not mask_id:
            raise ValueError("mask is missing an id")
        if mask.get("maskUnits", "objectBoundingBox") != "userSpaceOnUse":
            raise ValueError(f"mask {mask_id!r} must use userSpaceOnUse")
        retained = _expanded_cutout(root, mask)
        clip = etree.Element(svg_tag("clipPath"), id=mask_id)
        clip.append(etree.Element(svg_tag("path"), d=retained.d))
        mask.getparent().replace(mask, clip)

        for element in root.iter():
            if element.get("mask") == f"url(#{mask_id})":
                element.attrib.pop("mask")
                if element.get("clip-path"):
                    raise ValueError(f"element combines mask {mask_id!r} with clip-path")
                element.set("clip-path", f"url(#{mask_id})")


def _join_painted_paths(root: etree._Element) -> None:
    paths = list(root.iter(svg_tag("path")))
    if not paths:
        return
    if any(path.getparent() is not root for path in paths):
        raise ValueError("PicoSVG left nested paths before the Boolean-union pass")
    shapes: list[SVGPath] = []
    for path in paths:
        if path.get("opacity", "1") != "1" or path.get("fill-opacity", "1") != "1":
            raise ValueError("cannot join paths with partial opacity")
        shapes.append(
            SVGPath(
                d=path.get("d", ""),
                fill_rule=path.get("fill-rule", "nonzero"),
            )
        )
    joined = SVGPath.from_commands(
        union(
            [shape.as_cmd_seq() for shape in shapes],
            [shape.fill_rule for shape in shapes],
        )
    ).round_floats(3)
    insertion_index = min(root.index(path) for path in paths)
    for path in paths:
        root.remove(path)
    root.insert(
        insertion_index,
        etree.Element(
            svg_tag("path"),
            fill="currentColor",
            d=joined.d,
        ),
    )


def _canonicalize(root: etree._Element, title: str) -> bytes:
    converted = SVG.fromstring(etree.tostring(root, encoding="unicode")).topicosvg(
        ndigits=3,
        allow_text=True,
    )
    output = converted.svg_root
    output.set("class", "tosklight-icon")
    output.set("color", "#000")
    for attribute in list(output.attrib):
        if attribute.startswith("stroke") or attribute == "fill":
            output.attrib.pop(attribute)

    for defs in list(output.iter(svg_tag("defs"))):
        if len(defs) == 0:
            defs.getparent().remove(defs)

    _join_painted_paths(output)

    title_element = etree.Element(svg_tag("title"))
    title_element.text = title
    output.insert(0, title_element)
    output.insert(
        1,
        etree.Comment(
            " Generated by tools/generate_expanded_icons.py; do not edit directly. "
        ),
    )

    for element in output.iter():
        if not isinstance(element.tag, str):
            continue
        name = local_name(element)
        if name in PAINTED_TAGS:
            element.set("fill", "currentColor")
        for attribute in list(element.attrib):
            if attribute == "stroke" or attribute.startswith("stroke-"):
                element.attrib.pop(attribute)
        if element.get("style") == "":
            element.attrib.pop("style")

    result = etree.tostring(
        output,
        encoding="utf-8",
        pretty_print=True,
        xml_declaration=False,
    )
    return result.rstrip() + b"\n"


def expand_svg(source: Path) -> bytes:
    parser = etree.XMLParser(remove_blank_text=True)
    root = etree.fromstring(source.read_bytes(), parser)
    if local_name(root) != "svg" or root.get("viewBox") != "0 0 64 64":
        raise ValueError(f"{source.relative_to(ROOT)} must use a 64 by 64 SVG canvas")
    title_element = root.find(svg_tag("title"))
    if title_element is None or not (title_element.text or "").strip():
        raise ValueError(f"{source.relative_to(ROOT)} is missing a title")
    title = (title_element.text or "").strip()
    _expand_patterns(root)
    _resolve_binary_masks(root)
    return _canonicalize(root, title)


def validate_expanded_svg(path: Path, data: bytes) -> None:
    root = etree.fromstring(data)
    if (
        root.get("class") != "tosklight-icon"
        or root.get("color") != "#000"
        or root.get("viewBox") != "0 0 64 64"
    ):
        raise ValueError(f"{path.relative_to(ROOT)} has an invalid expanded SVG root")
    forbidden_tags = {"mask", "clipPath", "pattern", "use", "image", "filter"}
    painted_path_count = 0
    for element in root.iter():
        if not isinstance(element.tag, str):
            continue
        name = local_name(element)
        if name == "path":
            painted_path_count += 1
        if name in forbidden_tags:
            raise ValueError(f"{path.relative_to(ROOT)} retains <{name}>")
        if element.get("transform"):
            raise ValueError(f"{path.relative_to(ROOT)} retains a transform")
        stroke = element.get("stroke", "none")
        if stroke != "none":
            raise ValueError(f"{path.relative_to(ROOT)} retains stroke={stroke!r}")
        if name in PAINTED_TAGS and element.get("fill") != "currentColor":
            raise ValueError(f"{path.relative_to(ROOT)} has non-currentColor artwork")
    if painted_path_count > 1:
        raise ValueError(
            f"{path.relative_to(ROOT)} retains {painted_path_count} separate paths"
        )


def expected_expanded() -> dict[Path, bytes]:
    expected: dict[Path, bytes] = {}
    for source in source_icon_paths():
        destination = expanded_path(source)
        data = expand_svg(source)
        validate_expanded_svg(destination, data)
        expected[destination] = data
    return expected


def generate_expanded_icons(*, check: bool = False) -> dict[Path, bytes]:
    expected = expected_expanded()
    actual = set(ICON_ROOT.glob("*/*.expanded.svg"))
    stale = sorted(actual - set(expected))
    if check:
        problems = [path for path, data in expected.items() if not path.is_file() or path.read_bytes() != data]
        if stale or problems:
            details = [path.relative_to(ROOT).as_posix() for path in stale + problems]
            raise ValueError(
                "expanded icons are stale; run `npm run icons:contact-sheets`: "
                + ", ".join(details)
            )
    else:
        for path in stale:
            path.unlink()
        for path, data in expected.items():
            if not path.is_file() or path.read_bytes() != data:
                path.write_bytes(data)
    return expected


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        generated = generate_expanded_icons(check=args.check)
    except Exception as error:
        print(f"expanded icon generation failed: {error}", file=sys.stderr)
        return 1
    verb = "Verified" if args.check else "Generated"
    print(f"{verb} {len(generated)} expanded SVG icons")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
