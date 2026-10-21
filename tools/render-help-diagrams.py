#!/usr/bin/env python3
"""Rasterize authored SVG figures for the pinned PDF renderer; keep SVG in in-app Help."""
from pathlib import Path
import shutil
import cairosvg
from artifact_paths import artifact_path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'docs/help/assets/diagrams'
OUTPUT = artifact_path('LIGHT_MANUAL_ROOT', 'MANUAL_ROOT') / 'diagrams'


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    count = 0
    for source in sorted(SOURCE.glob('*.svg')):
        target = OUTPUT / source.with_suffix('.png').name
        cairosvg.svg2png(url=str(source), write_to=str(target), scale=2)
        # Like icon-contact-sheets, mirror only generated image assets inside the Help root:
        # the renderer deliberately rejects image links escaping that root.
        shutil.copyfile(target, source.with_suffix('.png'))
        count += 1
    print(f'Rendered {count} source SVG diagrams at 2x for PDF/HTML')


if __name__ == '__main__':
    main()
