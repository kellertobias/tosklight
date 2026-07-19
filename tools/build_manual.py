#!/usr/bin/env python3
"""Build the ToskLight operator manual from docs/help Markdown.

This module remains the stable entry point for the build wrapper and HTML
manual generator. PDF responsibilities live in the ``manual`` package.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from manual.build_pdf import build
from manual.config import DEFAULT_OUTPUT, HELP, ROOT, workspace_version
from manual.keycaps import configure_asset_directory
from manual.source import SourcePage, slug, source_pages, validate_sources

__all__ = [
    "DEFAULT_OUTPUT",
    "HELP",
    "ROOT",
    "SourcePage",
    "build",
    "slug",
    "source_pages",
    "validate_sources",
    "workspace_version",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def keycap_directory(output: Path) -> Path:
    configured = os.environ.get("LIGHT_MANUAL_KEYCAP_DIR")
    return Path(configured) if configured else output.resolve().parent / ".manual-keycaps"


def main() -> int:
    output = parse_args().output.resolve()
    configure_asset_directory(keycap_directory(output))
    try:
        build(output)
    except Exception as error:
        print(f"manual build failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
