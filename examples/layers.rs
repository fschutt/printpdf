use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Layers Example");

    // Create layers with different purposes
    let background_layer = Layer {
        name: "Background".to_string(),
        creator: "printpdf".to_string(),
        intent: LayerIntent::View,
        usage: LayerSubtype::Artwork,
    };

    let text_layer = Layer {
        name: "Text Content".to_string(),
        creator: "printpdf".to_string(),
        intent: LayerIntent::Design,
        usage: LayerSubtype::Artwork,
    };

    let graphics_layer = Layer {
        name: "Graphics".to_string(),
        creator: "printpdf".to_string(),
        intent: LayerIntent::Design,
        usage: LayerSubtype::Artwork,
    };

    // Add the layers to the document and get their IDs
    let bg_layer_id = doc.add_layer(&background_layer);
    let text_layer_id = doc.add_layer(&text_layer);
    let graphics_layer_id = doc.add_layer(&graphics_layer);

    // Create operations for our page, organizing content by layers
    let mut ops = Vec::new();

    // Background layer content - a colored rectangle covering the page
    ops.extend_from_slice(&[
        Op::BeginLayer {
            layer_id: bg_layer_id.clone(),
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(0.0),
                                y: Pt(0.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(595.0),
                                y: Pt(0.0),
                            }, // A4 width in points
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(595.0),
                                y: Pt(842.0),
                            }, // A4 height in points
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(0.0),
                                y: Pt(842.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        },
        Op::EndLayer,
    ]);

    // Text layer content
    ops.extend_from_slice(&[
        Op::BeginLayer {
            layer_id: text_layer_id.clone(),
        },
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(270.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(24.0),
        },
        Op::SetLineHeight { lh: Pt(24.0) },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.6,
                icc_profile: None,
            }),
        },
        Op::ShowText {
            items: vec![TextItem::Text("PDF Layers Example".to_string())],
        },
        Op::AddLineBreak,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
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
        Op::ShowText {
            items: vec![TextItem::Text(
                "This PDF demonstrates the use of layers (also called Optional Content Groups)."
                    .to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "The content is organized in three separate layers:".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "1. Background - light gray background".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "2. Text Content - the text you're reading now".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "3. Graphics - shapes and visual elements".to_string(),
            )],
        },
        Op::AddLineBreak,
        Op::AddLineBreak,
        Op::ShowText {
            items: vec![TextItem::Text(
                "In a PDF viewer that supports layers, you can toggle these on and off."
                    .to_string(),
            )],
        },
        Op::EndTextSection,
        Op::EndLayer,
    ]);

    // Graphics layer content
    ops.extend_from_slice(&[
        Op::BeginLayer {
            layer_id: graphics_layer_id.clone(),
        },
        // Draw a circle (approximated with bezier curves)
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.8,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.4,
                b: 0.0,
                icc_profile: None,
            }),
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        // Draw a rectangle
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(100.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(200.0),
                                y: Pt(100.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(200.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(100.0),
                                y: Pt(150.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Draw a triangle
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.7,
                b: 0.7,
                icc_profile: None,
            }),
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point {
                                x: Pt(300.0),
                                y: Pt(100.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(400.0),
                                y: Pt(100.0),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(350.0),
                                y: Pt(180.0),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::NonZero,
            },
        },
        // Draw some lines
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.7,
                g: 0.0,
                b: 0.7,
                icc_profile: None,
            }),
        },
        Op::SetOutlineThickness { pt: Pt(3.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(450.0),
                            y: Pt(100.0),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(550.0),
                            y: Pt(180.0),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        },
        // Set line dash pattern and draw another line
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: Some(10),
                gap_1: Some(5),
                dash_2: None,
                gap_2: None,
                dash_3: None,
                gap_3: None,
            },
        },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(450.0),
                            y: Pt(130.0),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(550.0),
                            y: Pt(210.0),
                        },
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        },
        Op::EndLayer,
    ]);

    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    std::fs::write("./layers_example.pdf", bytes).unwrap();
    println!("Created layers_example.pdf");
}
