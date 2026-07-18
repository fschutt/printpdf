#!/usr/bin/env python3
"""Verify CID/GID consistency of Identity-H fonts in a printpdf-produced PDF.

Simulates a spec-following viewer (Acrobat/Preview):
  code (2-byte, content stream) --Identity-H--> CID --charset/CIDToGIDMap--> GID --cmap^-1--> char

Asserts, for every text-showing string:
  1. every emitted code resolves to a real glyph in the embedded font program
  2. the resolved glyph is the glyph the original font's cmap assigns to the
     intended character (ground truth passed via --expect)
  3. /W widths per code match the embedded font's hmtx advance (scaled to 1000/em)
  4. ToUnicode maps each code to the intended character
Exit code != 0 on any mismatch.
"""
import re, zlib, sys, io, argparse

from fontTools.ttLib import TTFont


def pdf_objects(pdf):
    objs = {}
    for m in re.finditer(rb'(\d+) 0 obj(.*?)endobj', pdf, re.DOTALL):
        num, body = int(m.group(1)), m.group(2)
        if b'stream' in body:
            head, rest = body.split(b'stream', 1)
            data = rest.lstrip(b'\r\n').rsplit(b'endstream', 1)[0]
            if b'FlateDecode' in head:
                try:
                    data = zlib.decompress(data.strip(b'\r\n'))
                except Exception:
                    pass
            objs[num] = (head, data)
        else:
            objs[num] = (body, None)
    return objs


def parse_w_array(dict_bytes):
    m = re.search(rb'/W\s*\[(.*)', dict_bytes, re.DOTALL)
    if not m:
        return {}
    s = m.group(1)
    # cut at the matching close bracket
    depth = 1
    out = []
    for ch in s:
        c = bytes([ch])
        if c == b'[':
            depth += 1
        elif c == b']':
            depth -= 1
            if depth == 0:
                break
        out.append(ch)
    inner = bytes(out).decode('latin1')
    widths = {}
    i = 0
    toks = re.findall(r'\[|\]|-?\d+', inner)
    idx = 0
    while idx < len(toks):
        t = toks[idx]
        if t == '[' or t == ']':
            idx += 1
            continue
        start = int(t)
        if idx + 1 < len(toks) and toks[idx + 1] == '[':
            idx += 2
            code = start
            while idx < len(toks) and toks[idx] != ']':
                widths[code] = int(toks[idx])
                code += 1
                idx += 1
            idx += 1  # skip ]
        else:
            # c1 c2 w form
            c1, c2, w = start, int(toks[idx + 1]), int(toks[idx + 2])
            for code in range(c1, c2 + 1):
                widths[code] = w
            idx += 3
    return widths


def parse_tounicode(data):
    """Parse bfchar/bfranges into {code: unicode string}."""
    out = {}
    for m in re.finditer(rb'beginbfchar(.*?)endbfchar', data, re.DOTALL):
        for mm in re.finditer(rb'<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>', m.group(1)):
            code = int(mm.group(1), 16)
            hexstr = mm.group(2).decode()
            u = bytes.fromhex(hexstr).decode('utf-16-be', errors='replace')
            out[code] = u
    for m in re.finditer(rb'beginbfrange(.*?)endbfrange', data, re.DOTALL):
        body = m.group(1)
        for mm in re.finditer(rb'<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>', body):
            lo, hi = int(mm.group(1), 16), int(mm.group(2), 16)
            base = bytes.fromhex(mm.group(3).decode())
            baseu = int.from_bytes(base, 'big')
            nb = len(base)
            for i, code in enumerate(range(lo, hi + 1)):
                u = (baseu + i).to_bytes(nb, 'big').decode('utf-16-be', errors='replace')
                out[code] = u
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('pdf')
    ap.add_argument('--expect', help='file with the exact text lines shown, one per Tj', default=None)
    args = ap.parse_args()

    pdf = open(args.pdf, 'rb').read()
    objs = pdf_objects(pdf)

    font_bytes = None
    font_dict = None
    tounicode = None
    content = None
    for num, (head, data) in objs.items():
        if data and data[:4] in (b'OTTO', b'\x00\x01\x00\x00', b'true', b'ttcf'):
            font_bytes = data
        elif head and b'/W' in head and b'/DW' in head and b'Font' in head:
            font_dict = head
        elif data and (b'bfchar' in data or b'bfrange' in data):
            tounicode = parse_tounicode(data)
        elif data and (b' Tj' in data or b' TJ' in data):
            content = data

    assert font_bytes is not None, 'no embedded font program found'
    assert font_dict is not None, 'no CID font dict with /W found'
    assert content is not None, 'no content stream found'
    assert tounicode is not None, 'no ToUnicode CMap found'

    f = TTFont(io.BytesIO(font_bytes))
    upm = f['head'].unitsPerEm
    order = f.getGlyphOrder()
    num_glyphs = f['maxp'].numGlyphs
    hmtx = f['hmtx']

    # Viewer-side CID -> GID resolution
    if 'CFF ' in f:
        cff = f['CFF '].cff
        td = cff[cff.fontNames[0]]
        if hasattr(td, 'ROS'):
            # CID-keyed: charset[gid] = 'cidXXXXX'; viewer maps CID -> gid via charset
            cid_to_gid = {}
            for gid, name in enumerate(td.charset):
                if name == '.notdef':
                    cid_to_gid[0] = gid if gid == 0 else cid_to_gid.get(0, 0)
                    continue
                assert name.startswith('cid'), f'unexpected charset name {name}'
                cid_to_gid[int(name[3:])] = gid
        else:
            cid_to_gid = None  # name-keyed: CID == GID
    else:
        cid_to_gid = None  # TrueType with CIDToGIDMap Identity: CID == GID

    def resolve(cid):
        if cid_to_gid is None:
            return cid if cid < num_glyphs else None
        return cid_to_gid.get(cid)

    # cmap of the EMBEDDED font (subset keeps cmap of used chars; full font = all)
    cmap = f.getBestCmap() if 'cmap' in f else {}

    widths = parse_w_array(font_dict)

    # Collect shown strings
    shown = [bytes.fromhex(m.group(1).decode()) for m in
             re.finditer(rb'<([0-9A-Fa-f]+)>\s*T[jJ]', content)]
    expect_lines = None
    if args.expect:
        expect_lines = [l for l in open(args.expect, encoding='utf-8').read().splitlines()]
        assert len(expect_lines) == len(shown), \
            f'{len(shown)} shown strings vs {len(expect_lines)} expected lines'

    errors = []
    for li, s in enumerate(shown):
        codes = [int.from_bytes(s[i:i+2], 'big') for i in range(0, len(s), 2)]
        line_chars = []
        for ci, code in enumerate(codes):
            gid = resolve(code)
            if gid is None:
                errors.append(f'line {li}: code {code} resolves to NO glyph (viewer shows .notdef)')
                continue
            # ToUnicode check
            u = tounicode.get(code)
            if u is None:
                errors.append(f'line {li}: code {code} missing from ToUnicode')
            else:
                line_chars.append(u)
                # ground-truth glyph check: the char's glyph in the embedded font
                # must be the glyph the viewer resolves to
                cp = ord(u[0]) if len(u) >= 1 else None
                if cp is not None and cp in cmap:
                    expected_glyph = cmap[cp]
                    if order[gid] != expected_glyph:
                        errors.append(
                            f'line {li}: code {code} -> gid {gid} ({order[gid]}) but char '
                            f'{u!r} maps to glyph {expected_glyph} — WRONG GLYPH in viewer')
            # width check
            w = widths.get(code)
            if w is None:
                continue  # falls back to DW; only check if declared
            adv = hmtx[order[gid]][0]
            expected_w = int(adv * 1000 / upm)
            if abs(w - expected_w) > 1:
                errors.append(
                    f'line {li}: code {code}: /W {w} != hmtx {expected_w} (gid {gid})')
        if expect_lines is not None:
            got = ''.join(line_chars)
            want = expect_lines[li]
            if got != want:
                errors.append(f'line {li}: ToUnicode text {got!r} != expected {want!r}')

    if errors:
        print(f'FAIL: {len(errors)} error(s)')
        for e in errors[:40]:
            print('  ' + e)
        sys.exit(1)
    print(f'OK: {sum(len(s)//2 for s in shown)} codes across {len(shown)} strings verified '
          f'(charset={"CID-keyed CFF" if cid_to_gid else "identity"}, '
          f'{len(widths)} /W entries, {len(tounicode)} ToUnicode entries)')


if __name__ == '__main__':
    main()
