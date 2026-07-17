//! Regression: text positioned with SetTextCursor (the Td operator; used by the
//! direct API and by parsed PDFs) must render at its actual position in the SVG
//! preview, not dropped to the page bottom (f = page_height).
use printpdf::*;
#[test]
fn set_text_cursor_text_is_on_page_not_at_bottom() {
    let ph = 841.89_f32; // A4
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::Builtin(BuiltinFont::Helvetica), size: Pt(24.0) },
        Op::SetTextCursor { pos: Point { x: Pt(50.0), y: Pt(760.0) } }, // near the top
        Op::ShowText { items: vec![TextItem::Text("Near the top".to_string())] },
        Op::EndTextSection,
    ];
    let mut doc = PdfDocument::new("t");
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    let svg = doc.pages[0].to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut Vec::new());
    // find the text element's transform f (vertical translation)
    let seg = svg.split("<text").nth(1).expect("a <text> element");
    let m = seg.split("matrix(").nth(1).expect("a matrix()").split(')').next().unwrap();
    let f: f32 = m.split_whitespace().nth(5).unwrap().parse().unwrap();
    // PDF y=760 near the top -> SVG y = ph - 760 ~= 82, well within the page and
    // nowhere near the bottom (ph).
    assert!(f > 0.0 && f < ph * 0.5, "text baseline f={f} should be near the top, not the bottom (ph={ph})");
}
