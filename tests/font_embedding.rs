//! Structural verification of font embedding.
//!
//! These tests deliberately do **not** go through printpdf's own parser to check
//! printpdf's writer — a bug that is symmetric across both would cancel itself out and
//! the test would still pass. Instead they crack the produced PDF open with `lopdf`,
//! walk down to the actual `/FontFile2` (or `/FontFile3`) stream, and assert on the
//! bytes that a real reader (Acrobat, pdfium, poppler) would see.
//!
//! # What broke in 0.10.0 (issue #277)
//!
//! azul-layout's `ParsedFont::from_bytes` stopped retaining the source font bytes (a
//! deliberate perf change — layout and rasterization never read them, and holding them
//! duplicated a 4.27 MiB `.ttc` once per face). printpdf read them straight off the
//! struct and `.unwrap_or_default()`'d the `None` into an **empty `Vec<u8>`**, so every
//! external font embedded as a zero-length `/FontFile2`:
//!
//! ```text
//! $ pdffonts font-test.pdf
//! Syntax Error: Embedded font file may be invalid
//! name                  type          encoding    emb sub uni
//! HEIGID+Roboto-Medium  CID TrueType  Identity-H  yes yes yes   <- "emb yes", but empty
//! ```
//!
//! The PDF was 2,727 bytes instead of 165,238 and no glyph rendered. Nothing warned,
//! nothing failed — the old round-trip test still passed, because it only asserted that
//! *some* font resource came back.
//!
//! So the invariants below are the ones that would have caught it:
//!   - the font program is present, non-empty, and a *parseable sfnt*;
//!   - every glyph id in the content stream exists in that embedded program;
//!   - `/W` and `/ToUnicode` are keyed by those same glyph ids;
//!   - `/ToUnicode` maps them back to the exact source text (no mojibake);
//!   - a font that genuinely cannot be embedded raises an error and omits `/FontFile2`
//!     rather than writing an empty one.

#![cfg(feature = "text_layout")]

use lopdf::{Dictionary, Document, Object};
use printpdf::{
    ops::PdfFontHandle,
    units::{Mm, Pt},
    BuiltinFont, FontId, Op, ParsedFont, PdfDocument, PdfPage, PdfParseErrorSeverity,
    PdfParseOptions, PdfSaveOptions, PdfWarnMsg, TextItem, ToUnicodeCMap,
};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
const NOTO_JP_OTF: &[u8] = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");

/// Text from the issue, plus a non-ASCII script to keep the CMap honest.
const LATIN: &str = "Roboto";
const CYRILLIC: &str = "Привет, как дела?";

// ---------------------------------------------------------------------------
// PDF construction
// ---------------------------------------------------------------------------

fn save_opts(subset_fonts: bool) -> PdfSaveOptions {
    PdfSaveOptions {
        subset_fonts,
        optimize: false,
        ..Default::default()
    }
}

/// Build a one-page PDF that shows `text` in `font_bytes`. Returns the PDF and the
/// warnings `save()` produced.
fn pdf_with_font(font_bytes: &[u8], text: &str, subset: bool) -> (Vec<u8>, Vec<PdfWarnMsg>) {
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).expect("font must parse");
    let mut doc = PdfDocument::new("font-embedding-test");
    let font_id = doc.add_font(&font);
    let mut warnings = Vec::new();
    let bytes = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            show_text_ops(&font_id, text),
        )])
        .save(&save_opts(subset), &mut warnings);
    (bytes, warnings)
}

fn show_text_ops(font_id: &FontId, text: &str) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::External(font_id.clone()),
            size: Pt(24.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

fn errors(warnings: &[PdfWarnMsg]) -> Vec<&PdfWarnMsg> {
    warnings
        .iter()
        .filter(|w| w.severity == PdfParseErrorSeverity::Error)
        .collect()
}

fn assert_no_errors(warnings: &[PdfWarnMsg], ctx: &str) {
    let errs = errors(warnings);
    assert!(
        errs.is_empty(),
        "{ctx}: save() reported errors: {:#?}",
        errs.iter().map(|e| &e.msg).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Structural inspection of the produced PDF (via lopdf, not printpdf's parser)
// ---------------------------------------------------------------------------

/// A font exactly as it appears in the written PDF.
#[derive(Debug)]
struct EmbeddedFont {
    base_font: String,
    /// `CIDFontType2` (TrueType) or `CIDFontType0` (CFF), from the descendant font.
    cid_subtype: String,
    /// `FontFile2` / `FontFile3`, or `None` when the descriptor embeds no program.
    font_file_key: Option<String>,
    /// The raw font program bytes. `None` when there is no `/FontFile*` entry at all.
    font_program: Option<Vec<u8>>,
    /// `/W`, flattened to (glyph id -> width in 1/1000 em).
    widths: Vec<(u16, i64)>,
    /// The decoded `/ToUnicode` CMap stream.
    to_unicode: Option<String>,
    descriptor: Dictionary,
}

impl EmbeddedFont {
    fn program(&self) -> &[u8] {
        self.font_program
            .as_deref()
            .unwrap_or_else(|| panic!("font {} embeds no font program", self.base_font))
    }

    fn width_of(&self, gid: u16) -> Option<i64> {
        self.widths.iter().find(|(g, _)| *g == gid).map(|(_, w)| *w)
    }
}

fn stream_bytes(doc: &Document, obj: &Object) -> Vec<u8> {
    let obj = resolve(doc, obj);
    let stream = obj.as_stream().expect("expected a stream");
    // Font programs are written uncompressed; ToUnicode CMaps may be compressed.
    stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).expect("dangling reference"),
        other => other,
    }
}

fn name_of(obj: &Object) -> String {
    String::from_utf8_lossy(obj.as_name().expect("expected a /Name")).into_owned()
}

/// Flatten a PDF `/W` array. Format is `[ cFirst [w1 w2 …] cFirst cLast w … ]`.
fn parse_w_array(doc: &Document, w: &Object) -> Vec<(u16, i64)> {
    let arr = resolve(doc, w).as_array().expect("/W must be an array");
    let mut out = Vec::new();
    let mut i = 0;
    while i < arr.len() {
        let first = resolve(doc, &arr[i]).as_i64().expect("/W: expected CID");
        match arr.get(i + 1).map(|o| resolve(doc, o)) {
            Some(Object::Array(widths)) => {
                for (n, w) in widths.iter().enumerate() {
                    let w = resolve(doc, w).as_i64().expect("/W: expected width");
                    out.push((first as u16 + n as u16, w));
                }
                i += 2;
            }
            // `cFirst cLast w` range form
            Some(_) => {
                let last = resolve(doc, &arr[i + 1]).as_i64().expect("/W: expected cLast");
                let w = resolve(doc, &arr[i + 2]).as_i64().expect("/W: expected width");
                for gid in first..=last {
                    out.push((gid as u16, w));
                }
                i += 3;
            }
            None => break,
        }
    }
    out
}

/// Walk Catalog -> Pages -> Page -> Resources -> Font and pull out every font, as
/// written. Deliberately independent of printpdf's own deserializer.
fn fonts_in(pdf: &[u8]) -> Vec<EmbeddedFont> {
    let doc = Document::load_mem(pdf).expect("lopdf must be able to load the PDF");
    let mut out = Vec::new();

    for (_, page_id) in doc.get_pages() {
        let page = doc.get_dictionary(page_id).expect("page dict");
        let resources = resolve(&doc, page.get(b"Resources").expect("page /Resources"))
            .as_dict()
            .expect("/Resources dict");
        let Ok(fonts) = resources.get(b"Font") else {
            continue;
        };
        let fonts = resolve(&doc, fonts).as_dict().expect("/Font dict");

        for (_, font_ref) in fonts.iter() {
            let font = resolve(&doc, font_ref).as_dict().expect("font dict");
            let base_font = font.get(b"BaseFont").map(name_of).unwrap_or_default();

            let to_unicode = font
                .get(b"ToUnicode")
                .ok()
                .map(|tu| String::from_utf8_lossy(&stream_bytes(&doc, tu)).into_owned());

            // Built-in (Type1) fonts have no descendant and embed nothing.
            let Ok(descendants) = font.get(b"DescendantFonts") else {
                out.push(EmbeddedFont {
                    base_font,
                    cid_subtype: font.get(b"Subtype").map(name_of).unwrap_or_default(),
                    font_file_key: None,
                    font_program: None,
                    widths: Vec::new(),
                    to_unicode,
                    descriptor: Dictionary::new(),
                });
                continue;
            };

            let descendant = resolve(&doc, &resolve(&doc, descendants).as_array().unwrap()[0])
                .as_dict()
                .expect("descendant font dict");
            let cid_subtype = descendant.get(b"Subtype").map(name_of).unwrap_or_default();
            let widths = descendant
                .get(b"W")
                .map(|w| parse_w_array(&doc, w))
                .unwrap_or_default();

            let descriptor = resolve(
                &doc,
                descendant.get(b"FontDescriptor").expect("/FontDescriptor"),
            )
            .as_dict()
            .expect("descriptor dict")
            .clone();

            let (font_file_key, font_program) = ["FontFile2", "FontFile3", "FontFile"]
                .iter()
                .find_map(|k| {
                    descriptor
                        .get(k.as_bytes())
                        .ok()
                        .map(|ff| (k.to_string(), stream_bytes(&doc, ff)))
                })
                .map(|(k, v)| (Some(k), Some(v)))
                .unwrap_or((None, None));

            out.push(EmbeddedFont {
                base_font,
                cid_subtype,
                font_file_key,
                font_program,
                widths,
                to_unicode,
                descriptor,
            });
        }
    }
    out
}

/// Every glyph id the page's content stream actually draws.
///
/// With Identity-H encoding, `Tj`/`TJ` string operands are big-endian u16 glyph ids.
fn content_stream_gids(pdf: &[u8]) -> Vec<u16> {
    let doc = Document::load_mem(pdf).expect("load");
    let mut gids = Vec::new();

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
                    gids.push(u16::from_be_bytes([pair[0], pair[1]]));
                }
            }
        }
    }
    gids
}

/// sfnt magic. `0x00010000` / `true` = TrueType outlines, `OTTO` = CFF outlines.
fn is_sfnt(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0x00, 0x01, 0x00, 0x00]) | Some(b"true") | Some(b"OTTO") | Some(b"ttcf")
    )
}

// ===========================================================================
// The #277 regression: the font program must actually be there
// ===========================================================================

/// The exact scenario from issue #277: `subset_fonts: false`, an external TTF.
///
/// In 0.10.0 this produced a 2,727-byte PDF whose `/FontFile2` stream was empty.
#[test]
fn truetype_font_program_is_embedded_without_subsetting() {
    let (pdf, warnings) = pdf_with_font(ROBOTO_TTF, LATIN, false);
    assert_no_errors(&warnings, "full-font embed");

    let fonts = fonts_in(&pdf);
    assert_eq!(fonts.len(), 1, "expected exactly one font, got {fonts:#?}");
    let font = &fonts[0];

    assert_eq!(font.cid_subtype, "CIDFontType2", "TrueType => CIDFontType2");
    assert_eq!(
        font.font_file_key.as_deref(),
        Some("FontFile2"),
        "TrueType font program must be embedded as /FontFile2"
    );

    let program = font.program();
    assert!(
        !program.is_empty(),
        "/FontFile2 stream is EMPTY — this is issue #277. The font parsed, the font dict \
         was written, /W and /ToUnicode are present, and the PDF looks structurally fine \
         to everything except a real PDF reader, which cannot extract the font."
    );
    assert!(
        is_sfnt(program),
        "/FontFile2 does not start with an sfnt magic; first bytes: {:02x?}",
        &program[..program.len().min(8)]
    );

    // Embedding the *full* font must actually embed the full font.
    assert!(
        program.len() > ROBOTO_TTF.len() / 2,
        "full-font embed wrote only {} bytes for a {}-byte font",
        program.len(),
        ROBOTO_TTF.len()
    );

    // The 0.10.0 PDF was 2,727 bytes. A PDF that embeds a 162 KB font cannot be.
    assert!(
        pdf.len() > 100_000,
        "PDF is only {} bytes — the font program is missing (0.10.0 produced 2,727)",
        pdf.len()
    );

    // And the embedded program must be a font, not just font-shaped bytes.
    let reparsed = ParsedFont::from_bytes(program, 0, &mut Vec::new())
        .expect("the embedded /FontFile2 must itself parse as a font");
    assert!(reparsed.num_glyphs() > 0);
}

/// Same, with subsetting on — the path the HTML renderer uses, and the default.
#[test]
fn truetype_font_program_is_embedded_with_subsetting() {
    let (pdf, warnings) = pdf_with_font(ROBOTO_TTF, LATIN, true);
    assert_no_errors(&warnings, "subset embed");

    let fonts = fonts_in(&pdf);
    let font = &fonts[0];
    let program = font.program();

    assert!(!program.is_empty(), "subset /FontFile2 stream is empty");
    assert!(is_sfnt(program), "subset /FontFile2 is not an sfnt");

    let reparsed = ParsedFont::from_bytes(program, 0, &mut Vec::new())
        .expect("the embedded subset must itself parse as a font");
    assert!(reparsed.num_glyphs() > 0);

    // Subsetting is supposed to be a win: 6 glyphs out of a 162 KB font.
    assert!(
        program.len() < ROBOTO_TTF.len(),
        "subset ({} bytes) is not smaller than the full font ({} bytes)",
        program.len(),
        ROBOTO_TTF.len()
    );
}

/// A CFF/OpenType face must embed as `/FontFile3` + `CIDFontType0`, not `/FontFile2`.
///
/// Regression test: `NotoSansJP-Regular.otf` begins with the `OTTO` magic, so its outlines
/// are CFF — but printpdf used to write it as `CIDFontType2` + `/FontFile2`, both of which
/// mean "TrueType `glyf` outlines". The font dictionary was lying about the bytes it
/// carried. (`ParsedFont::font_type` reports `TrueType` even for OTTO faces, so the
/// subtype is now taken from the sfnt magic of the program actually being embedded.)
#[test]
fn cff_opentype_font_program_is_embedded() {
    let (pdf, warnings) = pdf_with_font(NOTO_JP_OTF, "こんにちは", true);
    assert_no_errors(&warnings, "CFF embed");

    let fonts = fonts_in(&pdf);
    let font = &fonts[0];

    assert_eq!(font.cid_subtype, "CIDFontType0", "CFF => CIDFontType0");
    assert_eq!(
        font.font_file_key.as_deref(),
        Some("FontFile3"),
        "CFF font program belongs in /FontFile3"
    );
    assert!(!font.program().is_empty(), "/FontFile3 stream is empty");
}

// ===========================================================================
// Agreement between the content stream, the font program, /W and /ToUnicode
// ===========================================================================

/// Every glyph the page draws must exist in the font that got embedded.
///
/// Subsetting renumbers glyph ids. If the content stream keeps emitting the *original*
/// ids while the embedded program has been renumbered (Identity-H means CID == GID),
/// every glyph points at the wrong outline — the page renders as garbage even though
/// the PDF is structurally valid and copy/paste still works.
#[test]
fn content_stream_glyph_ids_exist_in_the_embedded_font() {
    for subset in [false, true] {
        let (pdf, warnings) = pdf_with_font(ROBOTO_TTF, LATIN, subset);
        assert_no_errors(&warnings, &format!("subset={subset}"));

        let font = &fonts_in(&pdf)[0];
        let embedded = ParsedFont::from_bytes(font.program(), 0, &mut Vec::new())
            .expect("embedded program must parse");

        let gids = content_stream_gids(&pdf);
        assert_eq!(
            gids.len(),
            LATIN.chars().count(),
            "subset={subset}: expected one glyph per character"
        );

        for gid in &gids {
            assert!(
                *gid != 0,
                "subset={subset}: content stream draws .notdef (gid 0) — the glyph was not \
                 found in the embedded font"
            );
            assert!(
                (*gid as usize) < embedded.num_glyphs() as usize,
                "subset={subset}: content stream draws gid {gid}, but the embedded font only \
                 has {} glyphs",
                embedded.num_glyphs()
            );
        }
    }
}

/// `/ToUnicode` must map the glyph ids the content stream emits back to the *exact*
/// source text.
///
/// This is what makes copy/paste and search work. It is also the assertion the previous
/// test suite got wrong: it accepted either the real text *or* the literal mojibake
/// `"\n\u{6}\u{4}\u{c}\t…"`, which meant a scrambled CMap passed as "fine".
#[test]
fn tounicode_maps_content_glyph_ids_back_to_the_source_text() {
    for text in [LATIN, CYRILLIC] {
        for subset in [false, true] {
            let (pdf, warnings) = pdf_with_font(ROBOTO_TTF, text, subset);
            assert_no_errors(&warnings, &format!("{text:?} subset={subset}"));

            let font = &fonts_in(&pdf)[0];
            let cmap_src = font
                .to_unicode
                .as_ref()
                .unwrap_or_else(|| panic!("{text:?} subset={subset}: no /ToUnicode"));
            let cmap = ToUnicodeCMap::parse(cmap_src)
                .unwrap_or_else(|e| panic!("{text:?} subset={subset}: bad /ToUnicode: {e}"));

            let recovered: String = content_stream_gids(&pdf)
                .iter()
                .map(|gid| {
                    let mapped = cmap.mappings.get(&(*gid as u32)).unwrap_or_else(|| {
                        panic!(
                            "{text:?} subset={subset}: gid {gid} is drawn but absent from /ToUnicode"
                        )
                    });
                    char::from_u32(mapped[0]).expect("valid scalar")
                })
                .collect();

            assert_eq!(
                recovered, text,
                "{text:?} subset={subset}: /ToUnicode does not round-trip the source text — \
                 copy/paste out of this PDF would yield {recovered:?}"
            );
        }
    }
}

/// `/W` must be keyed by the same glyph ids the content stream uses, and the widths must
/// be in PDF glyph space (1/1000 em), not raw font units.
#[test]
fn widths_are_keyed_by_content_glyph_ids_and_normalised() {
    for subset in [false, true] {
        let (pdf, _) = pdf_with_font(ROBOTO_TTF, LATIN, subset);
        let font = &fonts_in(&pdf)[0];

        for gid in content_stream_gids(&pdf) {
            let w = font.width_of(gid).unwrap_or_else(|| {
                panic!("subset={subset}: gid {gid} is drawn but has no /W entry")
            });
            assert!(
                w > 0,
                "subset={subset}: gid {gid} has width {w}; a drawn glyph should advance"
            );
            // Roboto is a 2048-upm font. An unscaled advance would land ~1000-2000+;
            // in 1/1000 em a Latin letter is ~400-700.
            assert!(
                w < 2000,
                "subset={subset}: gid {gid} has width {w} — looks like raw font units, not \
                 1/1000 em (issue #271)"
            );
        }
    }
}

/// FontDescriptor metrics must be normalised to 1/1000 em, and CapHeight must be real.
///
/// Regression test for issue #271 (metrics emitted in raw font units; CapHeight hardcoded
/// to 0).
#[test]
fn font_descriptor_metrics_are_normalised_to_1000_em() {
    let (pdf, _) = pdf_with_font(ROBOTO_TTF, LATIN, true);
    let font = &fonts_in(&pdf)[0];
    let d = &font.descriptor;

    let int = |k: &str| -> i64 {
        d.get(k.as_bytes())
            .unwrap_or_else(|_| panic!("descriptor has no /{k}"))
            .as_i64()
            .unwrap_or_else(|_| panic!("/{k} is not an integer"))
    };

    let ascent = int("Ascent");
    let descent = int("Descent");
    let cap_height = int("CapHeight");

    // Roboto's upm is 2048; raw ascent is ~1946. Scaled it must be ~950.
    assert!(
        (0..=1500).contains(&ascent),
        "/Ascent = {ascent}: not in 1/1000 em (raw font units leaked through — issue #271)"
    );
    assert!(
        (-1000..0).contains(&descent),
        "/Descent = {descent}: expected a small negative value in 1/1000 em"
    );
    assert_ne!(
        cap_height, 0,
        "/CapHeight is 0 — readers use it for fallback metrics (issue #271)"
    );
    assert!(
        (0..=1500).contains(&cap_height),
        "/CapHeight = {cap_height}: not in 1/1000 em"
    );

    let bbox: Vec<i64> = d
        .get(b"FontBBox")
        .expect("/FontBBox")
        .as_array()
        .expect("array")
        .iter()
        .map(|o| o.as_i64().expect("int"))
        .collect();
    assert_eq!(bbox.len(), 4);
    assert!(
        bbox.iter().any(|v| *v != 0),
        "/FontBBox is all zeroes: {bbox:?}"
    );
    assert!(
        bbox[2] > bbox[0] && bbox[3] > bbox[1],
        "/FontBBox is degenerate: {bbox:?}"
    );

    // #271 (residual): these were hardcoded to 0 / 32 / 80 for every font. Readers use
    // them when substituting or synthesising a face, so they have to describe the real one.
    // Roboto Medium is upright (angle 0) and weight 500 => StemV = 50 + (500/65)^2 = 109.
    let stem_v = int("StemV");
    assert_eq!(
        stem_v, 109,
        "/StemV should be derived from OS/2 usWeightClass (500 for Roboto Medium), not the \
         hardcoded 80"
    );
    assert_eq!(int("ItalicAngle"), 0, "Roboto Medium is upright");

    // Nonsymbolic (bit 6 = 32), not italic.
    let flags = int("Flags");
    assert_eq!(flags & (1 << 5), 1 << 5, "/Flags must set Nonsymbolic");
    assert_eq!(flags & (1 << 6), 0, "/Flags must not claim Roboto Medium is italic");
}

/// An italic face must be *described* as italic: readers synthesise a slant from
/// `/ItalicAngle` and `/Flags` when they have to substitute the font.
#[test]
fn italic_font_descriptor_reports_the_slant() {
    const TIMES_ITALIC: &[u8] = include_bytes!("../examples/assets/fonts/Times-Oblique.ttf");

    let (pdf, _) = pdf_with_font(TIMES_ITALIC, "Slanted", true);
    let d = &fonts_in(&pdf)[0].descriptor;

    let int = |k: &str| -> i64 { d.get(k.as_bytes()).unwrap().as_i64().unwrap() };

    let angle = int("ItalicAngle");
    assert!(
        angle < 0,
        "/ItalicAngle is {angle}; an italic face slants right, which is a NEGATIVE angle \
         (it was hardcoded to 0 for every font)"
    );
    assert!(
        (-45..0).contains(&angle),
        "/ItalicAngle {angle} is out of any plausible range"
    );
    assert_eq!(
        int("Flags") & (1 << 6),
        1 << 6,
        "/Flags must set the Italic bit for an italic face"
    );
}

// ===========================================================================
// The safety net: a font that cannot be embedded must fail loudly, not silently
// ===========================================================================

/// If a face somehow reaches serialization without its source bytes, printpdf must
/// report an error and omit `/FontFile2` entirely.
///
/// An absent `/FontFile2` is a legal non-embedded font that readers substitute for. An
/// *empty* `/FontFile2` is a corrupt font they reject outright. 0.10.0 wrote the latter,
/// and said nothing.
#[test]
fn font_without_source_bytes_errors_and_omits_the_font_program() {
    // Construct the pathological case on purpose: azul's own constructor, which does not
    // retain the source bytes, adopted without going through `ParsedFont::from_bytes`.
    let azul = printpdf::font::AzulParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new())
        .expect("parse");
    let font = ParsedFont::from_azul(azul);

    // Precondition for this test to mean anything: on an azul-layout whose `from_bytes`
    // *does* retain bytes, this face is embeddable and there is nothing to assert.
    if font.has_source_bytes() {
        eprintln!("skipped: this azul-layout retains source bytes in from_bytes");
        return;
    }

    let mut doc = PdfDocument::new("no-source-bytes");
    let font_id = doc.add_font(&font);
    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            show_text_ops(&font_id, LATIN),
        )])
        .save(&save_opts(false), &mut warnings);

    let errs = errors(&warnings);
    assert!(
        !errs.is_empty(),
        "a font that cannot be embedded must raise an error, not embed silently"
    );
    assert!(
        errs.iter().any(|e| e.msg.contains("source bytes")),
        "the error should say why: {:#?}",
        errs.iter().map(|e| &e.msg).collect::<Vec<_>>()
    );

    let font = &fonts_in(&pdf)[0];
    assert!(
        font.font_file_key.is_none(),
        "expected NO /FontFile* entry, got {:?} with {} bytes — an empty font program is \
         worse than none: readers reject the whole font",
        font.font_file_key,
        font.font_program.as_ref().map(|p| p.len()).unwrap_or(0)
    );
}

/// `ParsedFont::from_bytes` must always retain the bytes needed for embedding — this is
/// the invariant the whole fix rests on, asserted directly.
#[test]
fn parsed_font_from_bytes_retains_source_bytes() {
    for (name, bytes) in [("Roboto.ttf", ROBOTO_TTF), ("NotoSansJP.otf", NOTO_JP_OTF)] {
        let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("parse");
        assert!(
            font.has_source_bytes(),
            "{name}: ParsedFont::from_bytes dropped the source bytes — every font embedded \
             from it would be an empty /FontFile2 (issue #277)"
        );
        assert_eq!(
            font.source_bytes().unwrap().as_slice(),
            bytes,
            "{name}: retained bytes differ from the input"
        );
    }
}

/// A `PdfFont` that survives a serde round-trip must still be embeddable.
///
/// azul's `Serialize for ParsedFont` writes `to_bytes(None).unwrap_or_default()` — on a
/// face with no retained bytes that silently serializes to an *empty* font. printpdf
/// owns both halves of this so the round-trip cannot quietly hollow a font out.
#[test]
fn serde_roundtrip_keeps_the_font_embeddable() {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");

    let json = serde_json::to_string(&font).expect("serialize");
    let restored: ParsedFont = serde_json::from_str(&json).expect("deserialize");

    assert!(
        restored.has_source_bytes(),
        "a deserialized font lost its source bytes and can no longer be embedded"
    );

    let mut doc = PdfDocument::new("serde");
    let font_id = doc.add_font(&restored);
    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            show_text_ops(&font_id, LATIN),
        )])
        .save(&save_opts(false), &mut warnings);

    assert_no_errors(&warnings, "serde round-trip");
    assert!(!fonts_in(&pdf)[0].program().is_empty());
}

// ===========================================================================
// Built-in fonts, multiple fonts, and the full round-trip
// ===========================================================================

/// Built-in fonts are the 14 standard faces — they must *not* embed a program, and
/// must not be turned into CID fonts.
#[test]
fn builtin_font_is_referenced_not_embedded() {
    let mut doc = PdfDocument::new("builtin");
    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                    size: Pt(24.0),
                },
                Op::ShowText {
                    items: vec![TextItem::Text("Helvetica".to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(&save_opts(false), &mut warnings);
    assert_no_errors(&warnings, "builtin");

    let fonts = fonts_in(&pdf);
    assert_eq!(fonts.len(), 1);
    assert!(
        fonts[0].font_program.is_none(),
        "built-in Helvetica must not embed a font program"
    );
    assert!(fonts[0].base_font.contains("Helvetica"));
}

/// The issue's own repro: a built-in and an external font on the same page. In 0.10.0
/// only Helvetica rendered.
#[test]
fn builtin_and_external_font_on_the_same_page() {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");
    let mut doc = PdfDocument::new("mixed");
    let roboto = doc.add_font(&font);
    let mut warnings = Vec::new();

    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                    size: Pt(24.0),
                },
                Op::ShowText {
                    items: vec![TextItem::Text("Helvetica".to_string())],
                },
                Op::AddLineBreak,
                Op::SetFont {
                    font: PdfFontHandle::External(roboto.clone()),
                    size: Pt(24.0),
                },
                Op::ShowText {
                    items: vec![TextItem::Text(LATIN.to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(&save_opts(false), &mut warnings);
    assert_no_errors(&warnings, "mixed fonts");

    let fonts = fonts_in(&pdf);
    assert_eq!(fonts.len(), 2, "expected both fonts: {fonts:#?}");

    let embedded: Vec<_> = fonts.iter().filter(|f| f.font_program.is_some()).collect();
    assert_eq!(embedded.len(), 1, "exactly one font should be embedded");
    assert!(
        !embedded[0].program().is_empty(),
        "the external font's program is empty — this is the 0.10.0 bug: Helvetica rendered, \
         Roboto did not"
    );
}

/// End-to-end: what printpdf writes, printpdf must be able to read back.
#[test]
fn roundtrip_recovers_font_resource_and_text() {
    for subset in [false, true] {
        let (pdf, _) = pdf_with_font(ROBOTO_TTF, CYRILLIC, subset);

        let mut warnings = Vec::new();
        let parsed = PdfDocument::parse(
            &pdf,
            &PdfParseOptions {
                fail_on_error: false,
            },
            &mut warnings,
        )
        .expect("parse back");

        assert!(
            !parsed.resources.fonts.map.is_empty(),
            "subset={subset}: no font resources came back — the embedded program was not a \
             parseable font"
        );

        let text = parsed.pages[0]
            .extract_text(&parsed.resources)
            .join("");
        assert_eq!(
            text, CYRILLIC,
            "subset={subset}: extracted text does not match what was written"
        );
    }
}


/// printpdf must link exactly ONE copy of the font parser.
///
/// It used to link two: `allsorts-azul 0.17` directly, and `0.16` again underneath
/// `rust-fontconfig`. Two copies of a font parser means two incompatible `ParsedFont`,
/// `CmapTarget`, `FontData` … so a type from one cannot cross into the other, and the
/// binary carries the whole parser twice. rust-fontconfig 4.4.5 moved to 0.17 to fix it;
/// this keeps it fixed.
#[test]
fn only_one_allsorts_is_linked() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["tree", "--features", "html", "--prefix", "none"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();

    let Ok(out) = out else {
        eprintln!("skipping: could not run `cargo tree`");
        return;
    };
    if !out.status.success() {
        eprintln!("skipping: `cargo tree` failed");
        return;
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut versions: Vec<&str> = stdout
        .lines()
        .filter_map(|l| l.strip_prefix("allsorts-azul "))
        .map(|v| v.split_whitespace().next().unwrap_or(v))
        .collect();
    versions.sort_unstable();
    versions.dedup();

    assert_eq!(
        versions.len(),
        1,
        "printpdf links {} copies of allsorts-azul: {versions:?}. Two font parsers means two \
         incompatible sets of font types and a needlessly fat binary — check what pulled the \
         extra one in with `cargo tree -i allsorts-azul@<version>`.",
        versions.len()
    );
}
