#!/usr/bin/env python3
"""Generate the deterministic mock fonts under tests/assets/fonts/mock/.

Three fonts with IDENTICAL, exactly-defined metrics (units_per_em = 1000) and
per-glyph DISTINCT advance widths, so tests can verify glyph identity and text
layout math against constants instead of against another font parser:

  glyph   char   advance   outline
  .notdef  —       500     empty
  space   ' '      250     empty
  A..J    A..J   300+50*i  rectangle of that exact width (i = 0 for A)

- mock_ttf.ttf        TrueType (glyf) — embeds as CIDFontType2 + /FontFile2
- mock_cff_named.otf  name-keyed CFF  — embeds as CIDFontType0 + /FontFile3,
                      viewers use CID == GID
- mock_cff_cid.otf    CID-keyed CFF (ROS Adobe-Identity-0) with a charset that
                      is deliberately NOT identity and NOT monotonic in gid:

                        gid:  0(.notdef) 1(space) 2(A) 3(B) 4(C) ... 11(J)
                        CID:  0          700      901  811  821 ... 891

                      This is the #280 regression shape: a spec-following
                      viewer resolves each Identity-H code as a CID *through
                      this charset* (ISO 32000-1 9.7.4.2), so a producer that
                      writes glyph ids as codes shows wrong/missing glyphs in
                      Acrobat/Preview while PDFium (code == GID fallback)
                      looks fine. A is out of ascending order on purpose, to
                      exercise /W and ToUnicode code sorting.

Regenerate with:  python3 scripts/gen_mock_fonts.py   (needs: pip install fonttools)
The outputs are committed — tests do not run this script.
"""
import os

# Deterministic output: fontTools honors SOURCE_DATE_EPOCH for the head table's
# created/modified timestamps. CI regenerates the fonts and diffs them against
# the committed binaries, so generation must be byte-reproducible.
os.environ.setdefault('SOURCE_DATE_EPOCH', '0')

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.cffLib import (
    FDArrayIndex,
    FDSelect,
    FontDict,
    PrivateDict,
)

UPM = 1000
LETTERS = [chr(ord('A') + i) for i in range(10)]  # A..J
ADVANCES = {'.notdef': 500, 'space': 250}
ADVANCES.update({ch: 300 + 50 * i for i, ch in enumerate(LETTERS)})
# gid order: .notdef, space, A..J
GLYPH_ORDER = ['.notdef', 'space'] + LETTERS
# Non-identity, non-monotonic CIDs for the CID-keyed font (gid -> CID)
CIDS = [0, 700, 901, 811, 821, 831, 841, 851, 861, 871, 881, 891]

OUT_DIR = os.path.join(os.path.dirname(__file__), '..', 'tests', 'assets', 'fonts', 'mock')


def draw_rect(pen, width):
    """A filled rectangle spanning the glyph's exact advance width."""
    pen.moveTo((0, 0))
    pen.lineTo((width, 0))
    pen.lineTo((width, 700))
    pen.lineTo((0, 700))
    pen.closePath()


def cmap():
    m = {0x20: 'space'}
    m.update({ord(ch): ch for ch in LETTERS})
    return m


def common_setup(fb, glyph_order):
    fb.setupGlyphOrder(glyph_order)
    fb.setupCharacterMap({cp: n for cp, n in cmap().items()
                          if n in glyph_order or n in ('space',)})
    fb.setupHorizontalMetrics({n: (ADVANCES[base_name(n)], 0) for n in glyph_order})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({'familyName': 'MockFont', 'styleName': 'Regular'})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost()


def base_name(glyph_name):
    """ADVANCES key for a glyph name ('cidNNNNN' aliases resolve by position)."""
    return glyph_name


def build_ttf(path):
    fb = FontBuilder(UPM, isTTF=True)
    glyphs = {}
    for name in GLYPH_ORDER:
        pen = TTGlyphPen(None)
        if name not in ('.notdef', 'space'):
            draw_rect(pen, ADVANCES[name])
        glyphs[name] = pen.glyph()
    common_setup(fb, GLYPH_ORDER)
    fb.setupGlyf(glyphs)
    fb.font.save(path)


def build_cff_named(path):
    fb = FontBuilder(UPM, isTTF=False)
    charstrings = {}
    for name in GLYPH_ORDER:
        pen = T2CharStringPen(ADVANCES[name], None)
        if name not in ('.notdef', 'space'):
            draw_rect(pen, ADVANCES[name])
        charstrings[name] = pen.getCharString()
    common_setup(fb, GLYPH_ORDER)
    fb.setupCFF('MockFont-Regular', {'FullName': 'MockFont Regular'}, charstrings, {})
    fb.font.save(path)


def build_cff_cid(path):
    """Name-keyed build, then in-place conversion to a CID-keyed CFF whose
    charset maps gid i -> CIDS[i]."""
    fb = FontBuilder(UPM, isTTF=False)
    charstrings = {}
    for name in GLYPH_ORDER:
        pen = T2CharStringPen(ADVANCES[name], None)
        if name not in ('.notdef', 'space'):
            draw_rect(pen, ADVANCES[name])
        charstrings[name] = pen.getCharString()
    common_setup(fb, GLYPH_ORDER)
    fb.setupCFF('MockFont-CID', {'FullName': 'MockFont CID'}, charstrings, {})

    font = fb.font
    cff = font['CFF '].cff
    td = cff[cff.fontNames[0]]

    # Rename every glyph to its CID spelling; the cffLib compiler writes a
    # CID-keyed charset from these names once ROS is set.
    rename = {GLYPH_ORDER[gid]: ('.notdef' if gid == 0 else 'cid%05d' % CIDS[gid])
              for gid in range(len(GLYPH_ORDER))}
    new_order = [rename[n] for n in GLYPH_ORDER]

    cs = td.CharStrings
    for old, new in rename.items():
        if old == new:
            continue
        cs.charStrings[new] = cs.charStrings.pop(old)
    td.charset = new_order

    # TTFont-level glyph order + cmap must use the same names.
    font.setGlyphOrder(new_order)
    if hasattr(font, '_reverseGlyphOrderDict'):
        del font._reverseGlyphOrderDict
    for table in font['cmap'].tables:
        table.cmap = {cp: rename.get(n, n) for cp, n in table.cmap.items()}
    hmtx = font['hmtx']
    hmtx.metrics = {rename.get(n, n): v for n, v in hmtx.metrics.items()}

    # CID-keyed structure: ROS + FDArray/FDSelect; private dict moves into the FD.
    td.ROS = ('Adobe', 'Identity', 0)
    td.rawDict['ROS'] = td.ROS
    td.CIDCount = max(CIDS) + 1
    fd = FontDict()
    fd.Private = td.Private
    fd_array = FDArrayIndex()
    fd_array.append(fd)
    td.FDArray = fd_array
    fd_select = FDSelect()
    fd_select.format = 3
    fd_select.gidArray = [0] * len(new_order)
    td.FDSelect = fd_select
    td.rawDict.pop('Private', None)
    if hasattr(td, 'Private'):
        del td.Private
    # CID fonts have no encoding.
    td.rawDict.pop('Encoding', None)
    if hasattr(td, 'Encoding'):
        td.Encoding = None

    font.save(path)


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    build_ttf(os.path.join(OUT_DIR, 'mock_ttf.ttf'))
    build_cff_named(os.path.join(OUT_DIR, 'mock_cff_named.otf'))
    build_cff_cid(os.path.join(OUT_DIR, 'mock_cff_cid.otf'))

    # Self-check: reload and assert the invariants the tests rely on.
    from fontTools.ttLib import TTFont
    for fname in ('mock_ttf.ttf', 'mock_cff_named.otf', 'mock_cff_cid.otf'):
        p = os.path.join(OUT_DIR, fname)
        f = TTFont(p)
        assert f['head'].unitsPerEm == UPM
        order = f.getGlyphOrder()
        assert len(order) == len(GLYPH_ORDER), (fname, order)
        hmtx = f['hmtx']
        for gid, name in enumerate(order):
            expected = ADVANCES[GLYPH_ORDER[gid]]
            assert hmtx[name][0] == expected, (fname, name, hmtx[name][0], expected)
        if fname == 'mock_cff_cid.otf':
            cff = f['CFF '].cff
            td = cff[cff.fontNames[0]]
            assert hasattr(td, 'ROS'), 'must be CID-keyed'
            got = [0 if n == '.notdef' else int(n[3:]) for n in td.charset]
            assert got == CIDS, (got, CIDS)
        print(f'{fname}: OK ({os.path.getsize(p)} bytes)')


if __name__ == '__main__':
    main()
