#!/usr/bin/env python3
"""Add the space to the bundled base-14 subsets in defaultfonts/ (#288).

The subsets were cut without a glyph for U+0020 (and U+00A0), so the HTML renderer
fell back to a system font for every space: two text objects per word, and a
machine-dependent second font in the PDF. A space has no outline, so it can be
added without any new font data: an empty glyph `space`, appended (existing glyph
ids keep their numbers), with the standard base-14 advance width, mapped from U+0020
and U+00A0.

Idempotent; writes gzip with mtime 0 so the result is reproducible.

    pip install fonttools && python3 scripts/add_space_glyphs.py
"""

import gzip
import io
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph

# Advance of the space in the base-14 AFM metrics, in 1/1000 em.
SPACE_WIDTH = {"Helvetica": 278, "Times": 250, "Courier": 600, "Symbol": 250, "ZapfDingbats": 278}

FONTS = Path(__file__).resolve().parent.parent / "defaultfonts"


def add_space(path: Path) -> bool:
    font = TTFont(io.BytesIO(gzip.decompress(path.read_bytes())))
    if "space" in font.getGlyphOrder():
        return False
    family = next(k for k in SPACE_WIDTH if path.name.startswith(k))
    advance = round(SPACE_WIDTH[family] * font["head"].unitsPerEm / 1000)

    order = font.getGlyphOrder() + ["space"]
    font.setGlyphOrder(order)
    glyph = Glyph()
    glyph.numberOfContours = 0
    font["glyf"].glyphs["space"] = glyph
    font["glyf"].glyphOrder = order
    font["hmtx"].metrics["space"] = (advance, 0)
    for table in font["cmap"].tables:
        if table.isUnicode():
            table.cmap[0x20] = "space"
            table.cmap[0xA0] = "space"
    if "post" in font and font["post"].formatType == 2.0:
        font["post"].extraNames = getattr(font["post"], "extraNames", [])

    out = io.BytesIO()
    font.save(out)
    path.write_bytes(gzip.compress(out.getvalue(), mtime=0))
    return True


if __name__ == "__main__":
    for path in sorted(FONTS.glob("*.subset.ttf")):
        print(f"{path.name:36} {'space added' if add_space(path) else 'has a space already'}")
