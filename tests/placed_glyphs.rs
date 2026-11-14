//! `show_placed_glyphs`: glyphs drawn exactly where a text layout put them - one kerned `TJ`
//! per stretch of a font, marks raised with the text rise - and read as the layout's text
//! through an `/ActualText` span where their own `/ToUnicode` text can not say it.
//!
//! Uses the deterministic mock font (see `scripts/gen_mock_fonts.py`): 1000 units/em, gid 2.. =
//! 'A'.. with advance 300 + 50 i, so every expected kern is exact.
#![cfg(feature = "text_layout")]

use printpdf::*;

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");

/// The mock font's `/W` width of a glyph, in thousandths of an em
fn width(gid: u16) -> f32 {
    300.0 + 50.0 * f32::from(gid - 2)
}

fn glyph(gid: u16, x: f32, y: f32, text: &str) -> PlacedGlyph {
    PlacedGlyph { font: 0, gid, x, y, text: text.to_string() }
}

/// Where a PDF viewer draws each glyph the operations show: (gid, x, rise), the pen moved on
/// by each glyph's `/W` width and back by each TJ number
fn drawn(ops: &[Op]) -> Vec<(u16, f32, f32)> {
    let (mut size, mut rise, mut pen) = (0.0f32, 0.0f32, 0.0f32);
    let mut out = Vec::new();
    for op in ops {
        match op {
            Op::SetFont { size: s, .. } => size = s.0,
            Op::SetLineOffset { multiplier } => rise = *multiplier,
            Op::ShowText { items } => {
                for item in items {
                    match item {
                        TextItem::Offset(o) => pen -= o / 1000.0 * size,
                        TextItem::GlyphIds(glyphs) => {
                            for cp in glyphs {
                                out.push((cp.gid, pen, rise));
                                pen += width(cp.gid) / 1000.0 * size;
                                pen -= cp.offset / 1000.0 * size;
                            }
                        }
                        TextItem::Text(t) => panic!("text {t:?} instead of glyphs"),
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn every_glyph_lands_where_the_layout_put_it() {
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let id = FontId("F1".to_string());
    // at 10 pt: A (3 pt wide) kerned 0.5 pt into B, C 1 pt left of where B leaves the pen, a
    // mark (gid 5) 2 pt up over C's middle, D after a gap
    let glyphs = [
        glyph(2, 0.0, 0.0, "A"),
        glyph(3, 2.5, 0.0, "B"),
        glyph(4, 5.0, 0.0, "C"),
        glyph(5, 6.0, 2.0, ""),
        glyph(6, 12.25, 0.0, "D"),
    ];
    let ops = show_placed_glyphs(&[(id.clone(), &*font)], Pt(10.0), &glyphs, None);
    let at = drawn(&ops);
    assert_eq!(at.len(), glyphs.len(), "{ops:#?}");
    for (g, (gid, x, rise)) in glyphs.iter().zip(at.iter()) {
        assert_eq!(g.gid, *gid);
        assert!((g.x - x).abs() < 1e-4, "glyph {gid} drawn at {x}, placed at {}: {ops:#?}", g.x);
        assert!((g.y - rise).abs() < 1e-4, "glyph {gid} raised {rise}, placed {}", g.y);
    }
    // the font once, the rise back at 0 at the end, no span without a text
    assert_eq!(ops.iter().filter(|o| matches!(o, Op::SetFont { .. })).count(), 1);
    let rises = |ops: &[Op]| -> Vec<f32> {
        ops.iter()
            .filter_map(|o| match o {
                Op::SetLineOffset { multiplier } => Some(*multiplier),
                _ => None,
            })
            .collect()
    };
    assert_eq!(rises(&ops), [2.0, 0.0], "raised for the mark, back for D: {ops:#?}");
    assert!(!ops.iter().any(|o| matches!(o, Op::BeginMarkedContentWithProperties { .. })));
    // a mark at the end: the rise goes back to 0 after it
    let ops = show_placed_glyphs(&[(id.clone(), &*font)], Pt(10.0), &glyphs[..4], None);
    assert_eq!(rises(&ops), [2.0, 0.0], "{ops:#?}");
    assert!(matches!(ops.last(), Some(Op::SetLineOffset { .. })), "{ops:#?}");
    // glyphs at their natural positions need no kern at all
    let natural = [glyph(2, 0.0, 0.0, "A"), glyph(3, 3.0, 0.0, "B"), glyph(4, 6.5, 0.0, "C")];
    let ops = show_placed_glyphs(&[(id, &*font)], Pt(10.0), &natural, None);
    let [Op::SetFont { .. }, Op::ShowText { items }] = ops.as_slice() else { panic!("{ops:#?}") };
    let [TextItem::GlyphIds(cps)] = items.as_slice() else { panic!("{items:#?}") };
    assert!(cps.iter().all(|cp| cp.offset == 0.0), "{cps:#?}");
}

#[test]
fn glyphs_of_two_fonts_switch_fonts_and_keep_their_places() {
    let a = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let b = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let fonts = [(FontId("F1".to_string()), &*a), (FontId("F2".to_string()), &*b)];
    let mut glyphs = vec![glyph(2, 0.0, 0.0, "A"), glyph(3, 3.25, 0.0, "B")];
    glyphs.push(PlacedGlyph { font: 1, ..glyph(4, 7.0, 0.0, "C") });
    // a font past the end of the list is the first one
    glyphs.push(PlacedGlyph { font: 7, ..glyph(2, 12.0, 0.0, "A") });
    let ops = show_placed_glyphs(&fonts, Pt(10.0), &glyphs, None);
    let switched: Vec<&FontId> = ops
        .iter()
        .filter_map(|o| match o {
            Op::SetFont { font: PdfFontHandle::External(id), .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(switched, [&fonts[0].0, &fonts[1].0, &fonts[0].0]);
    let at = drawn(&ops);
    for (g, (_, x, _)) in glyphs.iter().zip(at.iter()) {
        assert!((g.x - x).abs() < 1e-4, "{} drawn at {x}: {ops:#?}", g.x);
    }
}

/// The page's operators with their operands, as a reader sees them
fn page_ops(pdf: &[u8]) -> Vec<lopdf::content::Operation> {
    let doc = lopdf::Document::load_mem(pdf).expect("PDF parses");
    let (_, page) = doc.get_pages().into_iter().next().expect("one page");
    doc.get_and_decode_page_content(page).expect("content parses").operations
}

/// The `/ActualText` of every span on the page
fn actual_texts(ops: &[lopdf::content::Operation]) -> Vec<String> {
    ops.iter()
        .filter(|op| op.operator == "BDC")
        .filter_map(|op| match op.operands.get(1) {
            Some(lopdf::Object::Dictionary(d)) => d.get(b"ActualText").ok().cloned(),
            _ => None,
        })
        .filter_map(|text| match text {
            lopdf::Object::String(bytes, _) => {
                let units: Vec<u16> = bytes[2..].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                String::from_utf16(&units).ok()
            }
            _ => None,
        })
        .collect()
}

/// A page with `glyphs` shown at (50, 700), saved with printpdf's default options (`optimize`)
fn saved(glyphs: &[PlacedGlyph], text: &str) -> Vec<u8> {
    let mut doc = PdfDocument::new("placed glyphs");
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let id = doc.add_font(&font);
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextMatrix { matrix: TextMatrix::TranslateRotate(Pt(50.0), Pt(700.0), 30.0) },
    ];
    ops.extend(show_placed_glyphs(&[(id, &*font)], Pt(10.0), glyphs, Some(text)));
    ops.push(Op::EndTextSection);
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut Vec::new())
}

#[test]
fn letters_drawn_with_one_glyph_read_as_the_layouts_text() {
    // "AB" drawn with A's glyph twice (as two Arabic letters share one body): the glyphs'
    // /ToUnicode text can not tell them apart, so the span keeps its /ActualText
    let shared = [glyph(2, 0.0, 0.0, "A"), glyph(2, 3.0, 0.0, "B")];
    let ops = page_ops(&saved(&shared, "AB"));
    assert_eq!(actual_texts(&ops), ["AB"], "{ops:#?}");
    assert_eq!(ops.iter().filter(|op| op.operator == "EMC").count(), 1);

    // glyphs that spell their text as set: `optimize` drops the span again
    let plain = [glyph(2, 0.0, 0.0, "A"), glyph(3, 3.0, 0.0, "B"), glyph(4, 6.5, 0.0, "C")];
    let ops = page_ops(&saved(&plain, "ABC"));
    assert!(actual_texts(&ops).is_empty(), "{ops:#?}");
    assert!(ops.iter().any(|op| op.operator == "TJ"), "{ops:#?}");
}

#[test]
fn the_span_is_a_utf16_actual_text() {
    let Op::BeginMarkedContentWithProperties { tag, properties } = actual_text_span("قابس") else {
        panic!("a BDC")
    };
    assert_eq!(tag, "Span");
    let DictItem::Dict { map } = properties else { panic!("a dictionary") };
    let Some(DictItem::String { data, literal: false }) = map.get("ActualText") else {
        panic!("{map:?}")
    };
    let mut expected = vec![0xFE, 0xFF];
    for u in "قابس".encode_utf16() {
        expected.extend_from_slice(&u.to_be_bytes());
    }
    assert_eq!(data, &expected);
}

#[test]
fn a_glyph_without_text_keeps_the_text_it_was_shown_with() {
    // glyph 2 shown as "A", then as part of a cluster whose text another glyph carries (""),
    // glyph 3 first without a text, then as "B": each reads as its letter both times
    let glyphs = [
        glyph(2, 0.0, 0.0, "A"),
        glyph(2, 3.0, 0.0, ""),
        glyph(3, 6.0, 0.0, ""),
        glyph(3, 9.5, 0.0, "B"),
    ];
    let mut doc = PdfDocument::new("placed glyphs");
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let id = doc.add_font(&font);
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextMatrix { matrix: TextMatrix::Translate(Pt(50.0), Pt(700.0)) },
    ];
    ops.extend(show_placed_glyphs(&[(id, &*font)], Pt(10.0), &glyphs, None));
    ops.push(Op::EndTextSection);
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let opts = PdfSaveOptions { optimize: false, ..Default::default() };
    let pdf = doc.with_pages(vec![page]).save(&opts, &mut Vec::new());
    let parsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut Vec::new()).expect("parses");
    let text: String = parsed.extract_text().concat().concat().chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(text, "AABB");
}
