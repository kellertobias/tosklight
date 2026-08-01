#!/usr/bin/env python3
"""Put a wheel of shipped gobo artwork into the fixture packages that have a gobo wheel.

The artwork in `assets/gobos` is the repository's own; this places it slot by slot in the
packages whose fixtures actually carry a rotating or fixed wheel, so those profiles project their
own patterns instead of the visualizer's stand-ins. A fixture with no wheel is left alone.

    python3 tools/gobo-package.py            # write the wheels into assets/fixture-library
    python3 tools/gobo-package.py --check    # fail if a package is missing its wheel

Validate afterwards with:

    cargo run -p light-fixture --bin fixture-package -- validate assets/fixture-library/*.toskfixture
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import sys
import tempfile
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
LIBRARY = ROOT / "assets" / "fixture-library"
ARTWORK = ROOT / "assets" / "gobos"

# Slot 0 is the open slot on every wheel and carries no artwork, so it is never declared.
WHEELS: dict[str, list[tuple[int, str, str]]] = {
    # A profile moving head's rotating wheel: the working set a hire stock carries.
    "robe--robin-dls-profile": [
        (1, "Breakup", "breakup"),
        (2, "Dots", "dots"),
        (3, "Spokes", "spokes"),
        (4, "Rings", "rings"),
        (5, "Triangle", "triangle"),
        (6, "Bars", "bars"),
        (7, "Stars", "stars"),
    ],
    # A beam light's wheel is shorter and aerial: shapes that read in the air rather than on a
    # surface, because that is what a beam is for.
    "claypaky--sharpy": [
        (1, "Spokes", "spokes"),
        (2, "Rings", "rings"),
        (3, "Dots", "dots"),
        (4, "Stars", "stars"),
    ],
    # A scanner throws across a room and is usually asked for movement rather than detail.
    "high-end-systems--trackspot": [
        (1, "Breakup", "breakup"),
        (2, "Dots", "dots"),
        (3, "Spokes", "spokes"),
        (4, "Triangle", "triangle"),
    ],
}


def wheel_for(package: pathlib.Path) -> list[tuple[int, str, str]] | None:
    return WHEELS.get(package.stem)


def apply_wheel(package: pathlib.Path, wheel: list[tuple[int, str, str]], check: bool) -> bool:
    """Return True when the package on disk already carries exactly this wheel."""
    with zipfile.ZipFile(package) as archive:
        entries = {name: archive.read(name) for name in archive.namelist()}
    manifest = json.loads(entries["fixture.json"])
    profile = manifest["profile"]

    gobos = []
    artwork: dict[str, bytes] = {}
    for slot, name, pattern in wheel:
        path = f"assets/gobo-{slot}.png"
        gobos.append({"slot": slot, "name": name, "artwork_asset": path})
        artwork[path] = (ARTWORK / f"{pattern}.png").read_bytes()

    current_matches = profile.get("gobos") == gobos and all(
        entries.get(path) == data for path, data in artwork.items()
    )
    if check or current_matches:
        return current_matches

    profile["gobos"] = gobos
    entries["fixture.json"] = json.dumps(manifest, indent=2).encode() + b"\n"
    # Drop any wheel a previous run wrote before adding this one, so a shortened wheel does not
    # leave an orphan file behind — the reader refuses an archive entry nothing references.
    for name in [name for name in entries if name.startswith("assets/gobo-")]:
        del entries[name]
    entries.update(artwork)

    with tempfile.NamedTemporaryFile(dir=package.parent, suffix=".tmp", delete=False) as handle:
        temporary = pathlib.Path(handle.name)
    with zipfile.ZipFile(temporary, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("fixture.json", entries.pop("fixture.json"))
        for name in sorted(entries):
            archive.writestr(name, entries[name])
    shutil.move(temporary, package)
    return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report rather than write")
    arguments = parser.parse_args()

    missing = []
    for stem in WHEELS:
        package = LIBRARY / f"{stem}.toskfixture"
        if not package.exists():
            print(f"no package for {stem}", file=sys.stderr)
            return 1
        wheel = wheel_for(package)
        assert wheel is not None
        if apply_wheel(package, wheel, arguments.check):
            print(f"{package.name}: {len(wheel)} slots, unchanged")
        elif arguments.check:
            missing.append(package.name)
        else:
            print(f"{package.name}: wrote {len(wheel)} slots")
    if missing:
        print(f"gobo wheels are out of date: {', '.join(missing)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
