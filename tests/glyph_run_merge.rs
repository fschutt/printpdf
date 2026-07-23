//! The serializer must coalesce absolutely-positioned glyph sequences
//! (`SetTextMatrix` + one-glyph `ShowText` per glyph — the HTML/layout
//! pipelines' output shape) into a single `Tm` + kerned `TJ` per baseline.
//!
//! Serialized literally, PDFium's text page ignores the per-show-op matrices
//! and crams every character of a line into the first glyph's bounding box —
//! Chrome/Edge then draw the selection highlight as a sliver at the line start
//! (rendering is unaffected, which is why this shipped unnoticed). One kerned
//! `TJ` per line gives correct selection geometry in every viewer.
//!
//! Uses the deterministic mock font (see `scripts/gen_mock_fonts.py`):
//! 1000 units/em, gid 2.. = 'A'.. with advance 300+50i, so every expected kern
//! is exact integer arithmetic.

use printpdf::*;

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");

/// One absolutely-positioned glyph, as the layout pipelines emit them.
fn positioned_glyph(gid: u16, ch: char, x: f32, y: f32) -> [Op; 2] {
    [
        Op::SetTextMatrix {
            matrix: TextMatrix::Raw([1.0, 0.0, 0.0, 1.0, x, y]),
        },
        Op::ShowText {
            items: vec![TextItem::GlyphIds(vec![text::Codepoint {
                gid,
                offset: 0.0,
                cid: Some(ch.to_string()),
            }])],
        },
    ]
}

/// Decoded content-stream operations of the produced PDF's first page.
fn page_ops(pdf_bytes: &[u8]) -> Vec<lopdf::content::Operation> {
    let doc = lopdf::Document::load_mem(pdf_bytes).expect("PDF parses");
    let (_, page_id) = doc.get_pages().into_iter().next().expect("one page");
    doc.get_and_decode_page_content(page_id)
        .expect("content parses")
        .operations
}

#[test]
fn per_glyph_matrices_merge_into_kerned_tj_runs() {
    let mut doc = PdfDocument::new("glyph run merge");
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let font_id = doc.add_font(&font);

    // Two baselines. Line 1: A(gid2, W=300) at x=10, B(gid3, W=350) at x=20,
    // C(gid4, W=400) at x=100 — deliberately NOT at natural pen positions.
    // Line 2: a single glyph. Font size 10pt => 1pt = 100/1000 em.
    let mut ops = vec![
        Op::SetFont {
            font: PdfFontHandle::External(font_id.clone()),
            size: Pt(10.0),
        },
        Op::StartTextSection,
    ];
    ops.extend(positioned_glyph(2, 'A', 10.0, 700.0));
    ops.extend(positioned_glyph(3, 'B', 20.0, 700.0));
    ops.extend(positioned_glyph(4, 'C', 100.0, 700.0));
    ops.extend(positioned_glyph(2, 'A', 10.0, 680.0));
    ops.push(Op::EndTextSection);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let mut warnings = Vec::new();
    let opts = PdfSaveOptions {
        optimize: false,
        subset_fonts: false, // codes == gids, so the TJ strings are predictable
        ..Default::default()
    };
    let bytes = doc.with_pages(vec![page]).save(&opts, &mut warnings);

    let content = page_ops(&bytes);
    let tms: Vec<&lopdf::content::Operation> =
        content.iter().filter(|op| op.operator == "Tm").collect();
    let tjs: Vec<&lopdf::content::Operation> =
        content.iter().filter(|op| op.operator == "TJ").collect();

    // One Tm + one TJ per BASELINE — not per glyph.
    assert_eq!(tms.len(), 2, "one Tm per baseline, got: {content:#?}");
    assert_eq!(tjs.len(), 2, "one TJ per baseline, got: {content:#?}");

    // Line 1 anchors at (10, 700)...
    let tm1: Vec<f32> = tms[0]
        .operands
        .iter()
        .map(|o| o.as_float().expect("Tm operand"))
        .collect();
    assert_eq!(tm1, vec![1.0, 0.0, 0.0, 1.0, 10.0, 700.0]);

    // ...and its TJ reproduces the absolute x positions via kerns computed
    // against the /W advances. Pen after A = 300; B sits at (20-10)pt = 1000
    // -> kern 300-1000 = -700. Pen after B = 1000+350; C sits at 9000 -> kern
    // 1350-9000 = -7650.
    let lopdf::Object::Array(arr) = &tjs[0].operands[0] else {
        panic!("TJ operand must be an array");
    };
    let rendered: Vec<String> = arr
        .iter()
        .map(|o| match o {
            lopdf::Object::String(s, _) => {
                format!("<{}>", u16::from_be_bytes([s[0], s[1]]))
            }
            lopdf::Object::Real(r) => format!("{r}"),
            lopdf::Object::Integer(i) => format!("{i}"),
            other => panic!("unexpected TJ element {other:?}"),
        })
        .collect();
    assert_eq!(rendered, vec!["<2>", "-700", "<3>", "-7650", "<4>"]);

    // Line 2: new baseline, its own anchor + single-glyph TJ.
    let tm2: Vec<f32> = tms[1]
        .operands
        .iter()
        .map(|o| o.as_float().expect("Tm operand"))
        .collect();
    assert_eq!(tm2, vec![1.0, 0.0, 0.0, 1.0, 10.0, 680.0]);
    let lopdf::Object::Array(arr2) = &tjs[1].operands[0] else {
        panic!("TJ operand must be an array");
    };
    assert_eq!(arr2.len(), 1, "single glyph, no kerns: {arr2:?}");
}

#[test]
fn nonzero_character_spacing_disables_merging() {
    // With Tc != 0 the viewer adds Tc to every advance, which the kern math
    // does not model — the serializer must fall back to literal per-glyph
    // matrices (positionally exact under any text state).
    let mut doc = PdfDocument::new("no merge under Tc");
    let font = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock font parses");
    let font_id = doc.add_font(&font);

    let mut ops = vec![
        Op::SetFont {
            font: PdfFontHandle::External(font_id.clone()),
            size: Pt(10.0),
        },
        Op::StartTextSection,
        Op::SetCharacterSpacing { multiplier: 1.5 },
    ];
    ops.extend(positioned_glyph(2, 'A', 10.0, 700.0));
    ops.extend(positioned_glyph(3, 'B', 20.0, 700.0));
    ops.push(Op::EndTextSection);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let opts = PdfSaveOptions {
        optimize: false,
        subset_fonts: false,
        ..Default::default()
    };
    let bytes = doc.with_pages(vec![page]).save(&opts, &mut Vec::new());

    let content = page_ops(&bytes);
    let tm_count = content.iter().filter(|op| op.operator == "Tm").count();
    assert_eq!(tm_count, 2, "Tc active: keep one Tm per glyph");
}
