//! Verification against *external* PDF tools (poppler).
//!
//! Everything else in this repo checks printpdf against printpdf. That is exactly how
//! issue #277 escaped: the writer emitted an empty `/FontFile2`, the reader was happy to
//! read a font resource back, and the round-trip test went green while Acrobat refused
//! the file outright.
//!
//! These tests use `pdffonts` and `pdftotext` (poppler) as an oracle that shares no code
//! with printpdf. They answer the two questions a user actually cares about:
//!
//!   1. **Does the font embed cleanly?** — `pdffonts` prints
//!      `Syntax Error: Embedded font file may be invalid` when it cannot parse the
//!      embedded program. That single line is the entire content of issue #277.
//!   2. **Is the text copy-able?** — `pdftotext` extracts text the same way a reader's
//!      copy/paste and a search engine's indexer do, by inverting `/ToUnicode`. If the
//!      CMap is wrong, the page can *look* perfect and still yield mojibake.
//!
//! The tests skip themselves when poppler is not installed, so a bare `cargo test` still
//! works; CI installs `poppler-utils` so they always run there.

#![cfg(feature = "text_layout")]

use std::{
    io::Write,
    process::{Command, Stdio},
};

use printpdf::{
    graphics::Point,
    ops::PdfFontHandle,
    units::{Mm, Pt},
    BuiltinFont, FontId, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, TextItem,
};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
const NOTO_JP_OTF: &[u8] = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");

// ---------------------------------------------------------------------------
// poppler plumbing
// ---------------------------------------------------------------------------

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Run a poppler tool with the PDF on stdin, returning (stdout, stderr).
///
/// Callers pass the trailing file arguments themselves: poppler spells "read stdin" as a
/// `-` in the input-file slot, and `pdftotext` needs a *second* `-` for the output slot,
/// or it writes to `<stdin>.txt` on disk and prints nothing.
fn poppler(tool: &str, pdf: &[u8], args: &[&str]) -> (String, String) {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {tool}: {e}"));

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(pdf)
        .expect("write pdf to stdin");

    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// `pdffonts` — one row per font, plus any syntax errors on stderr.
struct PdfFontsReport {
    stderr: String,
    rows: Vec<PdfFontsRow>,
}

#[derive(Debug)]
struct PdfFontsRow {
    name: String,
    embedded: bool,
    has_unicode: bool,
}

impl PdfFontsReport {
    fn run(pdf: &[u8]) -> Self {
        // `pdffonts -` : read the PDF from stdin, report on stdout.
        let (stdout, stderr) = poppler("pdffonts", pdf, &["-"]);
        // Columns: name type encoding emb sub uni object ID
        let rows = stdout
            .lines()
            .skip_while(|l| !l.starts_with("---"))
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let cols: Vec<&str> = l.split_whitespace().collect();
                // Walk from the right: object ID (2 cols), uni, sub, emb.
                let n = cols.len();
                PdfFontsRow {
                    name: cols[0].to_string(),
                    embedded: cols.get(n - 5).map(|c| *c == "yes").unwrap_or(false),
                    has_unicode: cols.get(n - 3).map(|c| *c == "yes").unwrap_or(false),
                }
            })
            .collect();
        Self { stderr, rows }
    }

    /// The line from issue #277. Poppler prints it when the `/FontFile*` stream is not a
    /// font it can parse — which an empty stream certainly is not.
    fn assert_no_font_errors(&self, ctx: &str) {
        assert!(
            !self.stderr.contains("Embedded font file may be invalid"),
            "{ctx}: pdffonts rejected the embedded font program.\n\
             This is issue #277 — the PDF is structurally valid but no reader can render \
             the text.\npdffonts stderr:\n{}",
            self.stderr.trim()
        );
        assert!(
            !self.stderr.contains("Syntax Error"),
            "{ctx}: pdffonts reported a syntax error:\n{}",
            self.stderr.trim()
        );
    }
}

/// `pdftotext` — this is literally what copy/paste and search do.
fn extract_text(pdf: &[u8]) -> String {
    // `pdftotext - -` : PDF from stdin, extracted text to stdout.
    let (stdout, _) = poppler("pdftotext", pdf, &["-", "-"]);
    // pdftotext ends each page with a form feed.
    stdout.replace('\u{c}', "").trim().to_string()
}

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

fn text_ops(font: PdfFontHandle, text: &str) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetFont {
            font,
            size: Pt(24.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

fn pdf_with_external_font(font_bytes: &[u8], text: &str, subset: bool) -> Vec<u8> {
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).expect("parse font");
    let mut doc = PdfDocument::new("external-tools-test");
    let id: FontId = doc.add_font(&font);
    doc.with_pages(vec![PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        text_ops(PdfFontHandle::External(id), text),
    )])
    .save(&save_opts(subset), &mut Vec::new())
}

fn pdf_with_builtin_font(builtin: BuiltinFont, text: &str) -> Vec<u8> {
    let mut doc = PdfDocument::new("external-tools-builtin");
    doc.with_pages(vec![PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        text_ops(PdfFontHandle::Builtin(builtin), text),
    )])
    .save(&save_opts(false), &mut Vec::new())
}

macro_rules! require_poppler {
    ($tool:literal) => {
        if !have($tool) {
            eprintln!("skipping: {} not installed (install poppler-utils)", $tool);
            return;
        }
    };
}

// ===========================================================================
// pdffonts: does the font program embed cleanly?
// ===========================================================================

/// The exact reproduction from issue #277, checked with the exact tool the reporter used.
///
/// 0.10.0 output:
/// ```text
/// Syntax Error: Embedded font file may be invalid
/// name                  type          encoding    emb sub uni
/// HEIGID+Roboto-Medium  CID TrueType  Identity-H  yes yes yes
/// ```
#[test]
fn pdffonts_accepts_the_embedded_truetype_program() {
    require_poppler!("pdffonts");

    for subset in [false, true] {
        let pdf = pdf_with_external_font(ROBOTO_TTF, "Roboto", subset);
        let report = PdfFontsReport::run(&pdf);
        report.assert_no_font_errors(&format!("Roboto subset={subset}"));

        let row = report
            .rows
            .iter()
            .find(|r| r.name.contains("Roboto"))
            .unwrap_or_else(|| panic!("subset={subset}: Roboto not listed by pdffonts"));

        assert!(row.embedded, "subset={subset}: pdffonts says Roboto is not embedded");
        assert!(
            row.has_unicode,
            "subset={subset}: pdffonts says Roboto has no Unicode map — text will not be \
             copy-able"
        );
    }
}

#[test]
fn pdffonts_accepts_the_embedded_cff_program() {
    require_poppler!("pdffonts");

    for subset in [false, true] {
        let pdf = pdf_with_external_font(NOTO_JP_OTF, "こんにちは", subset);
        PdfFontsReport::run(&pdf)
            .assert_no_font_errors(&format!("NotoSansJP (CFF) subset={subset}"));
    }
}

// ===========================================================================
// pdftotext: is the text actually copy-able?
// ===========================================================================

/// Round-trip through poppler's text extractor — the same path as Ctrl+C in a reader.
///
/// This catches a whole class of bugs that leave the *rendering* perfect: a `/ToUnicode`
/// CMap keyed by the wrong glyph ids, subset renumbering applied to the CMap but not the
/// content stream (or vice versa), or a missing CMap entirely.
#[test]
fn pdftotext_roundtrips_external_font_text() {
    require_poppler!("pdftotext");

    let cases = [
        ("latin", "Roboto"),
        ("latin sentence", "The quick brown fox jumps over the lazy dog"),
        ("cyrillic", "Привет, как дела?"),
        ("digits+punct", "Invoice #1234 — total: 56.78 EUR"),
        ("accented", "Grüße aus Köln, naïve café"),
    ];

    for (label, text) in cases {
        for subset in [false, true] {
            let pdf = pdf_with_external_font(ROBOTO_TTF, text, subset);
            let got = extract_text(&pdf);
            assert_eq!(
                got, text,
                "{label} (subset={subset}): pdftotext round-trip failed.\n  wrote: {text:?}\n  \
                 read : {got:?}\nText in this PDF is not copy-able."
            );
        }
    }
}

#[test]
fn pdftotext_roundtrips_cjk_text() {
    require_poppler!("pdftotext");

    let text = "こんにちは世界";
    for subset in [false, true] {
        let pdf = pdf_with_external_font(NOTO_JP_OTF, text, subset);
        let got = extract_text(&pdf);
        assert_eq!(
            got, text,
            "CJK (subset={subset}): pdftotext round-trip failed.\n  wrote: {text:?}\n  read : \
             {got:?}"
        );
    }
}

/// Built-in (non-embedded) fonts must also produce copy-able text.
///
/// The 14 standard fonts are declared `WinAnsiEncoding`, so the content stream has to
/// carry *WinAnsi* bytes. Emitting raw UTF-8 into them makes every non-ASCII character
/// come back as mojibake (issue #273).
#[test]
fn pdftotext_roundtrips_builtin_font_text() {
    require_poppler!("pdftotext");

    for text in ["Helvetica", "The quick brown fox"] {
        let pdf = pdf_with_builtin_font(BuiltinFont::Helvetica, text);
        let got = extract_text(&pdf);
        assert_eq!(
            got, text,
            "builtin ASCII: pdftotext round-trip failed.\n  wrote: {text:?}\n  read : {got:?}"
        );
    }
}

/// Issue #273: non-ASCII text in a built-in font.
///
/// Regression test. This used to extract as mojibake:
///
/// ```text
/// wrote: "Grüße aus Köln"
/// read : "GrÃ¼ÃŸe aus KÃ¶ln"
/// ```
///
/// `WinAnsiEncoding` is a single-byte encoding that *does* contain ä/ö/ü/é/£, so this text
/// is perfectly representable and must round-trip. printpdf was writing the raw UTF-8
/// bytes, so `ü` (U+00FC, UTF-8 `C3 BC`) arrived as the two WinAnsi characters `Ã¼`. The
/// cause was `Encoding::SimpleEncoding(b"WinAnsiEncoding")`, a name lopdf 0.39 does not
/// recognise — it silently fell through to `text.as_bytes()`.
#[test]
fn pdftotext_roundtrips_builtin_font_non_ascii() {
    require_poppler!("pdftotext");

    let text = "Grüße aus Köln";
    let pdf = pdf_with_builtin_font(BuiltinFont::Helvetica, text);
    let got = extract_text(&pdf);
    assert_eq!(
        got, text,
        "builtin non-ASCII (issue #273): pdftotext round-trip failed.\n  wrote: {text:?}\n  \
         read : {got:?}\nA built-in font declares WinAnsiEncoding; the content stream must \
         carry WinAnsi bytes, not raw UTF-8."
    );
}

/// Both fonts on one page, the layout from the issue. In 0.10.0 only Helvetica rendered.
#[test]
fn pdftotext_roundtrips_builtin_and_external_together() {
    require_poppler!("pdftotext");

    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");
    let mut doc = PdfDocument::new("mixed");
    let roboto = doc.add_font(&font);

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
                    font: PdfFontHandle::External(roboto),
                    size: Pt(24.0),
                },
                Op::ShowText {
                    items: vec![TextItem::Text("Roboto".to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(&save_opts(false), &mut Vec::new());

    PdfFontsReport::run(&pdf).assert_no_font_errors("mixed builtin+external");

    let got = extract_text(&pdf);
    for want in ["Helvetica", "Roboto"] {
        assert!(
            got.contains(want),
            "mixed fonts: {want:?} missing from extracted text {got:?}"
        );
    }
}

/// The `'` and `"` operators show text exactly like `Tj` does, so they must encode for the
/// selected font too. They used to emit `text.as_bytes()` unconditionally — raw UTF-8,
/// which is wrong for a WinAnsi built-in font *and* wrong for an Identity-H external font
/// (where the bytes have to be glyph ids).
#[test]
fn pdftotext_roundtrips_quote_operators() {
    require_poppler!("pdftotext");

    // Built-in font: bytes must be WinAnsi.
    let mut doc = PdfDocument::new("quote-ops-builtin");
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
                Op::SetLineHeight { lh: Pt(28.0) },
                // `'` moves to the *next* line before showing, so start near the top of the
                // page — from the default origin it would draw below y=0, off the page.
                Op::SetTextCursor {
                    pos: Point {
                        x: Mm(20.0).into(),
                        y: Mm(250.0).into(),
                    },
                },
                Op::MoveToNextLineShowText {
                    text: "Café Zürich".to_string(),
                },
                Op::SetSpacingMoveAndShowText {
                    word_spacing: 0.0,
                    char_spacing: 0.0,
                    text: "Größe".to_string(),
                },
                Op::EndTextSection,
            ],
        )])
        .save(&save_opts(false), &mut Vec::new());

    let got = extract_text(&pdf);
    for want in ["Café Zürich", "Größe"] {
        assert!(
            got.contains(want),
            "built-in `'`/`\"` operators: {want:?} missing from extracted text {got:?}"
        );
    }

    // External font: bytes must be Identity-H glyph ids, not UTF-8.
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");
    let mut doc = PdfDocument::new("quote-ops-external");
    let id = doc.add_font(&font);
    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::External(id),
                    size: Pt(24.0),
                },
                Op::SetLineHeight { lh: Pt(28.0) },
                Op::SetTextCursor {
                    pos: Point {
                        x: Mm(20.0).into(),
                        y: Mm(250.0).into(),
                    },
                },
                Op::MoveToNextLineShowText {
                    text: "Grüße".to_string(),
                },
                Op::EndTextSection,
            ],
        )])
        .save(&save_opts(true), &mut Vec::new());

    PdfFontsReport::run(&pdf).assert_no_font_errors("external `'` operator");
    let got = extract_text(&pdf);
    assert_eq!(
        got, "Grüße",
        "external `'` operator: expected glyph-id encoding, got {got:?}"
    );
}

/// Text split across many `ShowText` ops and pages must still extract in order.
#[test]
fn pdftotext_roundtrips_multipage() {
    require_poppler!("pdftotext");

    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");
    let mut doc = PdfDocument::new("multipage");
    let id = doc.add_font(&font);

    let pages: Vec<PdfPage> = ["Page one text", "Page two text", "Page three text"]
        .iter()
        .map(|t| {
            PdfPage::new(
                Mm(210.0),
                Mm(297.0),
                text_ops(PdfFontHandle::External(id.clone()), t),
            )
        })
        .collect();

    let pdf = doc.with_pages(pages).save(&save_opts(true), &mut Vec::new());
    PdfFontsReport::run(&pdf).assert_no_font_errors("multipage");

    let got = extract_text(&pdf);
    for want in ["Page one text", "Page two text", "Page three text"] {
        assert!(got.contains(want), "multipage: {want:?} missing from {got:?}");
    }
}
