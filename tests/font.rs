use printpdf::{
    units::{Mm, Pt},
    Op, ParsedFont, PdfDocument, PdfPage, PdfParseErrorSeverity, PdfParseOptions, PdfSaveOptions,
    TextItem,
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

/// Test for issue #244: Parsing incomplete subsetted font should not panic
/// 
/// This font (r13-font.bin) is a malformed/incomplete TrueType font extracted from a PDF.
/// It's missing critical tables like HEAD, MAXP, CMAP, and GLYF which causes
/// allsorts Font::new() to fail with MissingValue error.
/// 
/// The test verifies that:
/// 1. The font parsing doesn't panic (previously had unwrap() at line 1066)
/// 2. ParsedFont::from_bytes returns None for invalid fonts
/// 3. Appropriate warnings are generated
#[test]
fn test_r13_incomplete_subsetted_font() {
    // This is the problematic font from issue #244
    // It's a subsetted Webdings font missing critical tables
    const R13_FONT: &[u8] = include_bytes!("r13-font.bin");

    let mut warnings = Vec::new();
    let result = ParsedFont::from_bytes(R13_FONT, 0, &mut warnings);

    // The font should fail to parse due to missing tables
    assert!(
        result.is_none(),
        "Expected None for incomplete font, but got Some"
    );

    // Check that we got warnings about the failure
    let has_failure_warning = warnings.iter().any(|w| {
        w.msg.contains("Failed to") || w.msg.contains("Missing") || w.msg.contains("failed")
    });

    assert!(
        has_failure_warning,
        "Expected warnings about parsing failure, got: {:?}",
        warnings
    );

    // Ensure we don't have a panic - if we reach here, the test passes
    println!("Successfully handled incomplete font without panicking");
    println!("Warnings generated: {}", warnings.len());
}

/// Test for standalone CFF parsing
/// 
/// This test verifies that the from_cff_bytes function can handle:
/// 1. Invalid CFF data (should return None with warnings)
/// 2. Empty data (should return None with warnings)
/// 
/// Note: We don't have a valid standalone CFF file to test with yet,
/// so this test focuses on error handling.
#[test]
fn test_standalone_cff_parsing() {
    // Test with empty data
    let mut warnings = Vec::new();
    let result = ParsedFont::from_cff_bytes(&[], &mut warnings);
    assert!(result.is_none(), "Expected None for empty CFF data");
    assert!(!warnings.is_empty(), "Expected warnings for empty data");
    
    // Verify we have actual warning/error messages (not just info)
    let has_warnings = warnings.iter().any(|w| matches!(w.severity, PdfParseErrorSeverity::Warning | PdfParseErrorSeverity::Error));
    assert!(has_warnings, "Expected warning-level messages for empty data");

    // Test with invalid CFF data
    let mut warnings = Vec::new();
    let invalid_data = b"This is not a valid CFF font";
    let result = ParsedFont::from_cff_bytes(invalid_data, &mut warnings);
    assert!(result.is_none(), "Expected None for invalid CFF data");
    
    let has_parse_error = warnings.iter().any(|w| {
        w.msg.contains("Failed to parse") || w.msg.contains("parse standalone CFF")
    });
    assert!(has_parse_error, "Expected parsing error warning for invalid CFF data");
    
    // Verify we have actual warning/error messages (not just info)
    let has_warnings = warnings.iter().any(|w| matches!(w.severity, PdfParseErrorSeverity::Warning | PdfParseErrorSeverity::Error));
    assert!(has_warnings, "Expected warning-level messages for invalid CFF data");

    println!("CFF parsing correctly handles invalid data");
}
