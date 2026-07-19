"""ReportLab page templates, running furniture, and PDF outline handling."""

from __future__ import annotations

import html

from reportlab.lib import colors
from reportlab.lib.units import mm
from reportlab.platypus import (
    BaseDocTemplate,
    Flowable,
    Frame,
    PageTemplate,
    Paragraph,
)

from .config import (
    CYAN,
    GRID,
    MUTED,
    NAVY,
    PAGE_HEIGHT,
    PAGE_WIDTH,
    SANS,
    SANS_BOLD,
    SOFTWARE_VERSION,
    TEAL,
)
from .keycaps import draw_manual_keycap


def page_frames() -> tuple[Frame, Frame]:
    cover = Frame(
        20 * mm,
        22 * mm,
        PAGE_WIDTH - 40 * mm,
        PAGE_HEIGHT - 44 * mm,
        id="cover",
        leftPadding=0,
        rightPadding=0,
        topPadding=0,
        bottomPadding=0,
    )
    body = Frame(
        20 * mm,
        19 * mm,
        PAGE_WIDTH - 40 * mm,
        PAGE_HEIGHT - 36 * mm,
        id="body",
        leftPadding=0,
        rightPadding=0,
        topPadding=8 * mm,
        bottomPadding=7 * mm,
    )
    return cover, body


class ManualDocTemplate(BaseDocTemplate):
    def __init__(self, filename: str, **kwargs):
        super().__init__(filename, **kwargs)
        cover, body = page_frames()
        self.addPageTemplates([
            PageTemplate(id="Cover", frames=[cover], onPage=self.draw_cover),
            PageTemplate(id="Contents", frames=[body], onPage=self.draw_contents),
            PageTemplate(id="Body", frames=[body], onPage=self.draw_body),
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
        self.draw_header(canvas, chapter)
        self.draw_footer(canvas, manual_page)
        canvas.restoreState()

    @staticmethod
    def draw_header(canvas, chapter: str) -> None:
        canvas.setStrokeColor(GRID)
        canvas.setLineWidth(0.5)
        canvas.line(20 * mm, PAGE_HEIGHT - 15 * mm, PAGE_WIDTH - 20 * mm, PAGE_HEIGHT - 15 * mm)
        canvas.setFont(SANS_BOLD, 7)
        canvas.setFillColor(MUTED)
        canvas.drawString(20 * mm, PAGE_HEIGHT - 11.5 * mm, "TOSKLIGHT OPERATOR MANUAL")
        canvas.setFont(SANS, 7)
        canvas.drawRightString(PAGE_WIDTH - 20 * mm, PAGE_HEIGHT - 11.5 * mm, chapter[:68])

    @staticmethod
    def draw_footer(canvas, manual_page: int) -> None:
        canvas.setFillColor(NAVY)
        canvas.setFont(SANS_BOLD, 8)
        revision = f"ToskLight v{SOFTWARE_VERSION} - Operator Manual"
        left, right = (revision, str(manual_page)) if manual_page % 2 else (str(manual_page), revision)
        canvas.drawString(20 * mm, 10 * mm, left)
        canvas.drawRightString(PAGE_WIDTH - 20 * mm, 10 * mm, right)

    def afterFlowable(self, flowable):
        if isinstance(flowable, ChapterMarker):
            self.current_chapter = flowable.title
            return
        if not isinstance(flowable, Paragraph):
            return
        style = flowable.style.name
        if style not in {"ManualChapterH1", "ManualH1", "ManualH2", "ManualH3"}:
            return
        self.record_heading(flowable, style)

    def record_heading(self, flowable: Paragraph, style: str) -> None:
        text = getattr(flowable, "manual_plain_title", flowable.getPlainText())
        default_levels = {"ManualChapterH1": 0, "ManualH1": 1, "ManualH2": 2, "ManualH3": 2}
        level = getattr(flowable, "manual_toc_level", default_levels[style])
        key = f"toc-{self.seq.nextf('heading')}"
        self.canv.bookmarkPage(key)
        self.canv.addOutlineEntry(text, key, level=level, closed=level > 0)
        if level <= 2:
            toc_text = (
                f"<u color='#12b8a6'>{html.escape(text)}</u>"
                if getattr(flowable, "manual_is_chapter", False)
                else text
            )
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
