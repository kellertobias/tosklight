"""Render and embed the desk keycaps used in PDF paragraphs."""

from __future__ import annotations

import hashlib
import html
import re
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote

from PIL import Image as PILImage, ImageDraw, ImageFont
from reportlab.lib import colors
from reportlab.pdfbase import pdfmetrics

from .config import DEFAULT_OUTPUT, MONO, SANS_BOLD

KEYCAP_PALETTES = {
    "number": ("#171c22", "#3a4652", "#090c10", "#edf3f6"),
    "command": ("#171c22", "#3a4652", "#090c10", "#ffb30f"),
    "clear": ("#261d08", "#d6a600", "#806000", "#f0c52f"),
    "record": ("#21090c", "#ff6872", "#70181f", "#ff6872"),
    "keyboard": ("#ffffff", "#cbd5e1", "#7d8b94", "#17202a"),
}

_asset_directory = DEFAULT_OUTPUT.parent / ".manual-keycaps"


def configure_asset_directory(path: Path) -> None:
    global _asset_directory
    _asset_directory = path


@dataclass(frozen=True)
class KeycapGeometry:
    scale: int
    horizontal_gap: int
    text_width: int
    text_height: int
    key_width: int
    width: int
    height: int
    shadow: int
    text_box: tuple[int, int, int, int]


def keycap_font(category: str, scale: int):
    path = (
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
        if category == "keyboard"
        else "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
    )
    if not Path(path).is_file():
        path = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
    return ImageFont.truetype(path, round(7.4 * scale))


def keycap_geometry(label: str, font, scale: int) -> KeycapGeometry:
    horizontal_gap = round(1.4 * scale)
    box = ImageDraw.Draw(PILImage.new("RGBA", (1, 1))).textbbox((0, 0), label, font=font)
    text_width = box[2] - box[0]
    text_height = box[3] - box[1]
    key_width = max(round(15 * scale), text_width + round(7 * scale))
    height = round(11.2 * scale)
    return KeycapGeometry(
        scale=scale,
        horizontal_gap=horizontal_gap,
        text_width=text_width,
        text_height=text_height,
        key_width=key_width,
        width=key_width + horizontal_gap * 2,
        height=height,
        shadow=round(1.2 * scale),
        text_box=box,
    )


def draw_keycap_image(label: str, category: str, font, geometry: KeycapGeometry) -> PILImage.Image:
    fill, border, shadow_color, text_color = KEYCAP_PALETTES[category]
    image = PILImage.new(
        "RGBA",
        (geometry.width, geometry.height + geometry.shadow),
        (255, 255, 255, 0),
    )
    draw = ImageDraw.Draw(image)
    radius = round(2.8 * geometry.scale)
    left = geometry.horizontal_gap
    right = left + geometry.key_width - 1
    draw.rounded_rectangle(
        (left, geometry.shadow, right, geometry.height + geometry.shadow - 1),
        radius=radius,
        fill=shadow_color,
    )
    draw.rounded_rectangle(
        (left, 0, right, geometry.height - 1),
        radius=radius,
        fill=fill,
        outline=border,
        width=max(1, geometry.scale),
    )
    x = left + (geometry.key_width - geometry.text_width) / 2 - geometry.text_box[0]
    y = (geometry.height - geometry.text_height) / 2 - geometry.text_box[1]
    draw.text((x, y), label, font=font, fill=text_color)
    return image


def keycap_asset(label: str, category: str) -> tuple[Path, float, float]:
    """Render an indivisible high-resolution inline keycap for Paragraph layout."""
    font = keycap_font(category, 4)
    geometry = keycap_geometry(label, font, 4)
    digest = hashlib.sha1(f"spaced-v2\0{category}\0{label}".encode()).hexdigest()[:12]
    path = _asset_directory / f"{category}-{digest}.png"
    if not path.is_file():
        _asset_directory.mkdir(parents=True, exist_ok=True)
        draw_keycap_image(label, category, font, geometry).save(path)
    return path, geometry.width / geometry.scale, (geometry.height + geometry.shadow) / geometry.scale


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
    escaped = html.escape(str(path), quote=True)
    return f'<img src="{escaped}" width="{width:.2f}" height="{height:.2f}" valign="-3"/>'


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
    fill, border, shadow, _text = KEYCAP_PALETTES.get(
        category,
        KEYCAP_PALETTES["command"],
    )
    font = MONO if category == "keyboard" else SANS_BOLD
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
