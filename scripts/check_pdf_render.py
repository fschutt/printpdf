#!/usr/bin/env python3
"""Empirical FreeType-family render check for a printpdf-produced PDF.

`verify_pdf_font.py` proves the *Acrobat/Preview* view of the file (charset
semantics) and structurally rejects the one encoding that CANNOT satisfy both
viewer families (sfnt-wrapped CID-keyed CFF with a non-identity charset,
#280/#281). This script closes the loop on the OTHER family by actually
running poppler — the same FreeType-based resolution PDFium/Chrome uses:

  1. `pdffonts` must not classify any font as "CID Type 0C (OT)" (poppler's
     name for a CID-keyed CFF still inside an OpenType wrapper) when
     --forbid-otto-cid-cff is given. Independent tripwire for #280: fontTools
     and poppler have to agree the wrapper is gone.
     (Only pass the flag for PDFs whose fonts are all CID-keyed — a name-keyed
     CFF legitimately embeds as a whole sfnt and gets the same label.)
  2. `pdftoppm` renders page 1 and the count of dark pixels must reach
     --min-ink. The #280 inversion drew codes as glyph indices: most CIDs
     exceeded the subset's glyph count and the page came out near-blank
     (~7400 dark px correct vs near-0 broken, at -r 100). Wrong-but-inked
     glyphs are the structural guard's job; THIS floor catches the
     blank/missing-glyph family, whatever causes it next time.

Exit code != 0 on any failure. Requires poppler-utils.
"""
import argparse
import subprocess
import sys


def pdffonts_forbid_otto_cid_cff(pdf):
    proc = subprocess.run(['pdffonts', pdf], capture_output=True, text=True)
    out = proc.stdout
    print(out.rstrip())
    if proc.returncode != 0:
        print(f'FAIL: {pdf}: pdffonts exited {proc.returncode}: {proc.stderr.strip()}')
        return False
    if 'CID Type 0C (OT)' in out:
        print(f'FAIL: {pdf}: poppler still sees a CID-keyed CFF inside an '
              f'OpenType wrapper ("CID Type 0C (OT)").')
        print('  FreeType does not flag such a face CID-keyed, so poppler and '
              'PDFium resolve Identity-H codes as glyph indices while '
              'Acrobat resolves them through the charset (#280).')
        print('  Embed the bare CFF table (/FontFile3 /Subtype /CIDFontType0C) instead.')
        return False
    return True


def dark_pixels(pdf, dpi):
    """Dark-pixel count of page 1, rendered by poppler at `dpi` in grayscale."""
    proc = subprocess.run(
        ['pdftoppm', '-gray', '-r', str(dpi), '-f', '1', '-l', '1', '-singlefile', pdf],
        capture_output=True,
    )
    pgm = proc.stdout
    # A PDF poppler cannot render at all fails the ink floor by definition.
    if proc.returncode != 0 or not pgm.startswith(b'P5'):
        print(f'pdftoppm exited {proc.returncode} for {pdf}: {proc.stderr.decode(errors="replace").strip()}')
        return 0
    # P5: "P5" <ws> width <ws> height <ws> maxval <single ws> raster
    tokens, pos = [], 2
    while len(tokens) < 3:
        while pgm[pos:pos + 1].isspace():
            pos += 1
        if pgm[pos:pos + 1] == b'#':  # comment line
            pos = pgm.index(b'\n', pos) + 1
            continue
        start = pos
        while not pgm[pos:pos + 1].isspace():
            pos += 1
        tokens.append(int(pgm[start:pos]))
    width, height, maxval = tokens
    raster = pgm[pos + 1:pos + 1 + width * height]
    threshold = maxval // 2
    return sum(1 for b in raster if b < threshold)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('pdf')
    ap.add_argument('--min-ink', type=int, required=True,
                    help='minimum dark-pixel count on page 1 at --dpi')
    ap.add_argument('--dpi', type=int, default=100)
    ap.add_argument('--forbid-otto-cid-cff', action='store_true',
                    help='fail if pdffonts reports "CID Type 0C (OT)"')
    args = ap.parse_args()

    ok = True
    if args.forbid_otto_cid_cff:
        ok = pdffonts_forbid_otto_cid_cff(args.pdf)

    ink = dark_pixels(args.pdf, args.dpi)
    if ink < args.min_ink:
        print(f'FAIL: {args.pdf}: page 1 renders with {ink} dark pixels '
              f'(< {args.min_ink}) at {args.dpi} dpi — poppler is drawing '
              f'blanks/notdef where the text should be.')
        ok = False
    else:
        print(f'OK: {args.pdf}: {ink} dark pixels (>= {args.min_ink}) at {args.dpi} dpi')

    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
