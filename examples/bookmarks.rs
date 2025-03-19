use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Bookmarks and Annotations Example");

    // Create a vector to hold our pages
    let mut pages = Vec::new();

    // Create page 1: Introduction
    let intro_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(24.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(24.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.7,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "Bookmarks and Annotations Example".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::SetFontSizeBuiltinFont {
            size: Pt(12.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(12.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "This document demonstrates bookmarks and annotations in PDFs.".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "Use the bookmarks panel in your PDF viewer to navigate through sections."
                    .to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("This document contains:".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "1. Document-level bookmarks (in the bookmarks panel)".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "2. Link annotations (clickable areas within pages)".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "Below is a link that navigates to Section 1:".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // Create a link annotation to page 2
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(200.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::go_to(Destination::Xyz {
                    page: 2,
                    left: Some(0.0),
                    top: Some(792.0),
                    zoom: None,
                }),
                None,                                   // default border
                Some(ColorArray::Rgb([0.0, 0.0, 1.0])), // blue highlight
                None,                                   // default highlighting mode
            ),
        },
        // Draw a background for the link area
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.9,
                g: 0.9,
                b: 1.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Add link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(60.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.8,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Go to Section 1: Documentation".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // External link example
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(40.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(12.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(12.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "This is an external link to a website:".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // External link annotation
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(110.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::uri("https://github.com/fschutt/printpdf".to_string()),
                None,                                   // default border
                Some(ColorArray::Rgb([0.0, 0.6, 0.0])), // green highlight
                None,                                   // default highlighting mode
            ),
        },
        // Draw a background for the external link
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.9,
                g: 1.0,
                b: 0.9,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(110.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(110.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(80.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(80.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // External link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(30.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Visit printpdf GitHub".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
    ];

    // Create page 2: Section 1
    let section1_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(24.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(24.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Section 1: Documentation".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::SetFontSizeBuiltinFont {
            size: Pt(12.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(12.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "This page demonstrates document navigation.".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "You arrived here by clicking a link on the previous page.".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "Below are links to navigate to other sections:".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // Link to Section 2
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(200.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::go_to(Destination::Xyz {
                    page: 3,
                    left: Some(0.0),
                    top: Some(792.0),
                    zoom: None,
                }),
                None,
                Some(ColorArray::Rgb([0.7, 0.0, 0.7])),
                None,
            ),
        },
        // Background for Section 2 link
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.9,
                b: 1.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Section 2 link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(60.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.5,
                g: 0.0,
                b: 0.5,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "Go to Section 2: Advanced Usage".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // Link back to introduction
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(150.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::go_to(Destination::Xyz {
                    page: 1,
                    left: Some(0.0),
                    top: Some(792.0),
                    zoom: None,
                }),
                None,
                Some(ColorArray::Rgb([0.0, 0.0, 0.7])),
                None,
            ),
        },
        // Background for back link
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.9,
                g: 0.9,
                b: 1.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(120.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(120.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Back link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(45.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.5,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Back to Introduction".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
    ];

    // Create page 3: Section 2
    let section2_ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(24.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(24.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.7,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Section 2: Advanced Usage".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::SetFontSizeBuiltinFont {
            size: Pt(12.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(12.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "This is the final section of our document.".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "You arrived here by clicking a link on the previous page.".to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::AddLineBreak,
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text(
                "The use of bookmarks and link annotations enhances PDF navigation and usability."
                    .to_string(),
            )],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // Link back to Section 1
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(200.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::go_to(Destination::Xyz {
                    page: 2,
                    left: Some(0.0),
                    top: Some(792.0),
                    zoom: None,
                }),
                None,
                Some(ColorArray::Rgb([0.0, 0.5, 0.0])),
                None,
            ),
        },
        // Background for Section 1 link
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.9,
                g: 1.0,
                b: 0.9,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(200.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(170.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Section 1 link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(60.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.5,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Back to Section 1".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
        // Link back to Introduction
        Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Pt(100.0),
                    y: Pt(150.0),
                    width: Pt(200.0),
                    height: Pt(30.0),
                },
                Actions::go_to(Destination::Xyz {
                    page: 1,
                    left: Some(0.0),
                    top: Some(792.0),
                    zoom: None,
                }),
                None,
                Some(ColorArray::Rgb([0.0, 0.0, 0.7])),
                None,
            ),
        },
        // Background for intro link
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.9,
                g: 0.9,
                b: 1.0,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(120.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(120.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Intro link text
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(35.0), Mm(45.0)),
        },
        Op::SetFontSizeBuiltinFont {
            size: Pt(14.0),
            font: BuiltinFont::Helvetica,
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.5,
                icc_profile: None,
            }),
        },
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Back to Introduction".to_string())],
            font: BuiltinFont::Helvetica,
        },
        Op::EndTextSection,
    ];

    // Add pages to our collection
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), intro_ops));
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), section1_ops));
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), section2_ops));

    // Add bookmarks for the document outline
    // Add Unicode bookmarks
    doc.add_bookmark("Unicode: Здравствуйте", 1);
    doc.add_bookmark("Unicode: Cześć", 2);
    doc.add_bookmark("Mixed: English and Русский", 3);

    // Save the PDF to a file
    let mut warnings = Vec::new();
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings);

    std::fs::write("./bookmarks_example.pdf", bytes).unwrap();
    println!("Created bookmarks_example.pdf");
}
