use printpdf::{
    shape::{TextAlign, TextHole, TextShapingOptions},
    Color, FontId, LinePoint, Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Point, PolygonRing, Pt,
    Rect, Rgb, TextItem,
};

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("Advanced Text Shaping Example");

    // Add fonts
    let regular_font_bytes = include_bytes!("./assets/fonts/RobotoMedium.ttf");
    let regular_font =
        printpdf::ParsedFont::from_bytes(regular_font_bytes, 0, &mut Vec::new()).unwrap();
    let regular_font_id = doc.add_font(&regular_font);

    // Create a page (A4)
    let page_width = Mm(210.0);
    let page_height = Mm(297.0);

    // Generate page with multiple text shaping examples
    let ops = create_example_page(&doc, &regular_font_id, page_width, page_height);
    let page = PdfPage::new(page_width, page_height, ops);

    // Add page to document
    doc.with_pages(vec![page]);

    // Save PDF
    let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write("text_shaping_advanced.pdf", bytes).unwrap();
    println!("Created text_shaping_advanced.pdf");
}

// Create a page with multiple text shaping examples
fn create_example_page(
    doc: &PdfDocument,
    font_id: &FontId,
    page_width: Mm,
    page_height: Mm,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // Start with a title
    ops.extend(create_title(
        doc,
        font_id,
        "Advanced Text Shaping",
        page_width.into_pt(),
        page_height.into_pt(),
    ));

    // Example 1: Centered text
    ops.extend(create_section_title(
        doc,
        font_id,
        "1. Centered Text",
        Pt(20.0),
        page_height.into_pt() - Pt(50.0),
    ));
    ops.extend(create_centered_text(
        doc,
        font_id,
        "This text is centered horizontally.",
        page_width.into_pt(),
        page_height.into_pt() - Pt(70.0),
    ));

    // Example 2: Right-aligned text
    ops.extend(create_section_title(
        doc,
        font_id,
        "2. Right-Aligned Text",
        Pt(20.0),
        page_height.into_pt() - Pt(100.0),
    ));
    ops.extend(create_aligned_text(
        doc,
        font_id,
        "This text is aligned to the right margin.",
        page_width.into_pt(),
        page_height.into_pt() - Pt(120.0),
        TextAlign::Right,
    ));

    // Example 3: Text with custom letter and word spacing
    ops.extend(create_section_title(
        doc,
        font_id,
        "3. Custom Spacing",
        Pt(20.0),
        page_height.into_pt() - Pt(150.0),
    ));
    ops.extend(create_custom_spacing_text(
        doc,
        font_id,
        "This text has increased letter and word spacing.",
        page_width.into_pt(),
        page_height.into_pt() - Pt(170.0),
    ));

    // Example 4: Text flowing around a hole
    ops.extend(create_section_title(
        doc,
        font_id,
        "4. Text Flow Around Objects",
        Pt(20.0),
        page_height.into_pt() - Pt(200.0),
    ));

    ops.extend(create_text_with_hole(
        doc,
        font_id,
        "This is a longer text that demonstrates how text can flow around objects like images or \
         other content. The text automatically wraps to avoid the rectangular area and continues \
         below it. This is useful for creating magazine-style layouts or technical documentation \
         where text needs to flow around diagrams, tables, or sidebar content.",
        Pt(300.0),
        Point {
            x: Pt(20.0),
            y: page_height.into_pt() - Pt(220.0),
        },
        &Rect {
            x: Pt(100.0),
            y: page_height.into_pt() - Pt(250.0), // Position hole with top at y=250 from top
            width: Pt(80.0),
            height: Pt(50.0),
        },
    ));

    // Example 5: Multi-column text
    ops.extend(create_section_title(
        doc,
        font_id,
        "5. Multi-Column Layout",
        Pt(20.0),
        page_height.into_pt() - Pt(320.0),
    ));
    ops.extend(create_two_column_text(
        doc,
        font_id,
        "This is the first column of text. It demonstrates how text can be flowed into multiple \
         columns on a page, similar to a newspaper or magazine layout. The text automatically \
         wraps to fit within the column width.",
        "This is the second column of text. With the text shaping functionality, you can create \
         sophisticated layouts for reports, newsletters, or any document that requires \
         professional typesetting features.",
        page_width.into_pt(),
        page_height.into_pt() - Pt(340.0),
    ));

    // Example 6: Text measurement for positioning
    ops.extend(create_section_title(
        doc,
        font_id,
        "6. Text Measurement",
        Pt(20.0),
        page_height.into_pt() - Pt(430.0),
    ));
    ops.extend(create_measured_text_in_box(
        doc,
        font_id,
        "This text is centered in a box both horizontally and vertically using text measurement.",
        Rect {
            x: Pt(50.0),
            y: page_height.into_pt() - Pt(510.0), // Position rect with top at y=450
            width: Pt(300.0),
            height: Pt(60.0),
        },
    ));

    // Add a footer
    ops.extend(create_footer(
        doc,
        font_id,
        "Created with printpdf text shaping API",
        page_width.into_pt(),
    ));

    ops
}

// Create a title at the top of the page
fn create_title(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    page_width: Pt,
    page_height: Pt,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // Title background - positioned at the top of the page
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            icc_profile: None,
        }),
    });

    ops.push(Op::DrawPolygon {
        polygon: printpdf::Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: Pt(0.0),
                            y: page_height, // Top left
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: page_width,
                            y: page_height, // Top right
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: page_width,
                            y: page_height - Pt(40.0), // Bottom right
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(0.0),
                            y: page_height - Pt(40.0), // Bottom left
                        },
                        bezier: false,
                    },
                ],
            }],
            mode: printpdf::PaintMode::Fill,
            winding_order: printpdf::WindingOrder::NonZero,
        },
    });

    ops.push(Op::RestoreGraphicsState);

    // Title text
    let options = TextShapingOptions {
        font_size: Pt(24.0),
        max_width: Some(page_width),
        align: TextAlign::Center,
        ..Default::default()
    };

    // TOP LEFT Position the text in the blue bar
    let top_left_origin = Point {
        x: Pt(0.0),
        y: page_height - Pt(5.0),
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            icc_profile: None,
        }),
    });

    ops.push(Op::SetFontSize {
        size: Pt(24.0),
        font: font_id.clone(),
    });

    // Get shaped text ops with correct PDF coordinate system
    let text_ops = shaped_text.get_ops(top_left_origin);

    // Add all text ops except the StartTextSection and EndTextSection
    for op in text_ops.iter().skip(1) {
        match op {
            Op::EndTextSection => break,
            _ => ops.push(op.clone()),
        }
    }

    ops.push(Op::EndTextSection);

    ops
}

// Create a section title
fn create_section_title(doc: &PdfDocument, font_id: &FontId, text: &str, x: Pt, y: Pt) -> Vec<Op> {
    let mut ops = Vec::new();

    let options = TextShapingOptions {
        font_size: Pt(14.0),
        ..Default::default()
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            icc_profile: None,
        }),
    });

    ops.push(Op::SetFontSize {
        size: Pt(14.0),
        font: font_id.clone(),
    });

    // Get shaped text ops with correct PDF coordinate system
    let text_ops = shaped_text.get_ops(Point { x, y });

    // Add all text ops except the StartTextSection and EndTextSection
    for op in text_ops.iter().skip(1) {
        match op {
            Op::EndTextSection => break,
            _ => ops.push(op.clone()),
        }
    }

    ops.push(Op::EndTextSection);

    // Underline
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    // Draw the line 2 points below the text origin (PDF coordinates)
    ops.push(Op::DrawLine {
        line: printpdf::Line {
            points: vec![
                LinePoint {
                    p: Point {
                        x,
                        y: y - options.font_size - Pt(2.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: x + Pt(200.0),
                        y: y - options.font_size - Pt(2.0),
                    },
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
    ops.push(Op::RestoreGraphicsState);

    ops
}

// Create centered text
fn create_centered_text(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    width: Pt,
    y: Pt,
) -> Vec<Op> {
    create_aligned_text(doc, font_id, text, width, y, TextAlign::Center)
}

// Create aligned text (left, center, or right)
fn create_aligned_text(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    width: Pt,
    y: Pt,
    align: TextAlign,
) -> Vec<Op> {
    let options = TextShapingOptions {
        font_size: Pt(12.0),
        max_width: Some(width),
        align,
        ..Default::default()
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();
    let origin = Point { x: Pt(0.0), y };
    shaped_text.get_ops(origin)
}

// Create text with custom letter and word spacing
fn create_custom_spacing_text(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    width: Pt,
    y: Pt,
) -> Vec<Op> {
    let options = TextShapingOptions {
        font_size: Pt(12.0),
        max_width: Some(width),
        letter_spacing: Some(1.5), // 1.5× normal letter spacing
        word_spacing: Some(2.0),   // 2× normal word spacing
        ..Default::default()
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();

    let origin = Point { x: Pt(20.0), y };
    shaped_text.get_ops(origin)
}

// Create text flowing around a hole
fn create_text_with_hole(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    max_text_width: Pt,
    text_position_relative_to_page: Point,
    hole_rect_relative_to_page: &Rect,
) -> Vec<Op> {
    let mut ops = Vec::new();

    ops.extend(create_hole_with_label(
        &hole_rect_relative_to_page,
        font_id,
        "IMAGE",
    ));

    let hole_relative_to_text_origin = Rect {
        width: hole_rect_relative_to_page.width,
        height: hole_rect_relative_to_page.height,
        x: hole_rect_relative_to_page.x - text_position_relative_to_page.x,
        y: (hole_rect_relative_to_page.y + hole_rect_relative_to_page.height)
            - text_position_relative_to_page.y,
    };

    let options = TextShapingOptions {
        font_size: Pt(12.0),
        max_width: Some(max_text_width),
        holes: vec![TextHole {
            rect: hole_relative_to_text_origin,
        }],
        ..Default::default()
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();
    let text_ops = shaped_text.get_ops(text_position_relative_to_page);

    ops.extend(text_ops);
    ops
}

fn create_hole_with_label(hole_rect: &Rect, font_id: &FontId, label: &str) -> Vec<Op> {
    let mut ops = Vec::new();

    // Draw a box for the hole
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r: 0.6,
            g: 0.6,
            b: 0.6,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    // Draw the hole rectangle
    ops.push(Op::DrawPolygon {
        polygon: printpdf::Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: hole_rect.x,
                            y: hole_rect.y,
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(hole_rect.x.0 + hole_rect.width.0),
                            y: hole_rect.y,
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(hole_rect.x.0 + hole_rect.width.0),
                            y: Pt(hole_rect.y.0 - hole_rect.height.0),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: hole_rect.x,
                            y: Pt(hole_rect.y.0 - hole_rect.height.0),
                        },
                        bezier: false,
                    },
                ],
            }],
            mode: printpdf::PaintMode::FillStroke,
            winding_order: printpdf::WindingOrder::NonZero,
        },
    });
    ops.push(Op::RestoreGraphicsState);

    // Add image placeholder text in the hole
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point {
            x: Pt(hole_rect.x.0 + hole_rect.width.0 / 2.0 - 20.0),
            y: Pt(hole_rect.y.0 - hole_rect.height.0 / 2.0),
        },
    });
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.4,
            g: 0.4,
            b: 0.4,
            icc_profile: None,
        }),
    });

    ops.push(Op::SetFontSize {
        size: Pt(12.0),
        font: font_id.clone(),
    });

    ops.push(Op::WriteText {
        items: vec![TextItem::Text(label.to_string())],
        font: font_id.clone(),
    });
    ops.push(Op::EndTextSection);
    ops
}

// Create a two-column text layout
fn create_two_column_text(
    doc: &PdfDocument,
    font_id: &FontId,
    text1: &str,
    text2: &str,
    width: Pt,
    y: Pt,
) -> Vec<Op> {
    let mut ops = Vec::new();

    let column_width = Pt(width.0 / 2.0 - 30.0);

    // Column 1
    let options1 = TextShapingOptions {
        font_size: Pt(12.0),
        line_height: Some(Pt(16.0)),
        max_width: Some(column_width),
        ..Default::default()
    };

    let shaped_text1 = doc.shape_text(text1, font_id, &options1).unwrap();

    // Column 2
    let options2 = TextShapingOptions {
        font_size: Pt(12.0),
        line_height: Some(Pt(16.0)),
        max_width: Some(column_width),
        ..Default::default()
    };

    let shaped_text2 = doc.shape_text(text2, font_id, &options2).unwrap();

    // Draw text for both columns
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            icc_profile: None,
        }),
    });

    // Get text ops for column 1
    let origin1 = Point { x: Pt(20.0), y };
    let text_ops1 = shaped_text1.get_ops(origin1);

    // Get text ops for column 2
    let origin2 = Point {
        x: Pt(width.0 / 2.0 + 10.0),
        y,
    };
    let text_ops2 = shaped_text2.get_ops(origin2);

    // Add all text ops except the StartTextSection and EndTextSection
    for op in text_ops1.iter().skip(1) {
        match op {
            Op::EndTextSection => break,
            _ => ops.push(op.clone()),
        }
    }

    for op in text_ops2.iter().skip(1) {
        match op {
            Op::EndTextSection => break,
            _ => ops.push(op.clone()),
        }
    }

    ops.push(Op::EndTextSection);

    // Draw a line between columns
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r: 0.7,
            g: 0.7,
            b: 0.7,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    ops.push(Op::DrawLine {
        line: printpdf::Line {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(width.0 / 2.0),
                        y: Pt(y.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(width.0 / 2.0),
                        y: Pt(y.0 - 80.0),
                    },
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
    ops.push(Op::RestoreGraphicsState);

    ops
}

// Create text centered in a box both horizontally and vertically
fn create_measured_text_in_box(
    doc: &PdfDocument,
    font_id: &FontId,
    text: &str,
    box_rect: Rect,
) -> Vec<Op> {
    let mut ops = Vec::new();

    // Draw the box
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb {
            r: 0.95,
            g: 0.95,
            b: 1.0,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r: 0.7,
            g: 0.7,
            b: 0.9,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(1.0) });

    ops.push(Op::DrawPolygon {
        polygon: printpdf::Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint {
                        p: Point {
                            x: box_rect.x,
                            y: box_rect.y,
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(box_rect.x.0 + box_rect.width.0),
                            y: box_rect.y,
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: Pt(box_rect.x.0 + box_rect.width.0),
                            y: Pt(box_rect.y.0 - box_rect.height.0),
                        },
                        bezier: false,
                    },
                    LinePoint {
                        p: Point {
                            x: box_rect.x,
                            y: Pt(box_rect.y.0 - box_rect.height.0),
                        },
                        bezier: false,
                    },
                ],
            }],
            mode: printpdf::PaintMode::FillStroke,
            winding_order: printpdf::WindingOrder::NonZero,
        },
    });
    ops.push(Op::RestoreGraphicsState);

    // Measure the text
    let font_size = Pt(12.0);
    let parsed_font = &doc.resources.fonts.map[font_id];
    let options = TextShapingOptions {
        font_size,
        align: TextAlign::Center,
        max_width: Some(box_rect.width),
        ..Default::default()
    };
    let shaped_text = parsed_font.shape_text(text, &options, font_id);
    let (text_width, text_height) = (shaped_text.width, shaped_text.height);

    // Calculate center position
    let x = box_rect.x.0 + (box_rect.width.0 / 2.0) - (text_width / 2.0);
    let y = box_rect.y.0 - (box_rect.height.0 / 2.0) + (text_height / 2.0);

    // Shape the text
    let origin = Point { x: Pt(x), y: Pt(y) };

    // Draw the text
    ops.extend(shaped_text.get_ops(origin));

    ops
}

// Create a footer at the bottom of the page
fn create_footer(doc: &PdfDocument, font_id: &FontId, text: &str, page_width: Pt) -> Vec<Op> {
    let mut ops = Vec::new();
    let footer_y = 20.0; // Distance from bottom of page

    let font_size = Pt(10.0);
    let options = TextShapingOptions {
        font_size,
        max_width: Some(page_width),
        align: TextAlign::Center,
        ..Default::default()
    };

    let origin = Point {
        x: Pt(0.0),
        y: Pt(footer_y + 10.0), // Position slightly above the footer line
    };

    let shaped_text = doc.shape_text(text, font_id, &options).unwrap();

    // Line above footer
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb {
            r: 0.7,
            g: 0.7,
            b: 0.7,
            icc_profile: None,
        }),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

    ops.push(Op::DrawLine {
        line: printpdf::Line {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(50.0),
                        y: Pt(footer_y + 20.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: page_width - Pt(50.0),
                        y: Pt(footer_y + 20.0),
                    },
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
    ops.push(Op::RestoreGraphicsState);

    // Draw footer text
    ops.extend(shaped_text.get_ops(origin));

    ops
}
