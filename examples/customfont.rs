use printpdf::{Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, Pt, TextItem};

const ROBOTO_TTF: &[u8] = include_bytes!("./assets/fonts/RobotoMedium.ttf");

fn main() {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let mut pdf = PdfDocument::new("Test");
    let font_id = pdf.add_font(&font);
    let bytes = pdf
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(210.0),
            vec![
                Op::StartTextSection,
                Op::SetFontSize {
                    // <- todo: mandatory set font size ???
                    font: font_id.clone(),
                    size: Pt(20.0),
                },
                Op::WriteText {
                    font: font_id,
                    items: vec![TextItem::Text("Привет, как дела?".to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    let _ = std::fs::write("./mini.pdf", bytes);
}
