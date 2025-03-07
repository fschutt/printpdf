use printpdf::*;

static ROBOTO_TTF: &[u8] = include_bytes!("./assets/fonts/RobotoMedium.ttf");

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Text Example");

    // Load and register an external font
    let roboto = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let roboto_id = doc.add_font(&roboto);

    // Create operations for different text styles
    let ops = vec![
        // Save the graphics state to allow for position resets later
        Op::SaveGraphicsState,
        // Start a text section (required for text operations)
        Op::StartTextSection,
        // Position the text cursor from the bottom left
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(270.0)),
        },
        // Set a built-in font (Helvetica) with its size
        Op::SetFontSizeBuiltinFont {
            size: Pt(24.0),
            font: BuiltinFont::Helvetica,
        },
        // Set text color to blue
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.8,
                icc_profile: None,
            }),
        },
        // Write text with the built-in font
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Hello from Helvetica!".to_string())],
            font: BuiltinFont::Helvetica,
        },
        // Add a line break to move down
        Op::AddLineBreak,
        // Change to Times Roman font
        Op::SetFontSizeBuiltinFont {
            size: Pt(18.0),
            font: BuiltinFont::TimesRoman,
        },
        // Change color to dark red
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        // Write text with Times Roman
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("This is Times Roman font".to_string())],
            font: BuiltinFont::TimesRoman,
        },
        // Add another line break
        Op::AddLineBreak,
        // Use our custom Roboto font
        Op::SetFontSize {
            size: Pt(14.0),
            font: roboto_id.clone(),
        },
        // Change color to dark green
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.6,
                b: 0.0,
                icc_profile: None,
            }),
        },
        // Write text with the custom font
        Op::WriteText {
            items: vec![TextItem::Text("This text uses the Roboto font".to_string())],
            font: roboto_id.clone(),
        },
        // End the text section
        Op::EndTextSection,
        // Restore the graphics state
        Op::RestoreGraphicsState,
    ];

    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    std::fs::write("./text_example.pdf", bytes).unwrap();
    println!("Created text_example.pdf");
}
