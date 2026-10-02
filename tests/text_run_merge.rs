//! With `PdfSaveOptions::optimize`, consecutive text objects become one, repeated font/colour
//! settings are dropped, and `/ActualText` spans on one baseline are joined (`src/text_merge.rs`).
//!
//! The HTML renderer emits one text object per glyph run, and a run ends at every font change:
//! a line whose spaces come from a fallback font becomes a text object per word and per space
//! (#288). The glyphs, their matrices, fonts and colours must come out unchanged.

use lopdf::content::Operation;
use lopdf::Object;
use printpdf::*;

const MOCK_TTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_ttf.ttf");
const MOCK_OTF: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_named.otf");

fn fonts(doc: &mut PdfDocument) -> (FontId, FontId) {
    let a = ParsedFont::from_bytes(MOCK_TTF, 0, &mut Vec::new()).expect("mock ttf parses");
    let b = ParsedFont::from_bytes(MOCK_OTF, 0, &mut Vec::new()).expect("mock otf parses");
    (doc.add_font(&a), doc.add_font(&b))
}

fn actual_text(text: &str) -> Op {
    let mut data = vec![0xFE, 0xFF];
    data.extend(text.encode_utf16().flat_map(|u| u.to_be_bytes()));
    let map = [("ActualText".to_string(), DictItem::String { data, literal: false })].into_iter().collect();
    Op::BeginMarkedContentWithProperties { tag: "Span".into(), properties: DictItem::Dict { map } }
}

/// One glyph run as the HTML renderer emits it: Tf, BT, /ActualText span, Tm + glyph, EMC, ET.
fn run(font: &FontId, text: &str, gid: u16, x: f32, y: f32) -> Vec<Op> {
    vec![
        Op::SetFont { font: PdfFontHandle::External(font.clone()), size: Pt(10.0) },
        Op::StartTextSection,
        actual_text(text),
        Op::SetTextMatrix { matrix: TextMatrix::Raw([1.0, 0.0, 0.0, 1.0, x, y]) },
        Op::ShowText {
            items: vec![TextItem::GlyphIds(vec![text::Codepoint { gid, offset: 0.0, cid: Some(text.into()) }])],
        },
        Op::EndMarkedContent,
        Op::EndTextSection,
    ]
}

fn save(mut doc: PdfDocument, ops: Vec<Op>, optimize: bool) -> Vec<Operation> {
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let opts = PdfSaveOptions { optimize, subset_fonts: false, ..Default::default() };
    let bytes = doc.with_pages(vec![page]).save(&opts, &mut Vec::new());
    let pdf = lopdf::Document::load_mem(&bytes).expect("PDF parses");
    let (_, page_id) = pdf.get_pages().into_iter().next().expect("one page");
    pdf.get_and_decode_page_content(page_id).expect("content parses").operations
}

fn count(ops: &[Operation], operator: &str) -> usize {
    ops.iter().filter(|op| op.operator == operator).count()
}

/// The `/ActualText` of every `BDC`, decoded.
fn spans(ops: &[Operation]) -> Vec<String> {
    ops.iter()
        .filter(|op| op.operator == "BDC")
        .map(|op| match &op.operands[1] {
            Object::Dictionary(d) => match d.get(b"ActualText").expect("ActualText") {
                Object::String(bytes, _) => {
                    let units: Vec<u16> = bytes[2..].chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                    String::from_utf16(&units).unwrap()
                }
                other => panic!("ActualText is {other:?}"),
            },
            other => panic!("BDC properties are {other:?}"),
        })
        .collect()
}

/// "Hello" in font A, " " in font B, "world" in font A, all on one baseline, then a second line.
fn two_lines(a: &FontId, b: &FontId) -> Vec<Op> {
    let mut ops = vec![Op::SetFillColor { col: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }) }];
    ops.extend(run(a, "Hello", 2, 10.0, 700.0));
    ops.extend(run(b, " ", 1, 40.0, 700.0));
    ops.extend(run(a, "world", 3, 45.0, 700.0));
    ops.extend(run(a, "Next", 4, 10.0, 680.0));
    ops
}

#[test]
fn runs_become_one_text_object_with_one_span_per_line() {
    let mut doc = PdfDocument::new("merge");
    let (a, b) = fonts(&mut doc);
    let ops = save(doc, two_lines(&a, &b), true);
    assert_eq!((count(&ops, "BT"), count(&ops, "ET")), (1, 1));
    assert_eq!(spans(&ops), ["Hello world", "Next"]);
    // font switches stay (A, B, A); "Next" repeats font A and is dropped
    assert_eq!(count(&ops, "Tf"), 3);
    // every glyph keeps its own position
    assert_eq!(count(&ops, "Tm"), 4);
    assert_eq!(count(&ops, "rg"), 1);
}

#[test]
fn without_optimize_the_ops_are_written_as_given() {
    let mut doc = PdfDocument::new("merge");
    let (a, b) = fonts(&mut doc);
    let ops = save(doc, two_lines(&a, &b), false);
    assert_eq!(count(&ops, "BT"), 4);
    assert_eq!(spans(&ops), ["Hello", " ", "world", "Next"]);
}

#[test]
fn a_text_object_that_relies_on_bt_is_left_alone() {
    let mut doc = PdfDocument::new("merge");
    let (a, _) = fonts(&mut doc);
    let mut ops = run(&a, "One", 2, 10.0, 700.0);
    // shows text at the position BT set (the origin): merged, it would continue after "One"
    ops.extend([
        Op::StartTextSection,
        Op::ShowText {
            items: vec![TextItem::GlyphIds(vec![text::Codepoint { gid: 3, offset: 0.0, cid: Some("B".into()) }])],
        },
        Op::EndTextSection,
    ]);
    let ops = save(doc, ops, true);
    assert_eq!(count(&ops, "BT"), 2);
}

#[test]
fn drawing_between_text_objects_keeps_them_apart() {
    let mut doc = PdfDocument::new("merge");
    let (a, _) = fonts(&mut doc);
    let mut ops = run(&a, "One", 2, 10.0, 700.0);
    ops.push(Op::DrawLine {
        line: Line { points: vec![LinePoint { p: Point::new(Mm(1.0), Mm(1.0)), bezier: false }, LinePoint { p: Point::new(Mm(5.0), Mm(1.0)), bezier: false }], is_closed: false },
    });
    ops.extend(run(&a, "Two", 3, 40.0, 700.0));
    let ops = save(doc, ops, true);
    assert_eq!(count(&ops, "BT"), 2);
    assert_eq!(spans(&ops), ["One", "Two"]);
}

#[test]
fn font_is_set_again_after_restoring_graphics_state() {
    let mut doc = PdfDocument::new("merge");
    let (a, _) = fonts(&mut doc);
    let mut ops = vec![Op::SaveGraphicsState];
    ops.extend(run(&a, "One", 2, 10.0, 700.0));
    ops.push(Op::RestoreGraphicsState);
    ops.extend(run(&a, "Two", 3, 10.0, 680.0));
    let ops = save(doc, ops, true);
    assert_eq!(count(&ops, "Tf"), 2);
}

#[cfg(feature = "html")]
#[test]
fn html_line_in_a_builtin_family_is_one_glyph_run() {
    // the bundled subsets had no space glyph, so every space came from a system font (#288)
    let html = r#"<html><body><p style="font-family: Helvetica; font-size: 12pt">Hello world and more words here</p></body></html>"#;
    let doc = PdfDocument::from_html(html, &Default::default(), &Default::default(), &Default::default(), &mut Vec::new())
        .expect("HTML renders");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let pdf = lopdf::Document::load_mem(&bytes).expect("PDF parses");
    let (_, page_id) = pdf.get_pages().into_iter().next().expect("one page");
    let ops = pdf.get_and_decode_page_content(page_id).expect("content parses").operations;
    assert_eq!((count(&ops, "BT"), count(&ops, "Tf"), count(&ops, "Tm"), count(&ops, "TJ")), (1, 1, 1, 1));
    assert_eq!(spans(&ops), ["Hello world and more words here"]);
    let fonts: Vec<_> = pdf.objects.values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"Type").ok().and_then(|t| t.as_name().ok()) == Some(b"Font".as_slice()))
        .filter_map(|d| d.get(b"BaseFont").ok().and_then(|n| n.as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned()))
        .collect();
    assert_eq!(fonts.len(), 1, "only Helvetica, no fallback font: {fonts:?}");
}

#[cfg(feature = "html")]
#[test]
fn optimizing_html_keeps_the_extracted_text() {
    let html = r#"<html><body><p style="font-family: Helvetica; font-size: 12pt">Hello world and more words here</p></body></html>"#;
    let doc_for = || {
        PdfDocument::from_html(html, &Default::default(), &Default::default(), &Default::default(), &mut Vec::new())
            .expect("HTML renders")
    };
    let content = |doc: PdfDocument, optimize: bool| {
        let opts = PdfSaveOptions { optimize, ..Default::default() };
        let bytes = doc.save(&opts, &mut Vec::new());
        let pdf = lopdf::Document::load_mem(&bytes).expect("PDF parses");
        let (_, page_id) = pdf.get_pages().into_iter().next().expect("one page");
        pdf.get_and_decode_page_content(page_id).expect("content parses").operations
    };
    let plain = content(doc_for(), false);
    let merged = content(doc_for(), true);
    assert_eq!(count(&merged, "BT"), 1);
    assert_eq!(spans(&merged).concat(), spans(&plain).concat());
    assert_eq!(spans(&merged).concat(), "Hello world and more words here");
    assert_eq!(count(&merged, "BDC"), 1, "one /ActualText span for the line");
}
