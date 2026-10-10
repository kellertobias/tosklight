#!/usr/bin/env python3
"""Append a caption footer to release evidence without resizing screenshot pixels.

Requires installed Pillow; installs nothing. Sequential use only (one evidence writer).
--evidence-root is required below canonical LIGHT_TEST_VISUAL_DIR/TEST_VISUAL;
LIGHT_ARTIFACTS_DIR overrides are honored. PNG goes into steps/ and steps.jsonl
records UTC caption time, source dimensions, hashes and all explicit observations.
Existing IDs and image paths are refused, preserving earlier evidence.

python3 -B tests/bench/release-caption-step.py --raw RAW.png \
 --evidence-root .artifacts/test/visual-inspection/release-acceptance/RUN \
 --id CASE-001 --status FAIL --intention 'Create an empty show' \
 --expected 'New independent show opens' --actual 'Recovery modal remains'

Caption time is not the capture time; provide capture time separately in the run
manifest. PASS means only the stated expected outcome, not whole-case acceptance.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from artifact_paths import artifact_path


def wrapped(draw, text, font, width):
    """Pixel-based wrap, including long paths; explicit newlines are preserved."""
    lines = []
    for paragraph in text.splitlines() or [""]:
        line = ""
        for character in paragraph:
            if line and draw.textlength(line + character, font=font) > width:
                split = line.rfind(" ")
                if split > 0:
                    lines.append(line[:split])
                    line = line[split + 1:] + character
                else:
                    lines.append(line)
                    line = character
            else:
                line += character
        lines.append(line)
    return lines


def caption(raw, destination, fields):
    with Image.open(raw) as original:
        screenshot = original.convert("RGBA")
    width, height = screenshot.size
    size = max(16, min(40, round(width / 60)))
    padding = size
    font = ImageFont.load_default(size=size)
    measure = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    rows = []
    for label, text in (("Step", f"{fields['id']} | {fields['status']}"),
                        ("Intention", fields["intention"]), ("Expected", fields["expected"]),
                        ("Actual", fields["actual"])):
        rows.extend(wrapped(measure, f"{label}: {text}", font, width - padding * 2))
        rows.append("")
    line_height = round(size * 1.5)
    canvas = Image.new("RGBA", (width, height + padding * 2 + line_height * len(rows)), "#111820")
    canvas.paste(screenshot, (0, 0))
    drawing = ImageDraw.Draw(canvas)
    for index, text in enumerate(rows):
        drawing.text((padding, height + padding + index * line_height), text, font=font, fill="#f4f7fc")
    canvas.save(destination, format="PNG")
    with Image.open(destination) as saved:
        if saved.crop((0, 0, width, height)).convert("RGBA").tobytes() != screenshot.tobytes():
            raise ValueError("source screenshot pixels changed")
    return [width, height], list(canvas.size)


def append_step(root, raw, fields):
    canonical = artifact_path("LIGHT_TEST_VISUAL_DIR", "TEST_VISUAL").resolve()
    root = Path(root).expanduser().resolve()
    if root == canonical or not root.is_relative_to(canonical):
        raise ValueError(f"evidence root must be below {canonical}")
    if not re.fullmatch(r"[A-Za-z0-9_-]+", fields["id"]):
        raise ValueError("ID must contain only letters, digits, underscores and hyphens")
    if any(not str(fields.get(key, "")).strip() for key in ("intention", "expected", "actual")):
        raise ValueError("intention, expected and actual must be explicit nonempty text")
    if fields["status"] not in {"PASS", "FAIL", "BLOCKED", "NOT_TESTED", "OBSERVATION", "RUNNING"}:
        raise ValueError("invalid status")
    ledger = root / "steps.jsonl"
    if ledger.exists():
        for line in ledger.read_text().splitlines():
            if json.loads(line)["id"] == fields["id"]:
                raise ValueError(f"step ID already recorded: {fields['id']}")
    destination = root / "steps" / f"{fields['id']}.png"
    if destination.exists():
        raise ValueError(f"captioned image already exists: {destination}")
    raw = Path(raw).expanduser().resolve(strict=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    pixels, output_pixels = caption(raw, destination, fields)
    record = dict(fields, raw=str(raw), captioned=str(destination), source_pixels=pixels,
                  captioned_pixels=output_pixels, captioned_at_utc=datetime.now(timezone.utc).isoformat(),
                  raw_sha256=hashlib.sha256(raw.read_bytes()).hexdigest(),
                  captioned_sha256=hashlib.sha256(destination.read_bytes()).hexdigest())
    with ledger.open("a") as stream:
        stream.write(json.dumps(record) + "\n")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--raw", required=True)
    parser.add_argument("--evidence-root", required=True)
    for name in ("id", "intention", "expected", "actual", "status"):
        parser.add_argument(f"--{name}", required=True)
    args = parser.parse_args()
    try:
        record = append_step(args.evidence_root, args.raw, {key: getattr(args, key) for key in ("id", "intention", "expected", "actual", "status")})
    except (ValueError, OSError) as exc:
        parser.error(str(exc))
    print(json.dumps(record))


if __name__ == "__main__":
    main()
