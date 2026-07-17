#!/usr/bin/env python3
"""Build the ToskLight operator manual from docs/help Markdown."""

from __future__ import annotations

import argparse
import hashlib
import html
import os
import re
import sys
from urllib.parse import quote, unquote
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from markdown_it import MarkdownIt
from PIL import Image as PILImage, ImageDraw, ImageFont
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from artifact_paths import artifact_path
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate,
    Flowable,
    Frame,
    HRFlowable,
    Image,
    KeepTogether,
    ListFlowable,
    ListItem,
    NextPageTemplate,
    PageBreak,
    PageTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
)
from reportlab.platypus.tableofcontents import SimpleIndex, TableOfContents

ROOT = Path(__file__).resolve().parents[1]
HELP = ROOT / "docs" / "help"
DEFAULT_OUTPUT = artifact_path("LIGHT_MANUAL_ROOT", "MANUAL_ROOT") / "pdf" / "tosklight-manual.pdf"
PAGE_WIDTH, PAGE_HEIGHT = A4
INK = colors.HexColor("#17202a")
MUTED = colors.HexColor("#64748b")
TEAL = colors.HexColor("#12b8a6")
CYAN = colors.HexColor("#5ee7f0")
NAVY = colors.HexColor("#071621")
PAPER = colors.HexColor("#f7f6f1")
GRID = colors.HexColor("#d8dee5")


def register_fonts() -> tuple[str, str, str, str]:
    candidates = [
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        ),
        (
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial Italic.ttf",
            "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
        ),
    ]
    for regular, bold, italic, mono in candidates:
        if all(Path(path).is_file() for path in (regular, bold, italic, mono)):
            pdfmetrics.registerFont(TTFont("ManualSans", regular))
            pdfmetrics.registerFont(TTFont("ManualSansBold", bold))
            pdfmetrics.registerFont(TTFont("ManualSansItalic", italic))
            pdfmetrics.registerFont(TTFont("ManualMono", mono))
            pdfmetrics.registerFontFamily(
                "ManualSans",
                normal="ManualSans",
                bold="ManualSansBold",
                italic="ManualSansItalic",
                boldItalic="ManualSansBold",
            )
            return "ManualSans", "ManualSansBold", "ManualSansItalic", "ManualMono"
    return "Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Courier"


SANS, SANS_BOLD, SANS_ITALIC, MONO = register_fonts()


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


def source_pages() -> list[SourcePage]:
    def manual_order(path: Path) -> tuple[str, ...]:
        parts = list(path.relative_to(HELP).parts)
        if parts[-1].lower() in {"index.md", "index.markdown"}:
            parts[-1] = "!index"
        return tuple(parts)

    paths = sorted(
        (
            path
            for path in HELP.rglob("*")
            if path.is_file() and path.suffix.lower() in {".md", ".markdown"}
        ),
        key=manual_order,
    )
    pages: list[SourcePage] = []
    seen: set[str] = set()
    for path in paths:
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
        pages.append(SourcePage(path, relative, titles[0], markdown, bookmark, is_chapter))
    if not pages:
        raise ValueError("docs/help contains no Markdown pages")
    return pages


def validate_sources(pages: list[SourcePage]) -> None:
    known = {page.relative for page in pages}
    problems: list[str] = []
    link_pattern = re.compile(r"!?\[[^]]*\]\(([^)\s]+)(?:\s+['\"].*?['\"])?\)")
    for page in pages:
        for target in link_pattern.findall(page.markdown):
            if target.startswith(("http://", "https://", "mailto:", "#", "data:")):
                continue
            clean = target.split("#", 1)[0]
            resolved = (page.path.parent / clean).resolve()
            if target.startswith("!"):
                continue
            if not resolved.exists():
                problems.append(f"{page.relative}: missing local target {target}")
            elif resolved.suffix.lower() in {".md", ".markdown"}:
                try:
                    relative = resolved.relative_to(HELP).as_posix()
                except ValueError:
                    continue
                if relative not in known:
                    problems.append(f"{page.relative}: unpublished Help page {target}")
    if problems:
        raise ValueError("\n".join(problems))


def make_styles():
    styles = getSampleStyleSheet()
    styles.add(ParagraphStyle(name="ManualBody", fontName=SANS, fontSize=9.2, leading=13.1, textColor=INK, spaceAfter=6.5, allowWidows=0, allowOrphans=0))
    styles.add(ParagraphStyle(name="ManualLead", parent=styles["ManualBody"], fontSize=11.2, leading=16, textColor=colors.HexColor("#334155"), spaceAfter=12))
    styles.add(ParagraphStyle(name="ManualH1", fontName=SANS_BOLD, fontSize=25, leading=29, textColor=NAVY, spaceBefore=0, spaceAfter=14, keepWithNext=True))
    styles.add(ParagraphStyle(name="ManualChapterH1", parent=styles["ManualH1"], fontSize=27, leading=32, textColor=colors.white, backColor=NAVY, borderPadding=(10, 12, 11, 12), spaceAfter=7, underlineWidth=1.2, underlineColor=CYAN))
    styles.add(ParagraphStyle(name="ManualH2", fontName=SANS_BOLD, fontSize=16, leading=20, textColor=colors.HexColor("#0f766e"), spaceBefore=15, spaceAfter=7, keepWithNext=True))
    styles.add(ParagraphStyle(name="ManualH3", fontName=SANS_BOLD, fontSize=11.5, leading=15, textColor=NAVY, spaceBefore=10, spaceAfter=5, keepWithNext=True))
    styles.add(ParagraphStyle(name="ManualH4", fontName=SANS_BOLD, fontSize=9.5, leading=13, textColor=INK, spaceBefore=8, spaceAfter=4, keepWithNext=True))
    styles.add(ParagraphStyle(name="ManualCode", fontName=MONO, fontSize=7.4, leading=10.5, textColor=colors.HexColor("#d9f7f4"), backColor=NAVY, borderPadding=8, spaceBefore=5, spaceAfter=8))
    styles.add(ParagraphStyle(name="ManualQuote", parent=styles["ManualBody"], leftIndent=12, borderWidth=0, borderColor=TEAL, borderPadding=(2, 0, 2, 9), textColor=colors.HexColor("#475569"), backColor=colors.HexColor("#eaf7f5")))
    styles.add(ParagraphStyle(name="ManualFutureFeature", fontName=SANS_BOLD, fontSize=22, leading=29, alignment=TA_CENTER, textColor=colors.black, backColor=colors.white, borderWidth=1, borderColor=colors.black, borderPadding=20, keepTogether=True))
    styles.add(ParagraphStyle(name="ManualCaption", fontName=SANS_ITALIC, fontSize=7.5, leading=10, alignment=TA_CENTER, textColor=MUTED, spaceBefore=3, spaceAfter=10))
    styles.add(ParagraphStyle(name="ManualTable", fontName=SANS, fontSize=7.1, leading=9.4, textColor=INK))
    styles.add(ParagraphStyle(name="ManualTableHead", fontName=SANS_BOLD, fontSize=7.1, leading=9.4, textColor=colors.white))
    styles.add(ParagraphStyle(name="ManualCoverTitle", fontName=SANS_BOLD, fontSize=35, leading=39, textColor=colors.white, alignment=TA_LEFT, spaceAfter=8))
    styles.add(ParagraphStyle(name="ManualCoverSub", fontName=SANS, fontSize=14, leading=19, textColor=CYAN, alignment=TA_LEFT))
    styles.add(ParagraphStyle(name="ManualContentsTitle", fontName=SANS_BOLD, fontSize=26, leading=31, textColor=NAVY, spaceAfter=16))
    for style in styles.byName.values():
        style.allowWidows = 0
        style.allowOrphans = 0
    return styles


STYLES = make_styles()


def workspace_version() -> str:
    override = os.environ.get("LIGHT_MANUAL_VERSION")
    if override:
        return override
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r"\[workspace\.package\][\s\S]*?^version\s*=\s*\"([^\"]+)\"", cargo, re.MULTILINE)
    return match.group(1) if match else "development"


SOFTWARE_VERSION = workspace_version()

KEYCAP_PALETTES = {
    "number": ("#171c22", "#3a4652", "#090c10", "#edf3f6"),
    "command": ("#171c22", "#3a4652", "#090c10", "#ffb30f"),
    "clear": ("#261d08", "#d6a600", "#806000", "#f0c52f"),
    "record": ("#21090c", "#ff6872", "#70181f", "#ff6872"),
    "keyboard": ("#ffffff", "#cbd5e1", "#7d8b94", "#17202a"),
}
KEYCAP_ASSET_DIR = DEFAULT_OUTPUT.parent / ".manual-keycaps"


def keycap_asset(label: str, category: str) -> tuple[Path, float, float]:
    """Render an indivisible high-resolution inline keycap for Paragraph layout."""
    scale = 4
    horizontal_gap = round(1.4 * scale)
    font_size = 7.4
    font_path = (
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
        if category == "keyboard"
        else "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
    )
    if not Path(font_path).is_file():
        font_path = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
    font = ImageFont.truetype(font_path, round(font_size * scale))
    probe = ImageDraw.Draw(PILImage.new("RGBA", (1, 1)))
    box = probe.textbbox((0, 0), label, font=font)
    text_width = box[2] - box[0]
    text_height = box[3] - box[1]
    key_width = max(round(15 * scale), text_width + round(7 * scale))
    width = key_width + horizontal_gap * 2
    height = round(11.2 * scale)
    shadow = round(1.2 * scale)
    fill, border, shadow_color, text_color = KEYCAP_PALETTES[category]
    digest = hashlib.sha1(f"spaced-v2\0{category}\0{label}".encode()).hexdigest()[:12]
    path = KEYCAP_ASSET_DIR / f"{category}-{digest}.png"
    if not path.is_file():
        KEYCAP_ASSET_DIR.mkdir(parents=True, exist_ok=True)
        image = PILImage.new("RGBA", (width, height + shadow), (255, 255, 255, 0))
        draw = ImageDraw.Draw(image)
        radius = round(2.8 * scale)
        left = horizontal_gap
        right = left + key_width - 1
        draw.rounded_rectangle((left, shadow, right, height + shadow - 1), radius=radius, fill=shadow_color)
        draw.rounded_rectangle((left, 0, right, height - 1), radius=radius, fill=fill, outline=border, width=max(1, scale))
        x = left + (key_width - text_width) / 2 - box[0]
        y = (height - text_height) / 2 - box[1]
        draw.text((x, y), label, font=font, fill=text_color)
        image.save(path)
    return path, width / scale, (height + shadow) / scale


def keycap_category(label: str) -> str:
    if re.fullmatch(r"(?:\d|0-9|\.)", label):
        return "number"
    if label == "CLR":
        return "clear"
    if label == "REC":
        return "record"
    return "command"


def keycap_markup(label: str, category: str) -> str:
    path, width, height = keycap_asset(label, category)
    return f'<img src="{html.escape(str(path), quote=True)}" width="{width:.2f}" height="{height:.2f}" valign="-3"/>'


def desk_keycap_markup(raw: str) -> str:
    label = raw.strip()
    state = ""
    if len(label) > 1 and label[-1:] in {"+", "*"}:
        state = "hold" if label[-1] == "+" else "optional"
        label = label[:-1]
    keycap = keycap_markup(label, keycap_category(label))
    if state:
        return f'{keycap}<font name="{SANS_BOLD}" size="5.5" color="#64748b"> {state.upper()}</font>'
    return keycap


def draw_manual_keycap(canvas, _kind, label: str | None) -> None:
    if not label or "|" not in label:
        return
    category, encoded = label.split("|", 1)
    key = unquote(encoded)
    fill, border, shadow, _text = KEYCAP_PALETTES.get(category, KEYCAP_PALETTES["command"])
    font = MONO if category == "keyboard" else SANS_BOLD
    # Match the exact advance of the non-breaking-space padded label. Drawing
    # beyond that advance lets the next adjacent keycap paint over this one.
    width = pdfmetrics.stringWidth(f" {key} ", font, 7.4)
    info = canvas._curr_tx_info
    x = info["cur_x"]
    y = info["cur_y"] - 2.5
    canvas.saveState()
    canvas.setFillColor(colors.HexColor(shadow))
    canvas.roundRect(x, y - 1.4, width, 11.1, 2.8, fill=1, stroke=0)
    canvas.setFillColor(colors.HexColor(fill))
    canvas.setStrokeColor(colors.HexColor(border))
    canvas.setLineWidth(0.65)
    canvas.roundRect(x, y, width, 10.2, 2.8, fill=1, stroke=1)
    canvas.restoreState()


def inline_markup(text: str, page: SourcePage, bookmarks: dict[str, str]) -> str:
    text = html.escape(text.strip(), quote=False)
    text = re.sub(r"`([^`]+)`", lambda m: f'<font name="{MONO}" color="#0f766e">{m.group(1)}</font>', text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<i>\1</i>", text)

    def link(match: re.Match[str]) -> str:
        label, target = match.group(1), html.unescape(match.group(2))
        if target.startswith(("http://", "https://", "mailto:")):
            return f'<link href="{html.escape(target, quote=True)}" color="#087f8c">{label}</link>'
        clean = target.split("#", 1)[0]
        if not clean:
            return label
        resolved = (page.path.parent / clean).resolve()
        try:
            relative = resolved.relative_to(HELP).as_posix()
        except ValueError:
            return label
        destination = bookmarks.get(relative)
        return f'<link href="#{destination}" color="#087f8c">{label}</link>' if destination else label

    text = re.sub(r"\[([^]]+)\]\(([^)\s]+)(?:\s+[^)]*)?\)", link, text)
    text = re.sub(
        r"\[KBD:([^\]\n]+)\]",
        lambda m: keycap_markup(m.group(1).strip(), "keyboard"),
        text,
    )
    text = re.sub(
        r"\[\s*([+\-−^.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]",
        lambda m: desk_keycap_markup(m.group(1)),
        text,
    )
    return text.replace("<br>", "<br/>").replace("&lt;br&gt;", "<br/>")


def paragraph(text: str, style: str, page: SourcePage, bookmarks: dict[str, str]) -> Paragraph:
    return Paragraph(inline_markup(text, page, bookmarks), STYLES[style])


def parse_table(lines: list[str], page: SourcePage, bookmarks: dict[str, str]) -> Table:
    rows: list[list[Paragraph]] = []
    for row_index, line in enumerate(lines):
        if row_index == 1 and re.fullmatch(r"\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*", line):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        style = "ManualTableHead" if not rows else "ManualTable"
        rows.append([paragraph(cell.replace("\\|", "|"), style, page, bookmarks) for cell in cells])
    columns = max(len(row) for row in rows)
    for row in rows:
        row.extend([Paragraph("", STYLES["ManualTable"])] * (columns - len(row)))
    available = PAGE_WIDTH - 40 * mm
    widths = [available / columns] * columns
    table = Table(rows, colWidths=widths, repeatRows=1, hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), NAVY),
        ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
        ("GRID", (0, 0), (-1, -1), 0.35, GRID),
        ("BACKGROUND", (0, 1), (-1, -1), colors.white),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.HexColor("#f1f5f4")]),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 5),
        ("RIGHTPADDING", (0, 0), (-1, -1), 5),
        ("TOPPADDING", (0, 0), (-1, -1), 4),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
    ]))
    return table


def image_flowable(source: Path, alt: str) -> list[Flowable]:
    if not source.is_file():
        raise ValueError(f"missing manual image: {source.relative_to(ROOT)}")
    image = Image(str(source))
    maximum_width = PAGE_WIDTH - 40 * mm
    maximum_height = 125 * mm
    scale = min(maximum_width / image.imageWidth, maximum_height / image.imageHeight, 1)
    image.drawWidth = image.imageWidth * scale
    image.drawHeight = image.imageHeight * scale
    image.hAlign = "CENTER"
    return [Spacer(1, 4), image, Paragraph(html.escape(alt), STYLES["ManualCaption"])]


def markdown_story(page: SourcePage, bookmarks: dict[str, str]) -> list[Flowable]:
    # Parse once with the same CommonMark engine used for validation of block boundaries.
    MarkdownIt("commonmark", {"html": False}).enable("table").parse(page.markdown)
    lines = page.markdown.splitlines()
    story: list[Flowable] = []
    index = 0
    first_paragraph = True
    while index < len(lines):
        raw = lines[index]
        stripped = raw.strip()
        if not stripped:
            index += 1
            continue
        image_match = re.fullmatch(r"!\[([^]]*)\]\(([^)\s]+)(?:\s+[^)]*)?\)", stripped)
        if image_match:
            story.extend(image_flowable((page.path.parent / image_match.group(2)).resolve(), image_match.group(1)))
            index += 1
            continue
        heading = re.match(r"^(#{1,4})\s+(.+?)\s*$", stripped)
        if heading:
            level = len(heading.group(1))
            title = heading.group(2)
            bookmark = page.bookmark if level == 1 else f"{page.bookmark}-{slug(title)}"
            style_name = "ManualChapterH1" if level == 1 and page.is_chapter else f"ManualH{level}"
            rendered = inline_markup(title, page, bookmarks)
            if level == 1 and page.is_chapter:
                rendered = f"<u color=\"#5ee7f0\">{rendered}</u>"
            heading_flowable = Paragraph(f'<a name="{bookmark}"/><index item="{html.escape(title, quote=True)}"/>{rendered}', STYLES[style_name])
            if level == 1:
                heading_flowable.manual_toc_level = 0 if page.is_chapter else 1
            else:
                heading_flowable.manual_toc_level = min(level - (1 if page.is_chapter else 0), 2)
            heading_flowable.manual_plain_title = title
            heading_flowable.manual_is_chapter = level == 1 and page.is_chapter
            story.append(heading_flowable)
            if level == 1 and page.is_chapter:
                story.append(HRFlowable(width="100%", thickness=1.2, color=TEAL, spaceBefore=0, spaceAfter=11))
            first_paragraph = level == 1
            index += 1
            continue
        if stripped.startswith("```"):
            language = stripped[3:].strip()
            code: list[str] = []
            index += 1
            while index < len(lines) and not lines[index].strip().startswith("```"):
                code.append(lines[index])
                index += 1
            index += 1
            label = f"{language}\n" if language else ""
            story.append(Paragraph((html.escape(label + "\n".join(code)) or " ").replace("\n", "<br/>"), STYLES["ManualCode"]))
            continue
        if stripped == "---":
            rule = Table([[""]], colWidths=[PAGE_WIDTH - 40 * mm], rowHeights=[1])
            rule.setStyle(TableStyle([("BACKGROUND", (0, 0), (-1, -1), GRID)]))
            story.extend([Spacer(1, 5), rule, Spacer(1, 5)])
            index += 1
            continue
        if "|" in stripped and index + 1 < len(lines) and re.match(r"^\s*\|?\s*:?-+", lines[index + 1]):
            table_lines = [raw, lines[index + 1]]
            index += 2
            while index < len(lines) and "|" in lines[index] and lines[index].strip():
                table_lines.append(lines[index])
                index += 1
            story.extend([parse_table(table_lines, page, bookmarks), Spacer(1, 7)])
            continue
        list_match = re.match(r"^\s*(?:[-*+]\s+|(\d+)\.\s+)(.+)$", raw)
        if list_match:
            ordered = list_match.group(1) is not None
            items: list[ListItem] = []
            while index < len(lines):
                match = re.match(r"^\s*(?:[-*+]\s+|(\d+)\.\s+)(.+)$", lines[index])
                if not match or (match.group(1) is not None) != ordered:
                    break
                items.append(ListItem(paragraph(match.group(2), "ManualBody", page, bookmarks), leftIndent=13, value=int(match.group(1)) if match.group(1) else None))
                index += 1
            list_options = {
                "bulletType": "1" if ordered else "bullet",
                "leftIndent": 18,
                "bulletFontName": SANS_BOLD,
                "bulletFontSize": 7,
                "spaceAfter": 7,
            }
            if ordered:
                list_options["start"] = "1"
            listing = ListFlowable(items, **list_options)
            story.append(KeepTogether([listing]) if len(items) <= 5 else listing)
            continue
        if stripped.startswith(">"):
            quote: list[str] = []
            while index < len(lines) and lines[index].strip().startswith(">"):
                quote.append(lines[index].strip()[1:].strip())
                index += 1
            quote_text = " ".join(quote)
            if re.sub(r"[*_]", "", quote_text).strip().lower() == "dynamics is a future feature.":
                story.extend([PageBreak(), Spacer(1, 88 * mm), paragraph("Dynamics is a future feature.", "ManualFutureFeature", page, bookmarks)])
            else:
                story.append(paragraph(quote_text, "ManualQuote", page, bookmarks))
            continue
        block = [stripped]
        index += 1
        while index < len(lines) and lines[index].strip():
            candidate = lines[index].strip()
            if re.match(r"^(#{1,4})\s+", candidate) or candidate.startswith("```") or re.match(r"^(?:[-*+]\s+|\d+\.\s+)", candidate):
                break
            if "|" in candidate and index + 1 < len(lines) and re.match(r"^\s*\|?\s*:?-+", lines[index + 1]):
                break
            block.append(candidate)
            index += 1
        style = "ManualLead" if first_paragraph else "ManualBody"
        first_paragraph = False
        story.append(paragraph(" ".join(block), style, page, bookmarks))
    return keep_section_openers_with_images(story)


def keep_section_openers_with_images(story: list[Flowable]) -> list[Flowable]:
    """Keep a screenshot section's heading, introduction, and first image together."""
    result: list[Flowable] = []
    index = 0
    while index < len(story):
        current = story[index]
        if isinstance(current, Paragraph) and current.style.name == "ManualH2":
            group: list[Flowable] = [current]
            cursor = index + 1
            saw_image = False
            while cursor < len(story) and len(group) < 7:
                candidate = story[cursor]
                if isinstance(candidate, Paragraph) and candidate.style.name.startswith("ManualH"):
                    break
                group.append(candidate)
                saw_image = saw_image or isinstance(candidate, Image)
                cursor += 1
                if saw_image and isinstance(candidate, Paragraph) and candidate.style.name == "ManualCaption":
                    break
            if saw_image:
                result.append(KeepTogether(group))
                index = cursor
                continue
        result.append(current)
        index += 1
    return result


class ManualDocTemplate(BaseDocTemplate):
    def __init__(self, filename: str, **kwargs):
        super().__init__(filename, **kwargs)
        cover_frame = Frame(20 * mm, 22 * mm, PAGE_WIDTH - 40 * mm, PAGE_HEIGHT - 44 * mm, id="cover", leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0)
        body_frame = Frame(20 * mm, 19 * mm, PAGE_WIDTH - 40 * mm, PAGE_HEIGHT - 36 * mm, id="body", leftPadding=0, rightPadding=0, topPadding=8 * mm, bottomPadding=7 * mm)
        self.addPageTemplates([
            PageTemplate(id="Cover", frames=[cover_frame], onPage=self.draw_cover),
            PageTemplate(id="Contents", frames=[body_frame], onPage=self.draw_contents),
            PageTemplate(id="Body", frames=[body_frame], onPage=self.draw_body),
        ])
        self.current_chapter = "Contents"

    def draw_cover(self, canvas, _doc):
        canvas.manualKeycap = draw_manual_keycap
        canvas.saveState()
        canvas.setFillColor(NAVY)
        canvas.rect(0, 0, PAGE_WIDTH, PAGE_HEIGHT, fill=1, stroke=0)
        canvas.setFillColor(TEAL)
        canvas.circle(PAGE_WIDTH - 15 * mm, PAGE_HEIGHT - 18 * mm, 48 * mm, fill=1, stroke=0)
        canvas.setFillColor(colors.HexColor("#0d3140"))
        canvas.circle(PAGE_WIDTH - 6 * mm, 32 * mm, 61 * mm, fill=1, stroke=0)
        canvas.setStrokeColor(CYAN)
        canvas.setLineWidth(0.7)
        for offset in range(0, 80, 8):
            canvas.line(0, (22 + offset) * mm, (70 + offset) * mm, 0)
        canvas.restoreState()

    def draw_body(self, canvas, doc):
        self.draw_running_page(canvas, doc, self.current_chapter)

    def draw_contents(self, canvas, doc):
        self.draw_running_page(canvas, doc, "Contents")

    def draw_running_page(self, canvas, doc, chapter):
        canvas.manualKeycap = draw_manual_keycap
        canvas.saveState()
        manual_page = max(1, doc.page - 1)
        canvas.setStrokeColor(GRID)
        canvas.setLineWidth(0.5)
        canvas.line(20 * mm, PAGE_HEIGHT - 15 * mm, PAGE_WIDTH - 20 * mm, PAGE_HEIGHT - 15 * mm)
        canvas.setFont(SANS_BOLD, 7)
        canvas.setFillColor(MUTED)
        canvas.drawString(20 * mm, PAGE_HEIGHT - 11.5 * mm, "TOSKLIGHT OPERATOR MANUAL")
        canvas.setFont(SANS, 7)
        canvas.drawRightString(PAGE_WIDTH - 20 * mm, PAGE_HEIGHT - 11.5 * mm, chapter[:68])
        canvas.setFillColor(NAVY)
        canvas.setFont(SANS_BOLD, 8)
        revision = f"ToskLight v{SOFTWARE_VERSION} - Operator Manual"
        if manual_page % 2:
            canvas.drawString(20 * mm, 10 * mm, revision)
            canvas.drawRightString(PAGE_WIDTH - 20 * mm, 10 * mm, str(manual_page))
        else:
            canvas.drawString(20 * mm, 10 * mm, str(manual_page))
            canvas.drawRightString(PAGE_WIDTH - 20 * mm, 10 * mm, revision)
        canvas.restoreState()

    def afterFlowable(self, flowable):
        if isinstance(flowable, ChapterMarker):
            self.current_chapter = flowable.title
            return
        if isinstance(flowable, Paragraph):
            style = flowable.style.name
            if style in {"ManualChapterH1", "ManualH1", "ManualH2", "ManualH3"}:
                text = getattr(flowable, "manual_plain_title", flowable.getPlainText())
                level = getattr(flowable, "manual_toc_level", {"ManualChapterH1": 0, "ManualH1": 1, "ManualH2": 2, "ManualH3": 2}[style])
                key = f"toc-{self.seq.nextf('heading')}"
                self.canv.bookmarkPage(key)
                self.canv.addOutlineEntry(text, key, level=level, closed=level > 0)
                if level <= 2:
                    toc_text = f"<u color='#12b8a6'>{html.escape(text)}</u>" if getattr(flowable, "manual_is_chapter", False) else text
                    self.notify("TOCEntry", (level, toc_text, max(1, self.page - 1), key))
                if level == 0:
                    self.current_chapter = text


class ChapterMarker(Flowable):
    def __init__(self, title: str):
        super().__init__()
        self.title = title

    def wrap(self, _available_width, _available_height):
        return 0, 0

    def draw(self):
        return None


def build(output: Path) -> None:
    pages = source_pages()
    validate_sources(pages)
    bookmarks = {page.relative: page.bookmark for page in pages}
    output.parent.mkdir(parents=True, exist_ok=True)
    doc = ManualDocTemplate(
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
    story: list[Flowable] = []
    logo = ROOT / "apps" / "control-ui" / "src-tauri" / "icons" / "icon.png"
    if logo.is_file():
        mark = Image(str(logo), width=38 * mm, height=38 * mm)
        mark.hAlign = "LEFT"
        story.extend([Spacer(1, 34 * mm), mark, Spacer(1, 19 * mm)])
    story.extend([
        Paragraph("ToskLight", STYLES["ManualCoverTitle"]),
        Paragraph(f"Operator manual / software v{SOFTWARE_VERSION}", STYLES["ManualCoverSub"]),
        Spacer(1, 83 * mm),
        Paragraph("DESK SETUP  /  SHOW SETUP  /  PROGRAMMING  /  RUNNING A SHOW", ParagraphStyle(name="CoverKicker", fontName=SANS_BOLD, fontSize=7.5, leading=11, textColor=colors.white)),
        NextPageTemplate("Contents"),
        PageBreak(),
        Paragraph("Contents", STYLES["ManualContentsTitle"]),
    ])
    toc = TableOfContents()
    toc.levelStyles = [
        ParagraphStyle(name="TOC1", fontName=SANS_BOLD, fontSize=11, leading=15, leftIndent=0, firstLineIndent=0, textColor=colors.HexColor("#0f766e"), spaceBefore=7, spaceAfter=2),
        ParagraphStyle(name="TOC2", fontName=SANS, fontSize=8.4, leading=11.5, leftIndent=11, firstLineIndent=0, textColor=INK),
        ParagraphStyle(name="TOC3", fontName=SANS, fontSize=7.8, leading=10.5, leftIndent=22, firstLineIndent=0, textColor=MUTED),
    ]
    story.append(toc)
    for page_index, page in enumerate(pages):
        story.extend([ChapterMarker(page.title), NextPageTemplate("Body"), PageBreak()])
        story.extend(markdown_story(page, bookmarks))
    story.extend([ChapterMarker("Index"), PageBreak(), Paragraph("Index", STYLES["ManualContentsTitle"]), Paragraph("Topics and feature names are indexed from the headings in the Markdown source.", STYLES["ManualBody"])])
    index = SimpleIndex(dot=" . ", headers=False)
    index.textStyle = ParagraphStyle(name="IndexText", fontName=SANS, fontSize=8, leading=11, textColor=INK)
    story.append(index)
    doc.multiBuild(story, canvasmaker=index.getCanvasMaker())
    print(f"Built {output} from {len(pages)} Markdown pages")


def main() -> int:
    global KEYCAP_ASSET_DIR
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    KEYCAP_ASSET_DIR = Path(os.environ.get("LIGHT_MANUAL_KEYCAP_DIR", args.output.resolve().parent / ".manual-keycaps"))
    try:
        build(args.output.resolve())
    except Exception as error:
        print(f"manual build failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
