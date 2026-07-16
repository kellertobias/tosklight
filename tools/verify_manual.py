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
    metadata = reader.metadata
    if metadata.title != "ToskLight Operator Manual":
        fail(f"unexpected PDF title: {metadata.title!r}")
    text_by_page = [(page.extract_text() or "") for page in reader.pages]
    full_text = "\n".join(text_by_page)
    required = [
        "Contents",
        "Quick Start",
        "Application Layout and Window Manager",
        "Pane Reference",
        "Desk Setup",
        "Show File Setup",
        "The Programmer",
        "Running a Show",
        "Development and Future Features",
        "Dynamics is a future feature.",
        "Index",
    ]
    missing = [title for title in required if title not in full_text]
    if missing:
        fail(f"manual text is missing: {', '.join(missing)}")
    positions = [full_text.index(title) for title in required]
    if positions != sorted(positions):
        fail("manual sections are not in the required order")
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
    # Cover is unnumbered. Every later page must expose its logical footer number.
    for physical, page_text in enumerate(text_by_page[1:], start=2):
        logical = physical - 1
        if str(logical) not in page_text.splitlines():
            fail(f"logical page number {logical} is missing from physical page {physical}")
        if "ToskLight v" not in page_text or "Operator Manual" not in page_text:
            fail(f"software revision footer is missing from physical page {physical}")
    with pdfplumber.open(path) as pdf:
        for number, page in enumerate(pdf.pages, start=1):
            for char in page.chars:
                if char["x0"] < -0.5 or char["x1"] > page.width + 0.5 or char["top"] < -0.5 or char["bottom"] > page.height + 0.5:
                    fail(f"text escapes the media box on page {number}")
    print(f"Verified {path}: {len(reader.pages)} pages, {image_count} embedded images, bookmarks, numbering, contents, and index")


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
