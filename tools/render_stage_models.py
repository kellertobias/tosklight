#!/usr/bin/env python3
"""Render one PNG per shipped model, and the help page that shows them.

Run through Blender::

    blender --background --factory-startup --python tools/render_stage_models.py -- \
        --models assets/models --images docs/help/assets/models \
        --page docs/help/45-Visualizer/02-model-catalogue.md

Or through the npm script, which does the same thing::

    npm run models:render

The renders come from importing the shipped ``.glb`` rather than from re-running the
builders, so what the help shows is what a desk would load. Anything that fails to
import fails the run, which makes this a round-trip check as well as a picture.

Each model is looked at from where it is really seen: under a lamp that hangs, over a
wedge or a rack that stands on the floor. See :func:`eye_height`.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MODELS = ROOT / "assets" / "models"
DEFAULT_IMAGES = ROOT / "docs" / "help" / "assets" / "models"
DEFAULT_PAGE = ROOT / "docs" / "help" / "45-Visualizer" / "02-model-catalogue.md"

GROUP_TITLES = {
    "lamps": "Lamps",
    "truss": "Truss and rigging",
    "stage": "Stage and venue",
    "av": "Projection and laser",
    "cases": "Flight cases",
    "backline": "Backline and PA",
    "people": "Figures",
}
GROUP_ORDER = ("lamps", "av", "truss", "stage", "cases", "backline", "people")


def parse_args() -> argparse.Namespace:
    arguments = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser(prog="render_stage_models")
    parser.add_argument("--models", type=Path, default=DEFAULT_MODELS)
    parser.add_argument("--images", type=Path, default=DEFAULT_IMAGES)
    parser.add_argument("--page", type=Path, default=DEFAULT_PAGE)
    parser.add_argument("--width", type=int, default=560)
    parser.add_argument("--height", type=int, default=420)
    parser.add_argument("--only", default="")
    return parser.parse_args(arguments)


def eye_height(entry: dict) -> float:
    """How far above or below its own centre a model is looked at, as a fraction of reach.

    Where a thing is rigged decides where it is looked at from, and the manifest already
    states that per model:

    * **Hung** — well below. A lamp points down, and its business end is the lens, the
      reflector cells, the open nose of a can. Level with it, or over it, is a row of
      identical black lids.
    * **On the floor and low** — steeply above, because everything worth seeing is on the
      top: the jog wheel of a media player, the faders of a mixer, the sloping driver
      face of a wedge, the spigots in a ground-support sleeve. Shallower than the slope
      of a monitor's own face and the model comes out as a black slab.
    * **On the floor and tall** — a little above only. A figure, an amp stack or a lift
      is seen standing next to it, and looking down on one foreshortens it to nothing.
    * **Anything else** — a truss section, a drape, a mirror ball: a little above, which
      is the eye-line the object is really seen from.
    """

    origin = str(entry["origin"])
    if origin.startswith("mounting point"):
        return -0.62
    if not origin.startswith("floor level"):
        return 0.28
    footprint = max(float(entry["width_millimetres"]), float(entry["depth_millimetres"]))
    return 1.0 if float(entry["height_millimetres"]) < footprint * 0.8 else 0.4


def framed_camera(low: Vector, high: Vector, elevation: float) -> None:
    """Point a camera at the model's own bounding box, from three-quarters round."""

    centre = (low + high) / 2
    size = max((high - low)[axis] for axis in range(3))
    target = bpy.data.objects.new("aim", None)
    bpy.context.collection.objects.link(target)
    target.location = centre

    data = bpy.data.cameras.new("camera")
    camera = bpy.data.objects.new("camera", data)
    bpy.context.collection.objects.link(camera)
    camera.location = centre + Vector((0.85, -1.35, elevation)).normalized() * size * 2.35
    track = camera.constraints.new("TRACK_TO")
    track.target = target
    track.track_axis = "TRACK_NEGATIVE_Z"
    track.up_axis = "UP_Y"
    bpy.context.scene.camera = camera


def render(path: Path, image: Path, width: int, height: int, elevation: float) -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(path))
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    if not meshes:
        raise SystemExit(f"render error: {path} imported with no geometry")
    low = Vector((1e9, 1e9, 1e9))
    high = Vector((-1e9, -1e9, -1e9))
    for obj in meshes:
        for corner in obj.bound_box:
            point = obj.matrix_world @ Vector(corner)
            low = Vector(min(low[axis], point[axis]) for axis in range(3))
            high = Vector(max(high[axis], point[axis]) for axis in range(3))
    framed_camera(low, high, elevation)

    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.color_type = "MATERIAL"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.show_shadows = False
    scene.display.shading.show_cavity = True
    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.compression = 90
    image.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(image)
    bpy.ops.render.render(write_still=True)


def catalogue(entries: list[dict], images: Path, page: Path) -> None:
    """Write the help page that lists every model with its picture and its name."""

    relative = Path("..") / images.relative_to(page.parent.parent)
    lines = [
        "# Model Catalogue",
        "",
        "Every model ToskLight ships with the Visualizer, with the name the fixture",
        "library and the show file refer to it by. A fixture profile points at one of",
        "these with `model_asset`; a fixture that points at nothing is given one",
        "automatically, as described in [Default Models](01-default-models.md).",
        "",
        "This page is generated by `npm run models:render` from `assets/models`. Edit the",
        "builders under `tools/stage_models/`, not this file.",
        "",
    ]
    for group in GROUP_ORDER:
        members = [entry for entry in entries if entry["group"] == group]
        if not members:
            continue
        lines.append(f"## {GROUP_TITLES.get(group, group.title())}")
        lines.append("")
        lines.append("| Model | Name | Size (W x H x D) |")
        lines.append("| --- | --- | --- |")
        for entry in members:
            name = Path(str(entry["file"])).stem
            picture = f"{relative.as_posix()}/{group}/{name}.png"
            size = (
                f"{entry['width_millimetres']:.0f} x "
                f"{entry['height_millimetres']:.0f} x "
                f"{entry['depth_millimetres']:.0f} mm"
            )
            lines.append(f"| ![{name}]({picture}) | `{name}`<br>{entry['summary']} | {size} |")
        lines.append("")
    page.parent.mkdir(parents=True, exist_ok=True)
    page.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    arguments = parse_args()
    manifest = json.loads((arguments.models / "manifest.json").read_text())
    entries = [
        entry
        for entry in manifest["models"]
        if not arguments.only or arguments.only in str(entry["file"])
    ]
    if not entries:
        raise SystemExit(f"render error: nothing matches --only {arguments.only!r}")
    for index, entry in enumerate(entries, start=1):
        source = arguments.models / str(entry["file"])
        image = arguments.images / f"{Path(str(entry['file'])).with_suffix('.png')}"
        render(source, image, arguments.width, arguments.height, eye_height(entry))
        print(f"[{index}/{len(entries)}] {image.relative_to(ROOT)}")
    if not arguments.only:
        catalogue(manifest["models"], arguments.images, arguments.page)
        print(f"wrote {arguments.page.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
