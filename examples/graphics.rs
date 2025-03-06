use printpdf::*;

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Shapes and Graphics Example");
    
    // Create operations for drawing various shapes
    let mut ops = Vec::new();
    
    // Title for the page
    ops.extend_from_slice(&[
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(24.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("PDF Shapes and Graphics".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
    ]);
    
    // 1. Rectangle - filled with color
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(260.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("1. Filled Rectangle".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Draw the rectangle
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.2,
                b: 0.2,
                icc_profile: None,
            })
        },
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(740.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(740.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(700.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(700.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            }
        },
    ]);
    
    // 2. Rectangle - outlined with thick border
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(230.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("2. Outlined Rectangle".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Set outline properties
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.2,
                g: 0.2,
                b: 0.8,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(3.0) },
        
        // Draw the rectangle (stroke only)
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(680.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(680.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(640.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(640.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Stroke,
                winding_order: WindingOrder::NonZero,
            }
        },
    ]);
    
    // 3. Rectangle - filled and outlined
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(200.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("3. Filled and Outlined Rectangle".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Set fill and outline properties
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.8,
                b: 0.2,
                icc_profile: None,
            })
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.4,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        
        // Draw the rectangle (fill and stroke)
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(620.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(620.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(580.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(580.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::NonZero,
            }
        },
    ]);
    
    // 4. Triangle
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(170.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("4. Triangle".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Set fill and outline properties
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.2,
                g: 0.8,
                b: 0.2,
                icc_profile: None,
            })
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.4,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        
        // Draw the triangle
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(560.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(560.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(150.0), y: Pt(480.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::NonZero,
            }
        },
    ]);
    
    // 5. Star-like shape
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(140.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("5. Complex Shape".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Set fill and outline properties
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                icc_profile: None,
            })
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.3,
                g: 0.2,
                b: 0.5,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        
        // Draw a star-like shape
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(150.0), y: Pt(450.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(170.0), y: Pt(410.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(420.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(180.0), y: Pt(380.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(200.0), y: Pt(350.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(150.0), y: Pt(360.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(350.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(120.0), y: Pt(380.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(100.0), y: Pt(420.0) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: Pt(130.0), y: Pt(410.0) },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::NonZero,
            }
        },
    ]);
    
    // 6. Line with different dash patterns
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(110.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("6. Lines with Different Dash Patterns".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Draw a solid line
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(2.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(100.0), y: Pt(300.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(300.0), y: Pt(300.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
        
        // Draw a dashed line
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: Some(10),
                gap_1: Some(5),
                dash_2: None,
                gap_2: None,
                dash_3: None,
                gap_3: None,
            }
        },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(100.0), y: Pt(280.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(300.0), y: Pt(280.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
        
        // Draw a dotted line
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: Some(2),
                gap_1: Some(5),
                dash_2: None,
                gap_2: None,
                dash_3: None,
                gap_3: None,
            }
        },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(100.0), y: Pt(260.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(300.0), y: Pt(260.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
        
        // Draw a dash-dot-dash line
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: Some(10),
                gap_1: Some(5),
                dash_2: Some(2),
                gap_2: Some(5),
                dash_3: None,
                gap_3: None,
            }
        },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(100.0), y: Pt(240.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(300.0), y: Pt(240.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
    ]);
    
    // 7. Polygon with even-odd winding rule
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(80.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("7. Polygon with Even-Odd Winding Rule".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Set fill and outline properties
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.8,
                b: 0.2,
                icc_profile: None,
            })
        },
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.6,
                g: 0.6,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(1.0) },
        
        // Reset line dash pattern
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: None,
                gap_1: None,
                dash_2: None,
                gap_2: None,
                dash_3: None,
                gap_3: None,
            }
        },
        
        // Draw a star with even-odd winding
        Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![
                    // Outer pentagon
                    PolygonRing {
                        points: vec![
                            LinePoint {
                                p: Point { x: Pt(150.0), y: Pt(210.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(190.0), y: Pt(180.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(180.0), y: Pt(130.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(120.0), y: Pt(130.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(110.0), y: Pt(180.0) },
                                bezier: false,
                            },
                        ],
                    },
                    // Inner pentagon (pointing in opposite direction)
                    PolygonRing {
                        points: vec![
                            LinePoint {
                                p: Point { x: Pt(150.0), y: Pt(150.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(130.0), y: Pt(170.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(140.0), y: Pt(190.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(160.0), y: Pt(190.0) },
                                bezier: false,
                            },
                            LinePoint {
                                p: Point { x: Pt(170.0), y: Pt(170.0) },
                                bezier: false,
                            },
                        ],
                    },
                ],
                mode: PaintMode::FillStroke,
                winding_order: WindingOrder::EvenOdd,
            }
        },
    ]);
    
    // 8. Lines with different caps and joins
    ops.extend_from_slice(&[
        // Add label
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(50.0)),
        },
        Op::SetFontSizeBuiltinFont { 
            size: Pt(12.0), 
            font: BuiltinFont::Helvetica 
        },
        Op::WriteTextBuiltinFont { 
            items: vec![TextItem::Text("8. Lines with Different Caps and Joins".to_string())], 
            font: BuiltinFont::Helvetica 
        },
        Op::EndTextSection,
        
        // Draw lines with butt caps and miter joins
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.8,
                icc_profile: None,
            })
        },
        Op::SetOutlineThickness { pt: Pt(10.0) },
        Op::SetLineCapStyle { cap: LineCapStyle::Butt },
        Op::SetLineJoinStyle { join: LineJoinStyle::Miter },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(350.0), y: Pt(680.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(400.0), y: Pt(650.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(450.0), y: Pt(680.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
        
        // Draw lines with round caps and round joins
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.8,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetLineCapStyle { cap: LineCapStyle::Round },
        Op::SetLineJoinStyle { join: LineJoinStyle::Round },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(350.0), y: Pt(630.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(400.0), y: Pt(600.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(450.0), y: Pt(630.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
        
        // Draw lines with projecting square caps and bevel joins
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 0.8,
                b: 0.0,
                icc_profile: None,
            })
        },
        Op::SetLineCapStyle { cap: LineCapStyle::ProjectingSquare },
        Op::SetLineJoinStyle { join: LineJoinStyle::Bevel },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point { x: Pt(350.0), y: Pt(580.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(400.0), y: Pt(550.0) },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point { x: Pt(450.0), y: Pt(580.0) },
                        bezier: false,
                    },
                ],
                is_closed: false,
            }
        },
    ]);
    
    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    
    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    
    std::fs::write("./shapes_example.pdf", bytes).unwrap();
    println!("Created shapes_example.pdf");
}