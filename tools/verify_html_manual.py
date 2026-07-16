#!/usr/bin/env python3
"""Verify the offline HTML manual site and deployment archive."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import zipfile
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath


class ManualParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.ids: list[str] = []
        self.hrefs: list[str] = []
        self.images: list[str] = []
        self.external_resources: list[str] = []
        self.styles = 0
        self.scripts = 0
        self.articles = 0
        self.title = ""
        self._in_title = False

    def handle_starttag(self, tag: str, attrs) -> None:
        values = dict(attrs)
        if identifier := values.get("id"):
            self.ids.append(identifier)
        if tag == "a" and (href := values.get("href")):
            self.hrefs.append(href)
        if tag == "img" and (source := values.get("src")):
            self.images.append(source)
        if tag in {"script", "link", "img"}:
            source = values.get("src") or values.get("href")
            if source and source.startswith(("http://", "https://", "//")):
                self.external_resources.append(source)
        if tag == "style":
            self.styles += 1
        if tag == "script":
            self.scripts += 1
        if tag == "article" and "manual-page" in values.get("class", "").split():
            self.articles += 1
        if tag == "title":
            self._in_title = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "title":
            self._in_title = False

    def handle_data(self, data: str) -> None:
        if self._in_title:
            self.title += data


def fail(message: str) -> None:
    raise ValueError(message)


def safe_relative(path: str) -> bool:
    candidate = PurePosixPath(path)
    return not candidate.is_absolute() and ".." not in candidate.parts


def verify(site: Path, archive: Path) -> None:
    index = site / "index.html"
    if not index.is_file() or index.stat().st_size < 100_000:
        fail(f"HTML manual is missing or unexpectedly small: {index}")
    source = index.read_text(encoding="utf-8")
    parser = ManualParser()
    parser.feed(source)
    if parser.styles != 1 or parser.scripts != 1:
        fail("HTML manual must contain exactly one inline style and one inline script")
    if parser.articles < 30:
        fail(f"HTML manual contains only {parser.articles} pages")
    if len(parser.ids) != len(set(parser.ids)):
        fail("HTML manual contains duplicate element IDs")
    if parser.external_resources:
        fail(f"HTML manual loads external resources: {parser.external_resources[0]}")
    required = [
        "ToskLight Operator Manual",
        "Search the manual",
        "Quick Start",
        "Pane Reference",
        "Desk Setup",
        "Show File Setup",
        "The Programmer",
        "Running a Show",
        "Dynamics is a future feature.",
    ]
    missing = [text for text in required if text not in source]
    if missing:
        fail(f"HTML manual is missing: {', '.join(missing)}")
    known_ids = set(parser.ids)
    for href in parser.hrefs:
        if href.startswith("#") and href[1:] not in known_ids:
            fail(f"internal link has no target: {href}")
    for image in parser.images:
        if not safe_relative(image):
            fail(f"unsafe image path: {image}")
        if not (site / image).is_file():
            fail(f"missing HTML manual image: {image}")
    dynamics = re.search(r'<section class="future-feature">.*?</section>', source, re.DOTALL)
    if not dynamics or "<img " in dynamics.group(0):
        fail("Dynamics must be a text-only future-feature page")
    if not all(marker in source for marker in ('desk-key-number', 'desk-key-clear', 'desk-key-record', 'keyboard-key')):
        fail("HTML manual keycap variants are incomplete")
    if not archive.is_file() or archive.stat().st_size < 100_000:
        fail(f"HTML deployment archive is missing or unexpectedly small: {archive}")
    site_files = sorted(path.relative_to(site).as_posix() for path in site.rglob("*") if path.is_file())
    with zipfile.ZipFile(archive) as bundle:
        names = bundle.namelist()
        if names != sorted(names) or len(names) != len(set(names)):
            fail("HTML deployment archive entries are not sorted and unique")
        if any(not safe_relative(name) for name in names):
            fail("HTML deployment archive contains an unsafe path")
        if names != site_files or "index.html" not in names:
            fail("HTML deployment archive does not exactly match the verified site")
        if bundle.read("index.html") != index.read_bytes():
            fail("archived index.html differs from the verified site")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()[:16]
    print(f"Verified {index}: {parser.articles} pages, {len(set(parser.images))} images, offline resources, navigation, search, safe links, and ZIP sha256 {digest}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("site", type=Path)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    try:
        verify(args.site.resolve(), args.archive.resolve())
    except Exception as error:
        print(f"HTML manual verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
