"""Compose and write the operator manual PDF."""

from __future__ import annotations

from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.units import mm
from reportlab.platypus import (
    Flowable,
    Image,
    NextPageTemplate,
    PageBreak,
    Paragraph,
    Spacer,
)
from reportlab.platypus.tableofcontents import SimpleIndex, TableOfContents

from .config import (
    INK,
    MUTED,
    ROOT,
    SANS,
    SANS_BOLD,
    SOFTWARE_VERSION,
    STYLES,
)
from .markdown import markdown_story
from .source import SourcePage, source_pages, validate_sources
from .template import ChapterMarker, ManualDocTemplate


def create_document(output: Path) -> ManualDocTemplate:
    return ManualDocTemplate(
        str(output),
        pagesize=A4,
        title="ToskLight Operator Manual",
        author="ToskLight contributors",
        subject="Desk setup, show setup, programming, and show operation",
        creator="tools/build_manual.py from docs/help Markdown",
        keywords=f"ToskLight operator manual software version {SOFTWARE_VERSION}",
        leftMargin=20 * mm,
        rightMargin=20 * mm,
        topMargin=19 * mm,
        bottomMargin=19 * mm,
    )


def cover_story() -> list[Flowable]:
    story: list[Flowable] = []
    logo = ROOT / "apps" / "control-ui" / "src-tauri" / "icons" / "icon.png"
    if logo.is_file():
        mark = Image(str(logo), width=38 * mm, height=38 * mm)
        mark.hAlign = "LEFT"
        story.extend([Spacer(1, 34 * mm), mark, Spacer(1, 19 * mm)])
    kicker = ParagraphStyle(
        name="CoverKicker",
        fontName=SANS_BOLD,
        fontSize=7.5,
        leading=11,
        textColor=colors.white,
    )
    story.extend([
        Paragraph("ToskLight", STYLES["ManualCoverTitle"]),
        Paragraph(f"Operator manual / software v{SOFTWARE_VERSION}", STYLES["ManualCoverSub"]),
        Spacer(1, 83 * mm),
        Paragraph("DESK SETUP  /  SHOW SETUP  /  PROGRAMMING  /  RUNNING A SHOW", kicker),
        NextPageTemplate("Contents"),
        PageBreak(),
        Paragraph("Contents", STYLES["ManualContentsTitle"]),
    ])
    return story


def contents_flowable() -> TableOfContents:
    contents = TableOfContents()
    contents.levelStyles = [
        ParagraphStyle(name="TOC1", fontName=SANS_BOLD, fontSize=11, leading=15, leftIndent=0, firstLineIndent=0, textColor=colors.HexColor("#0f766e"), spaceBefore=7, spaceAfter=2),
        ParagraphStyle(name="TOC2", fontName=SANS, fontSize=8.4, leading=11.5, leftIndent=11, firstLineIndent=0, textColor=INK),
        ParagraphStyle(name="TOC3", fontName=SANS, fontSize=7.8, leading=10.5, leftIndent=22, firstLineIndent=0, textColor=MUTED),
    ]
    return contents


def help_story(pages: list[SourcePage], bookmarks: dict[str, str]) -> list[Flowable]:
    story: list[Flowable] = []
    for page in pages:
        story.extend([ChapterMarker(page.title), NextPageTemplate("Body"), PageBreak()])
        story.extend(markdown_story(page, bookmarks))
    return story


def index_story() -> tuple[list[Flowable], SimpleIndex]:
    index = SimpleIndex(dot=" . ", headers=False)
    index.textStyle = ParagraphStyle(
        name="IndexText",
        fontName=SANS,
        fontSize=8,
        leading=11,
        textColor=INK,
    )
    story: list[Flowable] = [
        ChapterMarker("Index"),
        PageBreak(),
        Paragraph("Index", STYLES["ManualContentsTitle"]),
        Paragraph(
            "Topics and feature names are indexed from the headings in the Markdown source.",
            STYLES["ManualBody"],
        ),
        index,
    ]
    return story, index


def build(output: Path) -> None:
    pages = source_pages()
    validate_sources(pages)
    bookmarks = {page.relative: page.bookmark for page in pages}
    output.parent.mkdir(parents=True, exist_ok=True)
    document = create_document(output)
    story = cover_story()
    story.append(contents_flowable())
    story.extend(help_story(pages, bookmarks))
    index_pages, index = index_story()
    story.extend(index_pages)
    document.multiBuild(story, canvasmaker=index.getCanvasMaker())
    print(f"Built {output} from {len(pages)} Markdown pages")
