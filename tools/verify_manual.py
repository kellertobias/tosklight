#!/usr/bin/env python3
"""Structural checks for the generated ToskLight manual PDF."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import pdfplumber
from pypdf import PdfReader


def fail(message: str) -> None:
    raise ValueError(message)


def verify(path: Path) -> None:
    if not path.is_file() or path.stat().st_size < 100_000:
        fail(f"manual is missing or unexpectedly small: {path}")
    reader = PdfReader(str(path))
    if len(reader.pages) < 20:
        fail(f"manual has only {len(reader.pages)} pages")
    expected_width = 595.28
    expected_height = 841.89
    for number, page in enumerate(reader.pages, start=1):
        width = float(page.mediabox.width)
        height = float(page.mediabox.height)
        if abs(width - expected_width) > 0.75 or abs(height - expected_height) > 0.75:
            fail(f"page {number} is {width:.2f} x {height:.2f} pt instead of A4")
    metadata = reader.metadata
    if metadata.title != "ToskLight Operator Manual":
        fail(f"unexpected PDF title: {metadata.title!r}")
    text_by_page = [(page.extract_text() or "") for page in reader.pages]
    full_text = "\n".join(text_by_page)
    required = [
        "Contents",
        "Quick Start",
        "ToskLight Control",
        "Show Setup and Patching",
        "Programmer and Cues",
        "Windows and Panes",
        "ToskLight Architect",
        "ToskLight Pixel",
        "Protocols",
    ]
    missing = [title for title in required if title not in full_text]
    if missing:
        fail(f"manual text is missing: {', '.join(missing)}")
    positions = [full_text.index(title) for title in required]
    if positions != sorted(positions):
        fail("manual sections are not in the required order")
    if "[!danger]" in full_text or "Missing graphic" not in full_text:
        fail("Obsidian danger callouts were not rendered correctly")
    if not reader.outline:
        fail("manual has no PDF outline/bookmarks")
    image_count = 0
    for page in reader.pages:
        resources = page.get("/Resources") or {}
        xobjects = resources.get("/XObject") or {}
        for item in xobjects.values():
            if item.get_object().get("/Subtype") == "/Image":
                image_count += 1
    if image_count < 50:
        fail(f"manual contains only {image_count} embedded images")
    # Cover is unnumbered. React-PDF exposes physical page and total page count in every later footer.
    for physical, page_text in enumerate(text_by_page[1:], start=2):
        if f"{physical} / {len(reader.pages)}" not in page_text:
            fail(f"page number {physical} of {len(reader.pages)} is missing from physical page {physical}")
        if "ToskLight v" not in page_text or "Operator Manual" not in page_text:
            fail(f"software revision footer is missing from physical page {physical}")
    with pdfplumber.open(path) as pdf:
        for number, page in enumerate(pdf.pages, start=1):
            for char in page.chars:
                if char["x0"] < -0.5 or char["x1"] > page.width + 0.5 or char["top"] < -0.5 or char["bottom"] > page.height + 0.5:
                    fail(f"text escapes the media box on page {number}")
    print(f"Verified {path}: {len(reader.pages)} pages, {image_count} embedded images, bookmarks, numbering, and contents")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("pdf", type=Path)
    args = parser.parse_args()
    try:
        verify(args.pdf.resolve())
    except Exception as error:
        print(f"manual verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
