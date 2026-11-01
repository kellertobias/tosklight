"""Generate our outline-only test font; requires fontTools, never used by production."""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

builder = FontBuilder(1000, isTTF=True)
builder.setupGlyphOrder([".notdef", "symbol"])
builder.setupCharacterMap({0x2605: "symbol"})
empty = TTGlyphPen(None).glyph()
pen = TTGlyphPen(None)
pen.moveTo((100, 100))
pen.lineTo((700, 100))
pen.lineTo((700, 700))
pen.lineTo((100, 700))
pen.closePath()
builder.setupGlyf({".notdef": empty, "symbol": pen.glyph()})
builder.setupHorizontalMetrics({".notdef": (800, 0), "symbol": (800, 100)})
builder.setupHorizontalHeader(ascent=800, descent=-200)
builder.setupNameTable({"familyName": "Fallback Symbol Fixture", "styleName": "Regular",
                       "uniqueFontIdentifier": "ToskLightFallbackSymbolFixture",
                       "fullName": "Fallback Symbol Fixture Regular",
                       "psName": "FallbackSymbolFixture-Regular"})
builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
builder.setupPost()
builder.setupMaxp()
builder.font.recalcTimestamp = False
builder.font["head"].created = 2082844800
builder.font["head"].modified = 2082844800
builder.save(Path(__file__).with_name("symbol-only.ttf"))
