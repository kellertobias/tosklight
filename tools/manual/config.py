"""Shared paths, typography, palette, and styles for the PDF manual."""

from __future__ import annotations

import os
import re
from pathlib import Path

from artifact_paths import artifact_path
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

ROOT = Path(__file__).resolve().parents[2]
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
        if not all(Path(path).is_file() for path in (regular, bold, italic, mono)):
            continue
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
    match = re.search(
        r"\[workspace\.package\][\s\S]*?^version\s*=\s*\"([^\"]+)\"",
        cargo,
        re.MULTILINE,
    )
    return match.group(1) if match else "development"


SOFTWARE_VERSION = workspace_version()
