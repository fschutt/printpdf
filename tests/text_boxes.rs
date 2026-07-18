//! Selection-box (hOCR-style) extraction tests against the deterministic mock
//! fonts: every advance is a defined constant (space 250, A..J = 300+50i at
//! 1000 upm), so box geometry is asserted as arithmetic — no reference
//! renderer needed.

#![cfg(feature = "text_layout")]

use printpdf::{
    ops::PdfFontHandle,
    text_boxes::PageTextBoxes,
    units::{Mm, Pt},
    Op, ParsedFont, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions, Point, TextItem,
};

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");
const MOCK_CFF_CID: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_cid.otf");

const MM_TO_PT: f32 = 72.0 / 25.4;
const EPS: f32 = 0.1;

/// Defined advance of a mock-font char at 1000 upm, in em.
fn adv_em(c: char) -> f32 {
    match c {
        ' ' => 0.25,
        'A'..='J' => (300 + 50 * (c as u32 - 'A' as u32)) as f32 / 1000.0,
        _ => panic!("no metrics for {c:?}"),
    }
}

fn assert_close(a: f32, b: f32, ctx: &str) {
    assert!((a - b).abs() < EPS, "{ctx}: {a} != {b} (±{EPS})");
}

/// Build a one-page doc showing "AB CJ" at `size` pt with the cursor at
/// (20mm, 250mm), and return the doc.
fn doc_with_text(font_bytes: &[u8], size: f32) -> PdfDocument {
    let mut doc = PdfDocument::new("text boxes");
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).unwrap();
    let font_id = doc.add_font(&font);
    let ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Mm(20.0).into(),
                y: Mm(250.0).into(),
            },
        },
        Op::SetFont {
            font: PdfFontHandle::External(font_id),
            size: Pt(size),
        },
        Op::ShowText {
            items: vec![TextItem::Text("AB CJ".to_string())],
        },
        Op::EndTextSection,
    ];
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.pages.push(page);
    doc
}

fn assert_ab_cj_geometry(boxes: &PageTextBoxes, size: f32, ctx: &str) {
    let page_h = 297.0 * MM_TO_PT;
    let x0 = 20.0 * MM_TO_PT;
    let baseline = page_h - 250.0 * MM_TO_PT;

    assert_close(boxes.width, 210.0 * MM_TO_PT, &format!("{ctx}: page width"));
    assert_close(boxes.height, page_h, &format!("{ctx}: page height"));
    assert_eq!(boxes.lines.len(), 1, "{ctx}: one line");
    let line = &boxes.lines[0];
    assert_close(line.baseline, baseline, &format!("{ctx}: baseline"));
    assert_eq!(line.words.len(), 2, "{ctx}: two words: {line:#?}");

    let w1 = &line.words[0];
    let w2 = &line.words[1];
    assert_eq!(w1.text, "AB", "{ctx}");
    assert_eq!(w2.text, "CJ", "{ctx}");
    assert_close(w1.font_size, size, &format!("{ctx}: font size"));

    // Word 1 "AB": starts at the cursor, A=0.30em + B=0.35em wide.
    assert_close(w1.bbox[0], x0, &format!("{ctx}: AB x0"));
    assert_close(
        w1.bbox[2],
        x0 + (adv_em('A') + adv_em('B')) * size,
        &format!("{ctx}: AB x1"),
    );

    // Word 2 "CJ": after A+B+space; C=0.40em + J=0.75em wide.
    let w2_x0 = x0 + (adv_em('A') + adv_em('B') + adv_em(' ')) * size;
    assert_close(w2.bbox[0], w2_x0, &format!("{ctx}: CJ x0"));
    assert_close(
        w2.bbox[2],
        w2_x0 + (adv_em('C') + adv_em('J')) * size,
        &format!("{ctx}: CJ x1"),
    );

    // Vertical extent: mock fonts declare ascent 800 / descent -200.
    assert_close(w1.bbox[1], baseline - 0.8 * size, &format!("{ctx}: AB top"));
    assert_close(w1.bbox[3], baseline + 0.2 * size, &format!("{ctx}: AB bottom"));

    // Glyph boxes: 2 per word, each exactly the defined advance wide.
    assert_eq!(w1.glyphs.len(), 2, "{ctx}");
    assert_close(
        w1.glyphs[0].bbox[2] - w1.glyphs[0].bbox[0],
        adv_em('A') * size,
        &format!("{ctx}: glyph A width"),
    );
    assert_eq!(w1.glyphs[0].text, "A", "{ctx}");
}

#[test]
fn mock_ttf_boxes_match_defined_metrics() {
    let doc = doc_with_text(MOCK_TTF, 10.0);
    let boxes = doc.extract_text_boxes();
    assert_eq!(boxes.len(), 1);
    assert_ab_cj_geometry(&boxes[0], 10.0, "ttf/direct");
}

/// The same geometry must survive save + parse: the parsed ops arrive as
/// GlyphIds (with the #280 CID→GID resolution applied for the CID-keyed CFF),
/// and advances come from the re-parsed embedded font.
#[test]
fn boxes_survive_roundtrip_ttf_and_cid_cff() {
    for (bytes, name) in [(MOCK_TTF, "ttf"), (MOCK_CFF_CID, "cff-cid")] {
        for subset in [false, true] {
            let doc = doc_with_text(bytes, 10.0);
            let mut warnings = Vec::new();
            let pdf = doc.save(
                &PdfSaveOptions {
                    subset_fonts: subset,
                    optimize: false,
                    ..Default::default()
                },
                &mut warnings,
            );
            let parsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings)
                .expect("roundtrip parse");
            let boxes = parsed.extract_text_boxes();
            assert_eq!(boxes.len(), 1);
            assert_ab_cj_geometry(&boxes[0], 10.0, &format!("{name}/subset={subset}"));
        }
    }
}

/// Character spacing, word spacing and horizontal scaling must be folded into
/// the pen advance (ISO 32000-1 §9.4.4) — these were historically dropped from
/// all geometry.
#[test]
fn spacing_and_scaling_fold_into_geometry() {
    let mut doc = PdfDocument::new("spacing");
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).unwrap();
    let font_id = doc.add_font(&font);
    let size = 10.0;
    let tc = 1.5; // pt per glyph
    let tw = 4.0; // pt per space
    let tz = 0.5; // 50% horizontal scaling
    let ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Pt(100.0).into(),
                y: Pt(100.0).into(),
            },
        },
        Op::SetFont {
            font: PdfFontHandle::External(font_id),
            size: Pt(size),
        },
        Op::SetCharacterSpacing { multiplier: tc },
        Op::SetWordSpacing { pt: Pt(tw) },
        Op::SetHorizontalScaling { percent: tz * 100.0 },
        Op::ShowText {
            items: vec![TextItem::Text("AB A".to_string())],
        },
        Op::EndTextSection,
    ];
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));

    let boxes = doc.extract_text_boxes();
    let line = &boxes[0].lines[0];
    assert_eq!(line.words.len(), 2);

    // Word 1 "AB" width: each glyph is (adv·size + Tc)·Tz wide; the glyph BOX
    // is the scaled advance (Tc is pen movement, not ink).
    let w1 = &line.words[0];
    assert_close(w1.glyphs[0].bbox[2] - w1.glyphs[0].bbox[0], adv_em('A') * size * tz, "A glyph w");

    // Word 2 "A" starts after (advA + advB + advSpace)·size·Tz + 2·Tc·Tz
    // (A and B) + (Tc + Tw)·Tz (the space).
    let expected_x =
        100.0 + ((adv_em('A') + adv_em('B') + adv_em(' ')) * size + 3.0 * tc + tw) * tz;
    assert_close(line.words[1].bbox[0], expected_x, "second word x0");
}

/// JSON output sanity: hOCR-style [x0,y0,x1,y1] bboxes, serde round trip.
#[test]
fn json_shape_roundtrips() {
    let doc = doc_with_text(MOCK_TTF, 12.0);
    let boxes = doc.extract_text_boxes();
    let json = serde_json::to_string_pretty(&boxes).unwrap();
    assert!(json.contains("\"bbox\""), "bbox key present");
    assert!(json.contains("\"AB\""), "word text present");
    let back: Vec<PageTextBoxes> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, boxes);
}
