"""Discover and validate Markdown pages used by both manual renderers."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from .config import HELP, ROOT


@dataclass(frozen=True)
class SourcePage:
    path: Path
    relative: str
    title: str
    markdown: str
    bookmark: str
    is_chapter: bool


def slug(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-") or "section"


def manual_order(path: Path) -> tuple[str, ...]:
    parts = list(path.relative_to(HELP).parts)
    if parts[-1].lower() in {"index.md", "index.markdown"}:
        parts[-1] = "!index"
    return tuple(parts)


def markdown_paths() -> list[Path]:
    return sorted(
        (
            path
            for path in HELP.rglob("*")
            if path.is_file() and path.suffix.lower() in {".md", ".markdown"}
        ),
        key=manual_order,
    )


def source_page(path: Path, seen: set[str]) -> SourcePage:
    markdown = path.read_text(encoding="utf-8")
    titles = re.findall(r"^#\s+(.+?)\s*$", markdown, re.MULTILINE)
    if len(titles) != 1:
        raise ValueError(f"{path.relative_to(ROOT)} must contain exactly one first-level heading")
    relative = path.relative_to(HELP).as_posix()
    bookmark = f"page-{slug(relative)}"
    if bookmark in seen:
        raise ValueError(f"duplicate page bookmark for {relative}")
    seen.add(bookmark)
    is_chapter = path.name.lower() in {"index.md", "index.markdown"} or "/" not in relative
    return SourcePage(path, relative, titles[0], markdown, bookmark, is_chapter)


def source_pages() -> list[SourcePage]:
    seen: set[str] = set()
    pages = [source_page(path, seen) for path in markdown_paths()]
    if not pages:
        raise ValueError("docs/help contains no Markdown pages")
    return pages


def local_link_targets(markdown: str) -> list[str]:
    pattern = re.compile(r"!?\[[^]]*\]\(([^)\s]+)(?:\s+['\"].*?['\"])?\)")
    return pattern.findall(markdown)


def validate_local_target(page: SourcePage, target: str, known: set[str]) -> str | None:
    if target.startswith(("http://", "https://", "mailto:", "#", "data:")):
        return None
    clean = target.split("#", 1)[0]
    resolved = (page.path.parent / clean).resolve()
    if not resolved.exists():
        return f"{page.relative}: missing local target {target}"
    if resolved.suffix.lower() not in {".md", ".markdown"}:
        return None
    try:
        relative = resolved.relative_to(HELP).as_posix()
    except ValueError:
        return None
    if relative not in known:
        return f"{page.relative}: unpublished Help page {target}"
    return None


def validate_sources(pages: list[SourcePage]) -> None:
    known = {page.relative for page in pages}
    problems = [
        problem
        for page in pages
        for target in local_link_targets(page.markdown)
        if (problem := validate_local_target(page, target, known)) is not None
    ]
    if problems:
        raise ValueError("\n".join(problems))
