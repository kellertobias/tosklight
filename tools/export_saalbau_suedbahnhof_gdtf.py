#!/usr/bin/env python3
"""Create compatibility GDTFs for the SAALBAU Suedbahnhof patch.

The exported archives are derived from the reviewable ToskLight fixture packages.
They preserve the exact DMX mode names, channel footprints, byte widths, defaults,
and function starts used by the venue.  They are intentionally marked as ToskLight
exports, not manufacturer-supplied originals.
"""

from __future__ import annotations

import copy
import json
import re
import uuid
import xml.etree.ElementTree as etree
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape


ROOT = Path(__file__).resolve().parent.parent
LIBRARY = ROOT / "assets" / "fixture-library"
OUTPUT = ROOT.parent / "SAALBAU Suedbahnhof GDTF"

FIXTURES = {
    "cameo--auro-spot-z300": "Cameo-AURO-SPOT-Z300.gdtf",
    "martin--mac-250-entour": "Martin-MAC-250-Entour.gdtf",
    "claypaky--stage-zoom-1200-sv": "Clay-Paky-Stage-Zoom-1200-SV.gdtf",
    "robe--robin-300-ledwash": "ROBE-Robin-300-LEDWash.gdtf",
    "jb-lighting--jbled-a7": "JB-Lighting-JBLED-A7.gdtf",
    "martin--mac-300": "Martin-MAC-300.gdtf",
    "cameo--root-par-6": "Cameo-ROOT-PAR-6.gdtf",
    "generic--dimmer-rgb-control-par": "LED-PAR-56-Suedbahnhof.gdtf",
    "eurolite--ts-255-dmx-scan": "Eurolite-TS-255-DMX-Scan.gdtf",
    "prolights--ecl-fresnel-ct-plus-m": "Prolights-ECL-Fresnel-CT-plus-M.gdtf",
    "martin--elp-cl-profile": "Martin-ELP-CL-Profile.gdtf",
    "martin--elp-ww-profile": "Martin-ELP-WW-Profile.gdtf",
}

ATTRIBUTE_NAMES = {
    "intensity": "Dimmer",
    "shutter": "Shutter1",
    "pan": "Pan",
    "tilt": "Tilt",
    "iris": "Iris",
    "focus": "Focus",
    "zoom": "Zoom",
    "frost.1": "Frost1",
    "color.red": "ColorAdd_R",
    "color.green": "ColorAdd_G",
    "color.blue": "ColorAdd_B",
    "color.white": "ColorAdd_W",
    "color.warm_white": "ColorAdd_WW",
    "color.cold_white": "ColorAdd_CW",
    "color.cyan": "ColorSub_C",
    "color.magenta": "ColorSub_M",
    "color.yellow": "ColorSub_Y",
}


def package_profile(stem: str) -> dict:
    with zipfile.ZipFile(LIBRARY / f"{stem}.toskfixture") as archive:
        return json.loads(archive.read("fixture.json"))["profile"]


def remap_ids(value: object, ids: dict[str, str] | None = None) -> object:
    """Copy a mode while retaining references between its heads and channels."""
    if ids is None:
        ids = {}

        def collect(item: object) -> None:
            if isinstance(item, dict):
                if isinstance(item.get("id"), str):
                    ids[item["id"]] = str(uuid.uuid4())
                for child in item.values():
                    collect(child)
            elif isinstance(item, list):
                for child in item:
                    collect(child)

        collect(value)
    if isinstance(value, dict):
        return {key: remap_ids(item, ids) for key, item in value.items()}
    if isinstance(value, list):
        return [remap_ids(item, ids) for item in value]
    return ids.get(value, value) if isinstance(value, str) else value


def has_valid_head_references(mode: dict) -> bool:
    head_ids = {head["id"] for head in mode["heads"]}
    return all(channel["head_id"] in head_ids for channel in mode["channels"])


def update_stage_zoom_sv() -> None:
    """Give the SV package the 20-slot venue personality its manual shares."""
    sv_path = LIBRARY / "claypaky--stage-zoom-1200-sv.toskfixture"
    source = package_profile("claypaky--stage-zoom-1200")
    target = package_profile("claypaky--stage-zoom-1200-sv")
    required = "16 bit - gobo fine - lamp control"
    existing = next((index for index, mode in enumerate(target["modes"]) if mode["name"] == required), None)
    source_mode = next(mode for mode in source["modes"] if mode["name"] == required)
    changed = False
    if existing is None:
        target["modes"].append(remap_ids(copy.deepcopy(source_mode)))
        target["revision"] += 1
        target["notes"] += " The SV's shared 20-slot venue personality is included in revision 2."
        changed = True
    elif not has_valid_head_references(target["modes"][existing]):
        target["modes"][existing] = remap_ids(copy.deepcopy(source_mode))
        changed = True
    if changed:
        manifest = {
            "$schema": "https://tosklight.app/schemas/fixture-package-v1.json",
            "format": "tosklight.fixture",
            "format_version": 1,
            "profile": target,
        }
        with zipfile.ZipFile(sv_path, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("fixture.json", json.dumps(manifest, indent=2) + "\n")


def gdtf_name(value: str) -> str:
    value = value.replace("—", "-").replace("–", "-").replace("×", "x")
    return re.sub(r"[^A-Za-z0-9#%()*+\-/:;<=>@_` \"']", "_", value)


def attribute_name(value: str) -> str:
    if value in ATTRIBUTE_NAMES:
        return ATTRIBUTE_NAMES[value]
    gobo = re.fullmatch(r"gobo\.(\d+)", value)
    if gobo:
        return f"Gobo{gobo.group(1)}"
    return gdtf_name(value.replace(".", "_"))


def channel_offsets(channel: dict, primary: int, reserved: set[int]) -> tuple[str, int]:
    resolution = channel["resolution"]
    bytes_by_resolution = {"u8": 1, "u16": 2, "u24": 3, "u32": 4}
    width = bytes_by_resolution[resolution]
    while primary in reserved:
        primary += 1
    secondaries = channel.get("secondary_slots", [])
    offsets = [primary] + secondaries
    if len(offsets) != width:
        offsets = list(range(primary, primary + width))
    return ",".join(map(str, offsets)), primary + 1


def mode_xml(mode: dict) -> str:
    channels = []
    primary = 1
    reserved = {
        slot
        for channel in mode["channels"]
        for slot in channel.get("secondary_slots", [])
    }
    for channel in mode["channels"]:
        offsets, primary = channel_offsets(channel, primary, reserved)
        width = len(offsets.split(","))
        attr = attribute_name(channel["attribute"])
        physical_min = float(channel["physical_min"] or 0)
        physical_max = float(channel["physical_max"] or 1)
        if physical_min == physical_max:
            physical_max = physical_min + 1
        functions = "".join(
            f'              <ChannelSet Name="{escape(gdtf_name(function["name"]))}" DMXFrom="{function["dmx_from"]}/{width}"/>\n'
            for function in channel["functions"]
        )
        channels.append(
            f'          <DMXChannel DMXBreak="1" Offset="{offsets}" Highlight="None" Geometry="Body">\n'
            f'            <LogicalChannel Attribute="{attr}" Snap="No" Master="None" MibFade="0.000000" DMXChangeTimeLimit="0.000000">\n'
            f'              <ChannelFunction Name="{attr}" Attribute="{attr}" OriginalAttribute="" DMXFrom="0/1" Default="{channel["default_raw"]}/{width}" PhysicalFrom="{physical_min:.6f}" PhysicalTo="{physical_max:.6f}" RealFade="0.000000">\n'
            f'{functions}'
            '              </ChannelFunction>\n            </LogicalChannel>\n          </DMXChannel>\n'
        )
    return (
        f'      <DMXMode Name="{escape(gdtf_name(mode["name"]))}" Geometry="Body">\n'
        "        <DMXChannels>\n"
        + "".join(channels)
        + "        </DMXChannels>\n        <Relations/>\n        <FTMacros/>\n      </DMXMode>\n"
    )


def export_gdtf(profile: dict, output: Path) -> None:
    attributes = []
    for mode in profile["modes"]:
        for channel in mode["channels"]:
            name = attribute_name(channel["attribute"])
            if name not in attributes:
                attributes.append(name)
    attribute_xml = "".join(
        f'        <Attribute Name="{attribute}" Pretty="{attribute}" Feature="Control.Control"/>\n'
        for attribute in attributes
    )
    description = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<GDTF DataVersion="1.2">\n'
        f'  <FixtureType Name="{escape(gdtf_name(profile["name"]))}" ShortName="{escape(gdtf_name(profile["short_name"]))}" LongName="{escape(gdtf_name(profile["name"]))}" Manufacturer="{escape(gdtf_name(profile["manufacturer"]))}" Description="ToskLight compatibility export for SAALBAU Suedbahnhof" FixtureTypeID="{profile["id"]}" RefFT="">\n'
        "    <AttributeDefinitions>\n      <ActivationGroups/>\n      <FeatureGroups>\n        <FeatureGroup Name=\"Control\" Pretty=\"Control\"><Feature Name=\"Control\"/></FeatureGroup>\n      </FeatureGroups>\n      <Attributes>\n"
        + attribute_xml
        + "      </Attributes>\n    </AttributeDefinitions>\n    <Wheels/>\n    <PhysicalDescriptions/>\n    <Models/>\n    <Geometries><Geometry Name=\"Body\" Position=\"None\"/></Geometries>\n    <DMXModes>\n"
        + "".join(mode_xml(mode) for mode in profile["modes"])
        + "    </DMXModes>\n    <Revisions/>\n    <FTPresets/>\n    <Protocols/>\n  </FixtureType>\n</GDTF>\n"
    )
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("description.xml", description)


def verify() -> None:
    for stem, name in FIXTURES.items():
        profile = package_profile(stem)
        with zipfile.ZipFile(OUTPUT / name) as archive:
            root = etree.fromstring(archive.read("description.xml"))
        modes = root.findall(".//DMXModes/DMXMode")
        assert [mode.get("Name") for mode in modes] == [gdtf_name(mode["name"]) for mode in profile["modes"]]
        for expected, actual in zip(profile["modes"], modes, strict=True):
            footprint = max(
                int(offset)
                for channel in actual.findall("./DMXChannels/DMXChannel")
                for offset in channel.get("Offset", "").split(",")
            )
            assert footprint == expected["splits"][0]["footprint"], (stem, expected["name"], footprint)


def main() -> None:
    update_stage_zoom_sv()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for stem, name in FIXTURES.items():
        export_gdtf(package_profile(stem), OUTPUT / name)
    verify()


if __name__ == "__main__":
    main()
