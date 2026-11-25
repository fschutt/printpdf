//! `shape_line`: one line of text shaped in one font, without HTML, and drawn through
//! `show_placed_glyphs`.
//!
//! The mock font (see `scripts/gen_mock_fonts.py`) has 1000 units/em, gid 1 = space (250), gid 2.. =
//! 'A'.. with advance 300 + 50 i, no kerning. Kerning and ligatures come from RobotoMedium, and
//! right-to-left text from the Vazirmatn test font, whose glyph ids for `ARABIC` are the ones
//! HarfBuzz gives (see `tests/arabic_shaping.rs`). This file runs without the `html` feature.
#![cfg(feature = "text_layout")]

use printpdf::*;

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");
const MOCK_CFF: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_cid.otf");
const ROBOTO: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
const VAZIRMATN: &[u8] = include_bytes!("./assets/fonts/arabic/Vazirmatn-ArabicTest.ttf");

/// "Hello world" and the word "no" (the lam-alef ligature).
const ARABIC: &str = "مرحبا بالعالم لا";

fn font(bytes: &[u8]) -> ParsedFont {
    ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("font parses")
}

fn texts(line: &PlacedLine) -> Vec<&str> {
    line.glyphs.iter().map(|g| g.text.as_str()).collect()
}

fn gids(line: &PlacedLine) -> Vec<u16> {
    line.glyphs.iter().map(|g| g.gid).collect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn latin_advances_follow_the_font() {
    let mock = font(MOCK_TTF);
    // at 10 pt: A 3, B 3.5, space 2.5, C 4
    let line = shape_line(&mock, "AB C", Pt(10.0)).expect("shapes");
    assert_eq!(gids(&line), [2, 3, 1, 4]);
    assert_eq!(texts(&line), ["A", "B", " ", "C"]);
    let xs: Vec<f32> = line.glyphs.iter().map(|g| g.x).collect();
    for (x, expected) in xs.iter().zip([0.0, 3.0, 6.5, 9.0]) {
        assert!(close(*x, expected), "{xs:?}");
    }
    assert!(line.glyphs.iter().all(|g| g.font == 0 && g.y == 0.0));
    assert!(close(line.width.0, 13.0), "{:?}", line.width);

    // positions and width scale with the size
    let twice = shape_line(&mock, "AB C", Pt(20.0)).expect("shapes");
    assert!(close(twice.width.0, 26.0), "{:?}", twice.width);
    assert!(close(twice.glyphs[3].x, 18.0));
}

#[test]
fn a_character_the_font_lacks_is_its_notdef_glyph_with_its_text() {
    let line = shape_line(&font(MOCK_TTF), "AZ", Pt(10.0)).expect("shapes");
    assert_eq!(gids(&line), [2, 0]);
    assert_eq!(texts(&line), ["A", "Z"]);
    // .notdef is 500 wide
    assert!(close(line.width.0, 8.0), "{:?}", line.width);
}

#[test]
fn a_tab_draws_nothing_and_moves_the_pen() {
    let line = shape_line(&font(MOCK_TTF), "A\tB", Pt(10.0)).expect("shapes");
    assert_eq!(texts(&line), ["A", "B"]);
    assert!(line.glyphs[1].x > 3.0, "{line:?}");
}

#[test]
fn kerning_moves_a_pair_together() {
    let roboto = font(ROBOTO);
    let size = Pt(100.0);
    let a = shape_line(&roboto, "A", size).expect("shapes");
    let av = shape_line(&roboto, "AV", size).expect("shapes");
    assert_eq!(av.glyphs.len(), 2);
    // V starts before A's own advance has passed: the pair is kerned
    assert!(av.glyphs[1].x < a.width.0 - 1.0, "A is {:?} wide, V starts at {}", a.width, av.glyphs[1].x);
    // the line is shorter than the two letters set apart
    let v = shape_line(&roboto, "V", size).expect("shapes");
    assert!(av.width.0 < a.width.0 + v.width.0 - 1.0, "{av:?}");
}

#[test]
fn ligatures_carry_the_text_of_all_their_letters() {
    let line = shape_line(&font(ROBOTO), "fi ffl", Pt(20.0)).expect("shapes");
    let joined: String = texts(&line).concat();
    assert_eq!(joined, "fi ffl");
    assert!(line.glyphs.len() < 6, "the font joins f and i: {line:?}");
    assert_eq!(line.glyphs[0].text, "fi");
}

#[test]
fn arabic_is_joined_and_in_visual_order() {
    let vazir = font(VAZIRMATN);
    let line = shape_line(&vazir, ARABIC, Pt(20.0)).expect("shapes");
    // HarfBuzz's glyphs for this text, left to right
    let expected = [29, 2, 26, 24, 12, 22, 24, 12, 14, 2, 12, 15, 17, 19, 27];
    assert_eq!(gids(&line), expected);
    // the first letter of the text is the last glyph, the lam-alef ligature stands for two
    assert_eq!(line.glyphs[0].text, "لا");
    assert_eq!(line.glyphs[14].text, "م");
    // left to right with the pen moving on
    assert!(line.glyphs.windows(2).all(|w| w[0].x < w[1].x), "{line:?}");
    assert_eq!(line.glyphs[0].x, 0.0);
    assert!(line.width.0 > line.glyphs[14].x);
    // reading the clusters from the end gives the text back
    let logical: String = texts(&line).into_iter().rev().collect();
    assert_eq!(logical, ARABIC);
}

#[test]
fn mixed_direction_runs_keep_their_own_order() {
    let vazir = font(VAZIRMATN);
    // the first strong letter is Arabic: the line reads right to left, "ab" stays left to right
    // and is the leftmost word
    let rtl = shape_line(&vazir, "مرحبا ab", Pt(20.0)).expect("shapes");
    let t = texts(&rtl);
    assert_eq!(&t[..3], ["a", "b", " "], "{t:?}");
    assert_eq!(t.last(), Some(&"م"));

    // a Latin word between two Arabic words: Arabic on both sides, the last word first
    let middle = shape_line(&vazir, "مرحبا ab مرحبا", Pt(20.0)).expect("shapes");
    let t = texts(&middle);
    let a = t.iter().position(|s| *s == "a").expect("an a");
    assert_eq!(t.get(a + 1), Some(&"b"), "{t:?}");
    assert!(a >= 5 && t.len() - a - 2 >= 5, "Arabic on both sides: {t:?}");
    assert!(middle.glyphs.windows(2).all(|w| w[0].x < w[1].x), "{middle:?}");

    // the first strong letter is Latin: the line reads left to right, the Arabic word is
    // reversed inside it
    let ltr = shape_line(&vazir, "ab مرحبا", Pt(20.0)).expect("shapes");
    let t = texts(&ltr);
    assert_eq!(&t[..3], ["a", "b", " "], "{t:?}");
    assert_eq!(t.last(), Some(&"م"));
}

#[test]
fn text_that_is_one_cluster_is_carried_once() {
    // e with a combining acute: the letter carries the cluster's text, any mark glyph none
    let line = shape_line(&font(ROBOTO), "e\u{301}x", Pt(20.0)).expect("shapes");
    assert_eq!(texts(&line).concat(), "e\u{301}x");
}

#[test]
fn empty_text_is_an_empty_line() {
    let line = shape_line(&font(MOCK_TTF), "", Pt(10.0)).expect("empty is fine");
    assert!(line.is_empty());
    assert_eq!(line, PlacedLine::empty());
    assert_eq!(line.width, Pt(0.0));
    let ops = show_placed_glyphs(&[(FontId("F1".into()), font(MOCK_TTF).as_azul())], Pt(10.0), &line.glyphs, None);
    assert!(ops.is_empty());
}

#[test]
fn a_line_break_is_an_error() {
    let mock = font(MOCK_TTF);
    for text in ["A\nB", "A\r\nB", "A\rB", "\u{2028}", "A\u{85}", "\u{0B}", "\u{0C}", "A\u{2029}B"] {
        assert_eq!(shape_line(&mock, text, Pt(10.0)), Err(ShapeLineError::LineBreak), "{text:?}");
    }
}

#[test]
fn over_long_text_is_an_error() {
    let mock = font(MOCK_TTF);
    let long = "A".repeat(MAX_LINE_BYTES + 1);
    assert_eq!(
        shape_line(&mock, &long, Pt(10.0)),
        Err(ShapeLineError::TooLong { len: MAX_LINE_BYTES + 1, max: MAX_LINE_BYTES })
    );
    // the limit is bytes: a text of 2-byte letters that is short in letters is long in bytes
    let letters = "ب".repeat(MAX_LINE_BYTES / 2 + 1);
    assert!(letters.chars().count() <= MAX_LINE_BYTES);
    assert!(matches!(shape_line(&font(VAZIRMATN), &letters, Pt(10.0)), Err(ShapeLineError::TooLong { .. })));
    // a text of exactly the limit is shaped
    let at_limit = shape_line(&mock, &"A".repeat(MAX_LINE_BYTES), Pt(10.0)).expect("shapes");
    assert_eq!(at_limit.glyphs.len(), MAX_LINE_BYTES);
    // a line well inside the limit works
    let ok = shape_line(&mock, &"AB ".repeat(3000), Pt(10.0)).expect("shapes");
    assert_eq!(ok.glyphs.len(), 9000);
    // 3000 times A (3 pt), B (3.5 pt) and a space (2.5 pt)
    assert!((ok.width.0 - 27_000.0).abs() < 1.0, "{:?}", ok.width);
}

#[test]
fn a_size_that_is_not_usable_is_an_error() {
    let mock = font(MOCK_TTF);
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, MAX_LINE_FONT_SIZE * 2.0] {
        assert_eq!(shape_line(&mock, "A", Pt(size)), Err(ShapeLineError::InvalidSize), "{size}");
        // the size is checked before the text
        assert_eq!(shape_line(&mock, "", Pt(size)), Err(ShapeLineError::InvalidSize), "{size}");
    }
    assert!(shape_line(&mock, "A", Pt(MAX_LINE_FONT_SIZE)).is_ok());
    assert!(shape_line(&mock, "A", Pt(0.001)).is_ok());
}

#[test]
fn other_font_formats_and_odd_text_do_not_panic() {
    let cff = font(MOCK_CFF);
    let line = shape_line(&cff, "ABC", Pt(10.0)).expect("a CFF font shapes");
    assert_eq!(line.glyphs.len(), 3);
    let mock = font(MOCK_TTF);
    // text of other scripts, a lone mark, a lone surrogate-range character, a zero-width joiner,
    // emoji outside the BMP and a NUL
    for text in ["\u{301}", "日本語", "\u{200D}", "😀A", "\0", "\u{FFFD}", "ا", "\u{202E}AB", "A\u{200F}B"] {
        let r = shape_line(&mock, text, Pt(10.0));
        assert!(r.is_ok(), "{text:?}: {r:?}");
        let line = r.expect("ok");
        assert!(line.width.0.is_finite() && line.width.0 >= 0.0);
        assert!(line.glyphs.iter().all(|g| g.x.is_finite() && g.y.is_finite()));
    }
}

#[test]
fn a_shaper_gives_what_shape_line_gives() {
    let roboto = font(ROBOTO);
    let shaper = LineShaper::new(&roboto);
    for text in ["Voyage", "AV To", "fi"] {
        assert_eq!(shaper.shape(text, Pt(14.0)), shape_line(&roboto, text, Pt(14.0)), "{text:?}");
    }
    // the shaper can be used again and cloned
    let again = shaper.clone();
    assert_eq!(again.shape("Voyage", Pt(14.0)), shaper.shape("Voyage", Pt(14.0)));
}

#[test]
fn errors_read_as_sentences() {
    for e in [
        ShapeLineError::TooLong { len: 70_000, max: MAX_LINE_BYTES },
        ShapeLineError::LineBreak,
        ShapeLineError::InvalidSize,
        ShapeLineError::Shaping,
    ] {
        assert!(!e.to_string().is_empty());
        let _: &dyn std::error::Error = &e;
    }
}

/// A one-page PDF with `line` drawn at (50, 700) with the placed-glyph operations, optionally
/// inside an `/ActualText` span of `actual_text`.
fn page_with(font: &ParsedFont, line: &PlacedLine, size: Pt, actual_text: Option<&str>) -> Vec<u8> {
    let mut doc = PdfDocument::new("shape_line");
    let id = doc.add_font(font);
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextMatrix { matrix: TextMatrix::Translate(Pt(50.0), Pt(700.0)) },
    ];
    ops.extend(show_placed_glyphs(&[(id, font.as_azul())], size, &line.glyphs, actual_text));
    ops.push(Op::EndTextSection);
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut Vec::new())
}

fn extracted(pdf: &[u8]) -> String {
    let parsed = PdfDocument::parse(pdf, &PdfParseOptions::default(), &mut Vec::new()).expect("parses");
    parsed.extract_text().concat().concat()
}

#[test]
fn a_shaped_latin_line_reads_back_as_its_text() {
    let mock = font(MOCK_TTF);
    let text = "ABC AB CD";
    let line = shape_line(&mock, text, Pt(10.0)).expect("shapes");
    let pdf = page_with(&mock, &line, Pt(10.0), None);
    assert_eq!(extracted(&pdf), text);
    if let Some(t) = poppler_text(&pdf) {
        assert_eq!(t.trim(), text, "pdftotext read {t:?}");
    }
}

#[test]
fn a_shaped_arabic_line_reads_back_as_its_text() {
    let vazir = font(VAZIRMATN);
    let line = shape_line(&vazir, ARABIC, Pt(20.0)).expect("shapes");
    // the glyphs are drawn left to right in visual order; the span says what they read as
    let pdf = page_with(&vazir, &line, Pt(20.0), Some(ARABIC));
    // printpdf's own extraction reads the glyphs' /ToUnicode text, in the order they are drawn
    let visual: String = texts(&line).concat();
    assert_eq!(extracted(&pdf), visual);
    // the span carries the logical text for readers that honour /ActualText
    assert_eq!(actual_texts(&pdf), [ARABIC]);
    // poppler orders right-to-left text by its own rules (and wraps it in direction marks), so
    // only the letters are compared
    poppler_letters(&pdf, ARABIC);
}

/// The width a viewer gives the line is the one `shape_line` measured: the glyph drawn last
/// starts where the shaper put it.
#[test]
fn the_glyphs_land_where_the_shaper_put_them() {
    let mock = font(MOCK_TTF);
    let line = shape_line(&mock, "AB C", Pt(10.0)).expect("shapes");
    let ops = show_placed_glyphs(&[(FontId("F1".into()), mock.as_azul())], Pt(10.0), &line.glyphs, None);
    // glyphs at their natural places need no kerning numbers
    let [Op::SetFont { .. }, Op::ShowText { items }] = ops.as_slice() else { panic!("{ops:#?}") };
    let [TextItem::GlyphIds(cps)] = items.as_slice() else { panic!("{items:#?}") };
    assert_eq!(cps.len(), 4);
    assert!(cps.iter().all(|cp| cp.offset.abs() < 0.5), "{cps:#?}");
}

/// The `/ActualText` of every span on the first page
fn actual_texts(pdf: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(pdf).expect("PDF parses");
    let (_, page) = doc.get_pages().into_iter().next().expect("one page");
    let ops = doc.get_and_decode_page_content(page).expect("content parses").operations;
    ops.iter()
        .filter(|op| op.operator == "BDC")
        .filter_map(|op| match op.operands.get(1) {
            Some(lopdf::Object::Dictionary(d)) => d.get(b"ActualText").ok().cloned(),
            _ => None,
        })
        .filter_map(|text| match text {
            lopdf::Object::String(bytes, _) => {
                let units: Vec<u16> = bytes
                    .get(2..)?
                    .chunks(2)
                    .filter_map(|c| Some(u16::from_be_bytes([*c.first()?, *c.get(1)?])))
                    .collect();
                String::from_utf16(&units).ok()
            }
            _ => None,
        })
        .collect()
}

/// What `pdftotext` finds on the page, or None when poppler is not installed or fails.
fn poppler_text(pdf: &[u8]) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("printpdf-shape-line-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{:x}.pdf", pdf.len()));
    let out = std::fs::write(&path, pdf).ok().and_then(|()| {
        std::process::Command::new("pdftotext").arg("-enc").arg("UTF-8").arg(&path).arg("-").output().ok()
    });
    let _ = std::fs::remove_dir_all(&dir);
    match out {
        Some(out) if out.status.success() => Some(String::from_utf8_lossy(&out.stdout).into_owned()),
        _ => {
            eprintln!("pdftotext is not available, skipping the poppler check");
            None
        }
    }
}

/// Checks that `pdftotext` finds the same letters as `expected`, in any order.
fn poppler_letters(pdf: &[u8], expected: &str) {
    let Some(text) = poppler_text(pdf) else { return };
    let letters = |s: &str| {
        let mut v: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
        v.sort_unstable();
        v
    };
    assert_eq!(letters(&text), letters(expected), "pdftotext read {text:?}");
}
