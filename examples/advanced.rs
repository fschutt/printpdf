use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Advanced Text Positioning Example");

    // Create operations for advanced text positioning and styling
    let mut ops = Vec::new();

    // Title for the page
    ops.extend_from_slice(&[
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
            size: Pt(24.0),
        },
        Op::SetLineHeight { lh: Pt(28.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::ShowText {
            items: vec![TextItem::Text(
                "Advanced Text Positioning and Styling".to_string(),
            )],
        },
        Op::EndTextSection,
    ]);

    // 1. Text with character spacing
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(260.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "1. Normal text without spacing:".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text("CHARACTERSPACING".to_string())],
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "Text with added character spacing:".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Set character spacing to 2.0 points
        Op::SetCharacterSpacing { multiplier: 2.0 },
        Op::ShowText {
            items: vec![TextItem::Text("CHARACTERSPACING".to_string())],
        },
        // Reset character spacing
        Op::SetCharacterSpacing { multiplier: 0.0 },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 2. Rotated text using TextMatrix
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(230.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::ShowText {
            items: vec![TextItem::Text("2. Rotated text:".to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // Draw rotated text at different angles
    ops.extend(
        (0..=7)
            .flat_map(|i| {
                let angle = i as f32 * 45.0; // Rotate by 0°, 45°, 90°, 135°, 180°, 225°, 270°, 315°
                vec![
                    Op::SaveGraphicsState,
                    Op::StartTextSection,
                    Op::SetTextCursor {
                        pos: Point::new(Mm(50.0), Mm(210.0)),
                    },
                    Op::SetTextMatrix {
                        matrix: TextMatrix::TranslateRotate(
                            Pt(50.0 + i as f32 * 50.0),
                            Pt(600.0),
                            angle,
                        ),
                    },
                    Op::SetFont {
                        font: PdfFontHandle::Builtin(BuiltinFont::TimesRoman),
                        size: Pt(10.0),
                    },
                    Op::SetLineHeight { lh: Pt(12.0) },
                    Op::SetFillColor {
                        col: Color::Rgb(Rgb {
                            r: 0.0,
                            g: 0.0,
                            b: i as f32 / 7.0,
                            icc_profile: None,
                        }),
                    },
                    Op::ShowText {
                        items: vec![TextItem::Text(format!("{}deg", angle))],
                    },
                    Op::EndTextSection,
                    Op::RestoreGraphicsState,
                ]
            })
            .collect::<Vec<_>>()
            .iter()
            .cloned(),
    );

    // 3. Text on a curved path (simulated with multiple rotations)
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(190.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::ShowText {
            items: vec![TextItem::Text("3. Text on a curved path:".to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // Create a curve and place text along it (arc)
    let center_x = 300.0;
    let center_y = 460.0;
    let radius = 100.0;
    let text = "Curved Text Around A Circle Path";

    for (i, c) in text.chars().rev().enumerate() {
        let angle = 180.0 - (i as f32 * 8.0);
        let radians = angle.to_radians();
        let x = center_x + radius * radians.cos();
        let y = center_y + radius * radians.sin();

        ops.extend_from_slice(&[
            Op::SaveGraphicsState,
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point {
                    x: Pt(0.0),
                    y: Pt(0.0),
                },
            },
            Op::SetTextMatrix {
                matrix: TextMatrix::TranslateRotate(Pt(x), Pt(y), angle + 90.0),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(12.0),
            },
            Op::SetLineHeight { lh: Pt(14.0) },
            Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: i as f32 / text.len() as f32,
                    g: 0.3,
                    b: 1.0 - i as f32 / text.len() as f32,
                    icc_profile: None,
                }),
            },
            Op::ShowText {
                items: vec![TextItem::Text(c.to_string())],
            },
            Op::EndTextSection,
            Op::RestoreGraphicsState,
        ]);
    }

    // 4. Kerned text with manual spacing adjustments
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(150.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "4. Kerned text with manual adjustments:".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Normal text without kerning
        Op::ShowText {
            items: vec![TextItem::Text("AV AWAY WAVE To Va".to_string())],
        },
        Op::AddLineBreak,
        // The same text with manual kerning adjustments using TJ operator
        Op::ShowText {
            items: vec![
                TextItem::Text("A".to_string()),
                TextItem::Offset(-30.0), // Move closer to V
                TextItem::Text("V".to_string()),
                TextItem::Offset(10.0), // Normal spacing
                TextItem::Text("A".to_string()),
                TextItem::Offset(-30.0), // Move closer to W
                TextItem::Text("W".to_string()),
                TextItem::Offset(-30.0), // Move closer to A
                TextItem::Text("A".to_string()),
                TextItem::Offset(-20.0), // Move closer to Y
                TextItem::Text("Y".to_string()),
                TextItem::Offset(20.0), // Extra spacing
                TextItem::Text("W".to_string()),
                TextItem::Offset(-30.0), // Move closer to A
                TextItem::Text("A".to_string()),
                TextItem::Offset(-30.0), // Move closer to V
                TextItem::Text("V".to_string()),
                TextItem::Offset(-30.0), // Move closer to E
                TextItem::Text("E".to_string()),
                TextItem::Offset(40.0), // Extra spacing
                TextItem::Text("T".to_string()),
                TextItem::Offset(-20.0), // Move closer to o
                TextItem::Text("o".to_string()),
                TextItem::Offset(20.0), // Extra spacing
                TextItem::Text("V".to_string()),
                TextItem::Offset(-40.0), // Move closer to a
                TextItem::Text("a".to_string()),
            ],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 5. Text with different rendering modes
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(130.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::ShowText {
            items: vec![TextItem::Text(
                "5. Text with different rendering modes:".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Fill mode (default)
        Op::SetTextRenderingMode {
            mode: TextRenderingMode::Fill,
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.8,
                icc_profile: None,
            }),
        },
        Op::ShowText {
            items: vec![TextItem::Text("Fill mode".to_string())],
        },
        Op::AddLineBreak,
        // Stroke mode
        Op::SetTextRenderingMode {
            mode: TextRenderingMode::Stroke,
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineThickness { pt: Pt(0.5) },
        Op::ShowText {
            items: vec![TextItem::Text("Stroke mode".to_string())],
        },
        Op::AddLineBreak,
        // Fill and stroke mode
        Op::SetTextRenderingMode {
            mode: TextRenderingMode::FillStroke,
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.8,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.3,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::ShowText {
            items: vec![TextItem::Text("Fill and stroke mode".to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 6. Text with horizontal scaling
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(100.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetTextRenderingMode {
            mode: TextRenderingMode::Fill,
        },
        Op::ShowText {
            items: vec![TextItem::Text(
                "6. Text with horizontal scaling:".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Normal text (100% scaling)
        Op::SetHorizontalScaling { percent: 100.0 },
        Op::ShowText {
            items: vec![TextItem::Text("Normal text (100% scaling)".to_string())],
        },
        Op::AddLineBreak,
        // Condensed text (75% scaling)
        Op::SetHorizontalScaling { percent: 75.0 },
        Op::ShowText {
            items: vec![TextItem::Text("Condensed text (75% scaling)".to_string())],
        },
        Op::AddLineBreak,
        // Expanded text (150% scaling)
        Op::SetHorizontalScaling { percent: 150.0 },
        Op::ShowText {
            items: vec![TextItem::Text("Expanded text (150% scaling)".to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 7. Text with word spacing
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(80.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetHorizontalScaling { percent: 100.0 }, // Reset scaling
        Op::ShowText {
            items: vec![TextItem::Text("7. Text with word spacing:".to_string())],
        },
        Op::AddLineBreak,
        // Normal word spacing
        Op::SetWordSpacing { pt: Pt(0.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "This is text with normal word spacing.".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Extra word spacing
        Op::SetWordSpacing { pt: Pt(10.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "This is text with extra word spacing.".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Even more word spacing
        Op::SetWordSpacing { pt: Pt(20.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "This is text with even more word spacing.".to_string(),
            )],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 8. Text with different fonts in the same line
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(60.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetWordSpacing { pt: Pt(0.0) }, // Reset word spacing
        Op::ShowText {
            items: vec![TextItem::Text(
                "8. Mixed fonts in a single line:".to_string(),
            )],
        },
        Op::AddLineBreak,
        // Create a mixed-font text line by changing fonts between text segments
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text("This is in ".to_string())],
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::TimesRoman),
            size: Pt(12.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text("Times Roman".to_string())],
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Courier),
            size: Pt(12.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(" and this is in Courier".to_string())],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // 9. Example of positioned glyphs with azul text3 integration  
    // This demonstrates the complete workflow: azul UnifiedLayout → Vec<Op>
    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(40.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "9. Azul Text3 Integration for Complex Scripts:".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "✓ TextMatrix provides absolute positioning".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "✓ TextItem::GlyphIds preserves exact glyph positioning".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "✓ Perfect for Arabic/Indic script shaping via azul-layout".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "Architecture: ParsedFont → UnifiedLayout → Vec<Op>".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "See printpdf::html::from_html() for full implementation".to_string(),
            )],
        },
        Op::EndTextSection,
        Op::RestoreGraphicsState,
    ]);

    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    std::fs::write("./text_positioning_example.pdf", bytes).unwrap();
    println!("Created text_positioning_example.pdf");
}
