//! Deterministic font-embedding tests against the mock fonts from
//! `scripts/gen_mock_fonts.py` (committed under `tests/assets/fonts/mock/`).
//!
//! The three mock fonts share IDENTICAL, exactly-defined metrics (1000 units per
//! em, every glyph a DISTINCT advance width), so these tests verify glyph
//! *identity* and layout *math* against constants — the strategy azul-layout uses
//! for its text tests — instead of trusting a second font parser:
//!
//! ```text
//! glyph    char   advance
//! .notdef   —       500
//! space    ' '      250
//! A..J     A..J   300+50*i      (A=300, B=350, … J=750 — all distinct)
//! ```
//!
//! `mock_cff_cid.otf` is the #280 regression shape: a CID-keyed CFF whose charset
//! is NOT identity and NOT monotonic (gid 2..11 ↦ CID 901, 811, 821, …, 891;
//! space ↦ 700). A spec-following viewer (Acrobat, Preview) resolves every
//! Identity-H code as a CID *through that charset* (ISO 32000-1, 9.7.4.2), so
//! the content stream, `/W` and `/ToUnicode` must all be keyed by those CIDs.
//! printpdf used to emit raw glyph ids — PDFium's code==GID fallback made that
//! look fine in Chrome while Acrobat showed wrong or missing glyphs.
//!
//! Regenerate the fonts with `python3 scripts/gen_mock_fonts.py` (fontTools);
//! the tests only read the committed binaries.

#![cfg(feature = "text_layout")]

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object};
use printpdf::{
    ops::PdfFontHandle,
    units::{Mm, Pt},
    FontId, Op, ParsedFont, PdfDocument, PdfPage, PdfParseErrorSeverity, PdfParseOptions,
    PdfSaveOptions, PdfWarnMsg, TextItem, ToUnicodeCMap,
};

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");
const MOCK_CFF_NAMED: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_named.otf");
const MOCK_CFF_CID: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_cid.otf");

/// Uses A (first), J/I (last glyphs), B twice, and a space — order deliberately
/// not the glyph order, so code sequences can't accidentally pass sorted.
const TEXT: &str = "ABC JIB";

/// The defined advance width (1/1000 em units == font units at upm 1000).
fn advance_of(c: char) -> i64 {
    match c {
        ' ' => 250,
        'A'..='J' => 300 + 50 * (c as i64 - 'A' as i64),
        other => panic!("no defined metrics for {other:?}"),
    }
}

/// gid -> CID charset of `mock_cff_cid.otf`, as generated.
const CID_CHARSET: [u16; 12] = [0, 700, 901, 811, 821, 831, 841, 851, 861, 871, 881, 891];

/// The CID the mock CID-keyed font's charset assigns to `c`'s glyph.
fn cid_of(c: char) -> u16 {
    match c {
        ' ' => CID_CHARSET[1],
        'A'..='J' => CID_CHARSET[2 + (c as usize - 'A' as usize)],
        other => panic!("no CID for {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// PDF construction (mirrors tests/font_embedding.rs)
// ---------------------------------------------------------------------------

fn save_opts(subset_fonts: bool) -> PdfSaveOptions {
    PdfSaveOptions {
        subset_fonts,
        optimize: false,
        ..Default::default()
    }
}

fn pdf_with_font(font_bytes: &[u8], text: &str, subset: bool) -> (Vec<u8>, Vec<PdfWarnMsg>) {
    let mut doc = PdfDocument::new("mock font test");
    let mut parse_warnings = Vec::new();
    let font =
        ParsedFont::from_bytes(font_bytes, 0, &mut parse_warnings).expect("mock font parses");
    let font_id = doc.add_font(&font);

    let ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: printpdf::graphics::Point {
                x: Mm(20.0).into(),
                y: Mm(250.0).into(),
            },
        },
        Op::SetFont {
            font: PdfFontHandle::External(font_id.clone()),
            size: Pt(12.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ];
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    let mut warnings = Vec::new();
    let bytes = doc.with_pages(vec![page]).save(&save_opts(subset), &mut warnings);
    (bytes, warnings)
}

fn assert_no_errors(warnings: &[PdfWarnMsg], ctx: &str) {
    let errs: Vec<_> = warnings
        .iter()
        .filter(|w| w.severity == PdfParseErrorSeverity::Error)
        .collect();
    assert!(errs.is_empty(), "{ctx}: unexpected errors: {errs:#?}");
}

// ---------------------------------------------------------------------------
// Raw PDF dissection (independent of printpdf's own deserializer)
// ---------------------------------------------------------------------------

struct WrittenFont {
    cid_subtype: String,
    font_file_key: String,
    program: Vec<u8>,
    /// `/W`, flattened to (code -> width in 1/1000 em).
    widths: Vec<(u16, i64)>,
    to_unicode: ToUnicodeCMap,
    base_font: String,
    descriptor_font_name: String,
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).expect("dangling reference"),
        other => other,
    }
}

fn name_of(obj: &Object) -> String {
    String::from_utf8_lossy(obj.as_name().expect("expected /Name")).into_owned()
}

fn stream_bytes(doc: &Document, obj: &Object) -> Vec<u8> {
    let stream = resolve(doc, obj).as_stream().expect("expected a stream");
    stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

fn parse_w_array(doc: &Document, w: &Object) -> Vec<(u16, i64)> {
    let arr = resolve(doc, w).as_array().expect("/W must be an array");
    let mut out = Vec::new();
    let mut i = 0;
    while i < arr.len() {
        let first = resolve(doc, &arr[i]).as_i64().expect("/W: expected code");
        match arr.get(i + 1).map(|o| resolve(doc, o)) {
            Some(Object::Array(widths)) => {
                for (n, w) in widths.iter().enumerate() {
                    out.push((
                        first as u16 + n as u16,
                        resolve(doc, w).as_i64().expect("/W width"),
                    ));
                }
                i += 2;
            }
            Some(_) => {
                let last = resolve(doc, &arr[i + 1]).as_i64().expect("/W cLast");
                let w = resolve(doc, &arr[i + 2]).as_i64().expect("/W width");
                for code in first..=last {
                    out.push((code as u16, w));
                }
                i += 3;
            }
            None => break,
        }
    }
    out
}

/// The single external font of the produced one-page PDF, as written.
fn written_font(pdf: &[u8]) -> WrittenFont {
    let doc = Document::load_mem(pdf).expect("lopdf loads the PDF");
    let (_, page_id) = doc.get_pages().into_iter().next().expect("one page");
    let page = doc.get_dictionary(page_id).expect("page dict");
    let resources = resolve(&doc, page.get(b"Resources").expect("/Resources"))
        .as_dict()
        .expect("resources dict");
    let fonts = resolve(&doc, resources.get(b"Font").expect("/Font"))
        .as_dict()
        .expect("font dict");

    for (_, font_ref) in fonts.iter() {
        let font = resolve(&doc, font_ref).as_dict().expect("font dict");
        let Ok(descendants) = font.get(b"DescendantFonts") else {
            continue; // builtin
        };
        let base_font = font.get(b"BaseFont").map(name_of).unwrap_or_default();
        let to_unicode = ToUnicodeCMap::parse(&String::from_utf8_lossy(&stream_bytes(
            &doc,
            font.get(b"ToUnicode").expect("/ToUnicode"),
        )))
        .expect("ToUnicode parses");

        let descendant = resolve(&doc, &resolve(&doc, descendants).as_array().unwrap()[0])
            .as_dict()
            .expect("descendant dict");
        let cid_subtype = descendant.get(b"Subtype").map(name_of).unwrap_or_default();
        let widths = parse_w_array(&doc, descendant.get(b"W").expect("/W"));

        let descriptor: &Dictionary = resolve(
            &doc,
            descendant.get(b"FontDescriptor").expect("/FontDescriptor"),
        )
        .as_dict()
        .expect("descriptor dict");
        let descriptor_font_name = descriptor.get(b"FontName").map(name_of).unwrap_or_default();
        let (font_file_key, program) = ["FontFile2", "FontFile3", "FontFile"]
            .iter()
            .find_map(|k| {
                descriptor
                    .get(k.as_bytes())
                    .ok()
                    .map(|ff| (k.to_string(), stream_bytes(&doc, ff)))
            })
            .expect("embedded font program");

        return WrittenFont {
            cid_subtype,
            font_file_key,
            program,
            widths,
            to_unicode,
            base_font,
            descriptor_font_name,
        };
    }
    panic!("no external font in PDF");
}

/// Identity-H codes the content stream draws, in order.
fn content_stream_codes(pdf: &[u8]) -> Vec<u16> {
    let doc = Document::load_mem(pdf).expect("load");
    let mut codes = Vec::new();
    for (page_num, _) in doc.get_pages() {
        let content =
            lopdf::content::Content::decode(&doc.get_page_content(doc.get_pages()[&page_num]))
                .expect("decode content stream");
        for op in content.operations {
            let strings: Vec<&[u8]> = match op.operator.as_str() {
                "Tj" => op.operands.iter().filter_map(|o| o.as_str().ok()).collect(),
                "TJ" => op
                    .operands
                    .iter()
                    .filter_map(|o| o.as_array().ok())
                    .flatten()
                    .filter_map(|o| o.as_str().ok())
                    .collect(),
                _ => continue,
            };
            for s in strings {
                for pair in s.chunks_exact(2) {
                    codes.push(u16::from_be_bytes([pair[0], pair[1]]));
                }
            }
        }
    }
    codes
}

// ---------------------------------------------------------------------------
// The viewer simulation
// ---------------------------------------------------------------------------

/// Resolve an Identity-H code to a glyph id the way a spec-following viewer
/// does (ISO 32000-1, 9.7.4.2): through the CFF charset for CID-keyed CFF
/// programs, identity otherwise.
fn viewer_resolve(program: &[u8], code: u16) -> u16 {
    match printpdf::font::cff_charset_gid_to_cid_map(program, 0) {
        Some(map) => *map
            .iter()
            .find_map(|(gid, cid)| (*cid == code).then_some(gid))
            .unwrap_or_else(|| {
                panic!("code {code} is not a CID of the embedded charset — a viewer draws .notdef")
            }),
        None => code,
    }
}

/// The advance width of `gid` in the embedded program, in font units (== 1/1000
/// em: the mocks are 1000 upm).
fn embedded_advance(program: &[u8], gid: u16) -> i64 {
    let font = ParsedFont::from_bytes(program, 0, &mut Vec::new())
        .expect("embedded font program parses");
    let g = font
        .get_or_decode_glyph(gid)
        .unwrap_or_else(|| panic!("embedded program has no glyph {gid}"));
    g.horz_advance as i64
}

/// Run every invariant against one (font, subsetting) combination.
fn assert_mock_font_invariants(
    font_bytes: &[u8],
    subset: bool,
    expected_subtype: &str,
    expected_font_file: &str,
    ctx: &str,
) {
    let (pdf, warnings) = pdf_with_font(font_bytes, TEXT, subset);
    assert_no_errors(&warnings, ctx);

    let font = written_font(&pdf);
    let codes = content_stream_codes(&pdf);
    let chars: Vec<char> = TEXT.chars().collect();

    // -- container honesty ---------------------------------------------------
    assert_eq!(font.cid_subtype, expected_subtype, "{ctx}: descendant subtype");
    assert_eq!(font.font_file_key, expected_font_file, "{ctx}: font file key");
    if expected_subtype == "CIDFontType0" {
        assert_eq!(&font.program[..4], b"OTTO", "{ctx}: CFF program magic");
    } else {
        assert_eq!(&font.program[..4], &[0, 1, 0, 0], "{ctx}: glyf program magic");
    }

    // -- subset tag discipline ----------------------------------------------
    // 6-uppercase-letter tag iff actually subset, and BaseFont == FontName.
    let tagged = font.base_font.len() > 7 && font.base_font.as_bytes()[6] == b'+';
    assert_eq!(
        tagged, subset,
        "{ctx}: subset tag must appear iff the program was subset (BaseFont {})",
        font.base_font
    );
    assert_eq!(
        font.base_font, font.descriptor_font_name,
        "{ctx}: /BaseFont and /FontName must match"
    );

    // -- every drawn code: correct glyph, correct width, correct unicode -----
    assert_eq!(
        codes.len(),
        chars.len(),
        "{ctx}: one Identity-H code per source char"
    );
    let mut total_width = 0i64;
    for (i, (&code, &ch)) in codes.iter().zip(chars.iter()).enumerate() {
        // Glyph identity via the defined metrics: resolve the code the way a
        // viewer does and check the outline's advance is the char's advance.
        let gid = viewer_resolve(&font.program, code);
        assert_ne!(gid, 0, "{ctx}: char {i} {ch:?} resolves to .notdef");
        assert_eq!(
            embedded_advance(&font.program, gid),
            advance_of(ch),
            "{ctx}: char {i} {ch:?} (code {code}) resolves to the wrong glyph"
        );

        // /W must be keyed by the code and carry the defined width.
        let w = font
            .widths
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, w)| *w)
            .unwrap_or_else(|| panic!("{ctx}: code {code} has no /W entry"));
        assert_eq!(w, advance_of(ch), "{ctx}: /W width for {ch:?}");
        total_width += w;

        // ToUnicode must be keyed by the code and map to the char.
        assert_eq!(
            font.to_unicode.lookup_string(code as u32).as_deref(),
            Some(ch.to_string().as_str()),
            "{ctx}: ToUnicode for code {code}"
        );
    }

    // -- layout math against constants --------------------------------------
    // "ABC JIB" = 300+350+400+250+750+700+350. With defined metrics the
    // text advance is checkable as arithmetic, not against another engine.
    let expected_total: i64 = chars.iter().map(|c| advance_of(*c)).sum();
    assert_eq!(total_width, expected_total, "{ctx}: total text advance");
    assert_eq!(expected_total, 3100, "defined-metrics sanity");

    // -- roundtrip through printpdf's own parser -----------------------------
    let mut rt_warnings = Vec::new();
    let parsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut rt_warnings)
        .unwrap_or_else(|e| panic!("{ctx}: roundtrip parse failed: {e}"));
    let extracted: String = parsed
        .extract_text()
        .iter()
        .flatten()
        .flat_map(|chunk| chunk.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(extracted, "ABC JIB", "{ctx}: extracted text");

    // Re-save and dissect again: the second generation must uphold the same
    // code/glyph/width consistency (this is where a wrong CID->GID parse shows).
    let mut resave_warnings = Vec::new();
    let resaved = parsed.save(&save_opts(subset), &mut resave_warnings);
    let font2 = written_font(&resaved);
    let codes2 = content_stream_codes(&resaved);
    assert_eq!(codes2.len(), chars.len(), "{ctx}: resaved code count");
    for (i, (&code, &ch)) in codes2.iter().zip(chars.iter()).enumerate() {
        let gid = viewer_resolve(&font2.program, code);
        assert_eq!(
            embedded_advance(&font2.program, gid),
            advance_of(ch),
            "{ctx}: resaved char {i} {ch:?} (code {code}) resolves to the wrong glyph"
        );
    }
}

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

#[test]
fn mock_ttf_full_embed() {
    assert_mock_font_invariants(MOCK_TTF, false, "CIDFontType2", "FontFile2", "ttf/full");
}

#[test]
fn mock_ttf_subset() {
    assert_mock_font_invariants(MOCK_TTF, true, "CIDFontType2", "FontFile2", "ttf/subset");
}

#[test]
fn mock_cff_named_full_embed() {
    assert_mock_font_invariants(
        MOCK_CFF_NAMED,
        false,
        "CIDFontType0",
        "FontFile3",
        "cff-named/full",
    );
}

#[test]
fn mock_cff_named_subset() {
    assert_mock_font_invariants(
        MOCK_CFF_NAMED,
        true,
        "CIDFontType0",
        "FontFile3",
        "cff-named/subset",
    );
}

#[test]
fn mock_cff_cid_full_embed() {
    assert_mock_font_invariants(
        MOCK_CFF_CID,
        false,
        "CIDFontType0",
        "FontFile3",
        "cff-cid/full",
    );
}

#[test]
fn mock_cff_cid_subset() {
    assert_mock_font_invariants(
        MOCK_CFF_CID,
        true,
        "CIDFontType0",
        "FontFile3",
        "cff-cid/subset",
    );
}

/// #280 canary, spelled out: for a CID-keyed CFF the Identity-H codes must be
/// the charset CIDs — NOT glyph ids. The allsorts subsetter preserves the
/// original CIDs in the subset charset, so the expected code sequence is the
/// same with and without subsetting, and never equals the gid sequence.
#[test]
fn cid_keyed_cff_codes_are_charset_cids() {
    for subset in [false, true] {
        let (pdf, warnings) = pdf_with_font(MOCK_CFF_CID, TEXT, subset);
        assert_no_errors(&warnings, "cid canary");
        let codes = content_stream_codes(&pdf);
        let expected: Vec<u16> = TEXT.chars().map(cid_of).collect();
        assert_eq!(
            codes, expected,
            "subset={subset}: content stream must emit the charset CIDs \
             (901 for 'A', 700 for space, …), not glyph ids — Acrobat and \
             Preview resolve codes through the charset (#280)"
        );
    }
}

/// The parse side of #280: loading a spec-correct PDF (codes == CIDs) must
/// resolve each code back to the real glyph id via the embedded charset, so
/// re-rendering and re-saving keep the right outlines.
#[test]
fn parse_resolves_cids_through_charset() {
    let (pdf, _) = pdf_with_font(MOCK_CFF_CID, TEXT, false);
    let mut warnings = Vec::new();
    let parsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings)
        .expect("parse");

    // The embedded program is the full mock font: glyph ids are the generation
    // order (.notdef, space, A..J), NOT the CIDs.
    let expected_gids: BTreeMap<char, u16> = ('A'..='J')
        .enumerate()
        .map(|(i, c)| (c, 2 + i as u16))
        .chain([(' ', 1u16)])
        .collect();

    let mut checked = 0;
    for page in &parsed.pages {
        for op in &page.ops {
            let Op::ShowText { items } = op else { continue };
            for item in items {
                let TextItem::GlyphIds(glyphs) = item else { continue };
                for (i, g) in glyphs.iter().enumerate() {
                    let ch = TEXT.chars().nth(i).expect("in range");
                    assert_eq!(
                        g.gid, expected_gids[&ch],
                        "char {i} {ch:?}: parsed gid must be the charset-resolved \
                         glyph id, not the CID"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, TEXT.chars().count(), "all glyphs checked");
}
