"""Translate Help Markdown into ReportLab flowables."""

from __future__ import annotations

import html
import re
from pathlib import Path

from markdown_it import MarkdownIt
from reportlab.lib import colors
from reportlab.lib.units import mm
from reportlab.platypus import (
    Flowable,
    HRFlowable,
    Image,
    KeepTogether,
    ListFlowable,
    ListItem,
    PageBreak,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
)

from .config import (
    GRID,
    HELP,
    MONO,
    NAVY,
    PAGE_WIDTH,
    ROOT,
    SANS_BOLD,
    STYLES,
    TEAL,
)
from .keycaps import desk_keycap_markup, keycap_markup
from .source import SourcePage, slug


def render_link(match: re.Match[str], page: SourcePage, bookmarks: dict[str, str]) -> str:
    label, target = match.group(1), html.unescape(match.group(2))
    if target.startswith(("http://", "https://", "mailto:")):
        escaped = html.escape(target, quote=True)
        return f'<link href="{escaped}" color="#087f8c">{label}</link>'
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


def inline_markup(text: str, page: SourcePage, bookmarks: dict[str, str]) -> str:
    text = html.escape(text.strip(), quote=False)
    text = re.sub(r"`([^`]+)`", lambda match: f'<font name="{MONO}" color="#0f766e">{match.group(1)}</font>', text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<i>\1</i>", text)
    text = re.sub(
        r"\[([^]]+)\]\(([^)\s]+)(?:\s+[^)]*)?\)",
        lambda match: render_link(match, page, bookmarks),
        text,
    )
    text = re.sub(
        r"\[KBD:([^\]\n]+)\]",
        lambda match: keycap_markup(match.group(1).strip(), "keyboard"),
        text,
    )
    text = re.sub(
        r"\[\s*([+\-−^.]|[A-Z0-9.][A-Z0-9._ ←-]*[+*]?)\s*\]",
        lambda match: desk_keycap_markup(match.group(1)),
        text,
    )
    return text.replace("<br>", "<br/>").replace("&lt;br&gt;", "<br/>")


def paragraph(
    text: str,
    style: str,
    page: SourcePage,
    bookmarks: dict[str, str],
) -> Paragraph:
    return Paragraph(inline_markup(text, page, bookmarks), STYLES[style])


def parse_table(lines: list[str], page: SourcePage, bookmarks: dict[str, str]) -> Table:
    rows: list[list[Paragraph]] = []
    separator = r"\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*"
    for row_index, line in enumerate(lines):
        if row_index == 1 and re.fullmatch(separator, line):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        style = "ManualTableHead" if not rows else "ManualTable"
        rows.append([
            paragraph(cell.replace("\\|", "|"), style, page, bookmarks)
            for cell in cells
        ])
    columns = max(len(row) for row in rows)
    for row in rows:
        row.extend([Paragraph("", STYLES["ManualTable"])] * (columns - len(row)))
    available = PAGE_WIDTH - 40 * mm
    table = Table(rows, colWidths=[available / columns] * columns, repeatRows=1, hAlign="LEFT")
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


class StoryParser:
    def __init__(self, page: SourcePage, bookmarks: dict[str, str]):
        self.page = page
        self.bookmarks = bookmarks
        self.lines = page.markdown.splitlines()
        self.story: list[Flowable] = []
        self.index = 0
        self.first_paragraph = True

    @property
    def raw(self) -> str:
        return self.lines[self.index]

    @property
    def stripped(self) -> str:
        return self.raw.strip()

    def parse(self) -> list[Flowable]:
        while self.index < len(self.lines):
            if not self.stripped:
                self.index += 1
            elif not (
                self.image()
                or self.heading()
                or self.code()
                or self.rule()
                or self.table()
                or self.listing()
                or self.quote()
            ):
                self.body_paragraph()
        return keep_section_openers_with_images(self.story)

    def image(self) -> bool:
        match = re.fullmatch(r"!\[([^]]*)\]\(([^)\s]+)(?:\s+[^)]*)?\)", self.stripped)
        if not match:
            return False
        source = (self.page.path.parent / match.group(2)).resolve()
        self.story.extend(image_flowable(source, match.group(1)))
        self.index += 1
        return True

    def heading(self) -> bool:
        match = re.match(r"^(#{1,4})\s+(.+?)\s*$", self.stripped)
        if not match:
            return False
        level = len(match.group(1))
        title = match.group(2)
        bookmark = self.page.bookmark if level == 1 else f"{self.page.bookmark}-{slug(title)}"
        style = "ManualChapterH1" if level == 1 and self.page.is_chapter else f"ManualH{level}"
        rendered = inline_markup(title, self.page, self.bookmarks)
        if level == 1 and self.page.is_chapter:
            rendered = f'<u color="#5ee7f0">{rendered}</u>'
        flowable = Paragraph(
            f'<a name="{bookmark}"/><index item="{html.escape(title, quote=True)}"/>{rendered}',
            STYLES[style],
        )
        self.configure_heading(flowable, level, title)
        self.story.append(flowable)
        if level == 1 and self.page.is_chapter:
            self.story.append(HRFlowable(
                width="100%", thickness=1.2, color=TEAL, spaceBefore=0, spaceAfter=11,
            ))
        self.first_paragraph = level == 1
        self.index += 1
        return True

    def configure_heading(self, flowable: Paragraph, level: int, title: str) -> None:
        if level == 1:
            flowable.manual_toc_level = 0 if self.page.is_chapter else 1
        else:
            flowable.manual_toc_level = min(level - (1 if self.page.is_chapter else 0), 2)
        flowable.manual_plain_title = title
        flowable.manual_is_chapter = level == 1 and self.page.is_chapter

    def code(self) -> bool:
        if not self.stripped.startswith("```"):
            return False
        language = self.stripped[3:].strip()
        body: list[str] = []
        self.index += 1
        while self.index < len(self.lines) and not self.lines[self.index].strip().startswith("```"):
            body.append(self.lines[self.index])
            self.index += 1
        self.index += 1
        label = f"{language}\n" if language else ""
        markup = (html.escape(label + "\n".join(body)) or " ").replace("\n", "<br/>")
        self.story.append(Paragraph(markup, STYLES["ManualCode"]))
        return True

    def rule(self) -> bool:
        if self.stripped != "---":
            return False
        rule = Table([[""]], colWidths=[PAGE_WIDTH - 40 * mm], rowHeights=[1])
        rule.setStyle(TableStyle([("BACKGROUND", (0, 0), (-1, -1), GRID)]))
        self.story.extend([Spacer(1, 5), rule, Spacer(1, 5)])
        self.index += 1
        return True

    def table(self) -> bool:
        if not self.is_table_start():
            return False
        table_lines = [self.raw, self.lines[self.index + 1]]
        self.index += 2
        while self.index < len(self.lines) and "|" in self.lines[self.index] and self.lines[self.index].strip():
            table_lines.append(self.lines[self.index])
            self.index += 1
        self.story.extend([parse_table(table_lines, self.page, self.bookmarks), Spacer(1, 7)])
        return True

    def is_table_start(self) -> bool:
        return (
            "|" in self.stripped
            and self.index + 1 < len(self.lines)
            and re.match(r"^\s*\|?\s*:?-+", self.lines[self.index + 1]) is not None
        )

    def listing(self) -> bool:
        match = re.match(r"^\s*(?:[-*+]\s+|(\d+)\.\s+)(.+)$", self.raw)
        if not match:
            return False
        ordered = match.group(1) is not None
        items: list[ListItem] = []
        while self.index < len(self.lines):
            item = re.match(r"^\s*(?:[-*+]\s+|(\d+)\.\s+)(.+)$", self.lines[self.index])
            if not item or (item.group(1) is not None) != ordered:
                break
            value = int(item.group(1)) if item.group(1) else None
            content = paragraph(item.group(2), "ManualBody", self.page, self.bookmarks)
            items.append(ListItem(content, leftIndent=13, value=value))
            self.index += 1
        options = self.list_options(ordered)
        listing = ListFlowable(items, **options)
        self.story.append(KeepTogether([listing]) if len(items) <= 5 else listing)
        return True

    @staticmethod
    def list_options(ordered: bool) -> dict[str, object]:
        options: dict[str, object] = {
            "bulletType": "1" if ordered else "bullet",
            "leftIndent": 18,
            "bulletFontName": SANS_BOLD,
            "bulletFontSize": 7,
            "spaceAfter": 7,
        }
        if ordered:
            options["start"] = "1"
        return options

    def quote(self) -> bool:
        if not self.stripped.startswith(">"):
            return False
        lines: list[str] = []
        while self.index < len(self.lines) and self.lines[self.index].strip().startswith(">"):
            lines.append(self.lines[self.index].strip()[1:].strip())
            self.index += 1
        text = " ".join(lines)
        if re.sub(r"[*_]", "", text).strip().lower() == "dynamics is a future feature.":
            self.story.extend([
                PageBreak(),
                Spacer(1, 88 * mm),
                paragraph("Dynamics is a future feature.", "ManualFutureFeature", self.page, self.bookmarks),
            ])
        else:
            self.story.append(paragraph(text, "ManualQuote", self.page, self.bookmarks))
        return True

    def body_paragraph(self) -> None:
        block = [self.stripped]
        self.index += 1
        while self.index < len(self.lines) and self.lines[self.index].strip():
            candidate = self.lines[self.index].strip()
            if self.starts_new_block(candidate):
                break
            block.append(candidate)
            self.index += 1
        style = "ManualLead" if self.first_paragraph else "ManualBody"
        self.first_paragraph = False
        self.story.append(paragraph(" ".join(block), style, self.page, self.bookmarks))

    def starts_new_block(self, candidate: str) -> bool:
        if re.match(r"^(#{1,4})\s+", candidate) or candidate.startswith("```"):
            return True
        if re.match(r"^(?:[-*+]\s+|\d+\.\s+)", candidate):
            return True
        return self.is_table_start()


def markdown_story(page: SourcePage, bookmarks: dict[str, str]) -> list[Flowable]:
    MarkdownIt("commonmark", {"html": False}).enable("table").parse(page.markdown)
    return StoryParser(page, bookmarks).parse()


def keep_section_openers_with_images(story: list[Flowable]) -> list[Flowable]:
    """Keep a screenshot section's heading, introduction, and first image together."""
    result: list[Flowable] = []
    index = 0
    while index < len(story):
        current = story[index]
        if isinstance(current, Paragraph) and current.style.name == "ManualH2":
            group, cursor, saw_image = section_opener(story, index)
            if saw_image:
                result.append(KeepTogether(group))
                index = cursor
                continue
        result.append(current)
        index += 1
    return result


def section_opener(story: list[Flowable], index: int) -> tuple[list[Flowable], int, bool]:
    group: list[Flowable] = [story[index]]
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
    return group, cursor, saw_image
