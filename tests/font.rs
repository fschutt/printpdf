use printpdf::{
    units::{Mm, Pt},
    Op, ParsedFont, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions, TextItem,
};

/// Creates and parses a PDF file with a custom font
#[test]
fn test_custom_font_roundtrip() {
    // Load the RobotoMedium font
    const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

    // Create a PDF with RobotoMedium and Russian text
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let mut pdf = PdfDocument::new("Test");
    let font_id = pdf.add_font(&font);

    // The same Russian text as in your example
    let russian_text = "Привет, как дела?";

    let bytes = pdf
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(210.0),
            vec![
                Op::StartTextSection,
                Op::SetFontSize {
                    font: font_id.clone(),
                    size: Pt(20.0),
                },
                Op::WriteText {
                    font: font_id,
                    items: vec![TextItem::Text(russian_text.to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    // Save the PDF for inspection if needed
    // std::fs::write("./test_font_roundtrip.pdf", &bytes).unwrap();

    // Now try to parse the PDF back
    let mut warnings = Vec::new();
    let opts = PdfParseOptions {
        fail_on_error: false,
    };
    let parsed_pdf = PdfDocument::parse(&bytes, &opts, &mut warnings).unwrap();

    // Check if the font resource was loaded
    let font_resources = &parsed_pdf.resources.fonts.map;
    assert!(!font_resources.is_empty(), "No font resources loaded");

    // Check the page operations
    assert!(!parsed_pdf.pages.is_empty(), "No pages in the parsed PDF");
    let page = &parsed_pdf.pages[0];

    // Find WriteText operations
    let text_ops = page
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::WriteText { items, .. } => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        TextItem::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();

    // Check that we found a WriteText operation with the correct text
    assert!(!text_ops.is_empty(), "No WriteText operations found");

    const RUSSIAN_SUBSETTED: &str =
        "\n\u{6}\u{4}\u{c}\t\u{2}\u{1}\u{7}\u{b}\u{7}\u{1}\u{5}\u{c}\u{8}\u{b}\u{3}";
    assert_eq!(text_ops[0], RUSSIAN_SUBSETTED, "Text not decoded correctly");
}
