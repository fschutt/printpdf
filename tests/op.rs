// tests/op_tests.rs

use printpdf::{
    Color, CurTransMat, LayerInternalId, Line, LineCapStyle, LineDashPattern,
    LineJoinStyle, LinePoint, Mm, Op, PaintMode, PdfDocument, PdfPage, PdfParseOptions,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rgb, TextItem, TextMatrix, WindingOrder,
};

// Helper function to test operation serialization/deserialization
fn test_op(op: Op, op_name: &str) -> bool {
    let mut doc = PdfDocument::new(&format!("test_{}", op_name));
    let page = PdfPage::new(Mm(210.0), Mm(297.0), vec![op.clone()]);
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    match PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()) {
        Ok(dd) => {
            if dd.pages.is_empty() || dd.pages[0].ops.is_empty() {
                panic!("empty pages for encoded {:?}", [op.clone()]);
            }
            pretty_assertions::assert_eq!(dd.pages[0].ops, vec![op.clone()]);
            true
        }
        Err(e) => {
            panic!("{}", e);
        }
    }
}

#[test]
fn test_op_marker() {
    assert!(test_op(
        Op::Marker {
            id: "test_marker".to_string()
        },
        "marker"
    ));
}

#[test]
fn test_op_graphics_state() {
    assert!(test_op(Op::SaveGraphicsState, "save_graphics_state"));
    assert!(test_op(Op::RestoreGraphicsState, "restore_graphics_state"));
}

#[test]
fn test_op_layer() {
    let layer_id = LayerInternalId::new();
    assert!(test_op(
        Op::BeginOptionalContent {
            layer_id: layer_id.clone()
        },
        "begin_optional_content"
    ));
}

#[test]
fn test_op_text_section() {
    assert!(test_op(Op::StartTextSection, "start_text_section"));
    assert!(test_op(Op::EndTextSection, "end_text_section"));
}

#[test]
fn test_op_write_text() {
    // Test that text operations with proper font setup roundtrip correctly
    let mut doc = PdfDocument::new("test_write_text");
    let page = PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: printpdf::ops::PdfFontHandle::Builtin(printpdf::BuiltinFont::Helvetica),
                size: Pt(12.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Hello, World!".to_string())],
            },
            Op::EndTextSection,
        ],
    );
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    match PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()) {
        Ok(dd) => {
            if dd.pages.is_empty() || dd.pages[0].ops.is_empty() {
                panic!("empty pages");
            }
            
            // The parsed ops should now emit new SetFont and ShowText operations
            pretty_assertions::assert_eq!(dd.pages[0].ops.len(), 4);
            
            // Check each operation type and content
            assert!(matches!(dd.pages[0].ops[0], Op::StartTextSection));
            // Deserialization now emits SetFont instead of SetFontSizeBuiltinFont
            assert!(matches!(
                &dd.pages[0].ops[1],
                Op::SetFont { font: printpdf::ops::PdfFontHandle::Builtin(printpdf::BuiltinFont::Helvetica), size } if *size == Pt(12.0)
            ));
            // Deserialization now emits ShowText instead of WriteTextBuiltinFont
            assert!(matches!(
                &dd.pages[0].ops[2],
                Op::ShowText { items } if items.len() == 1
            ));
            assert!(matches!(dd.pages[0].ops[3], Op::EndTextSection));
        }
        Err(e) => {
            panic!("{}", e);
        }
    }
}

#[test]
fn test_op_write_text_without_font_setup() {
    // Test that text operations without font setup in text mode still work
    // (font was set before BT, or implicitly uses last font)
    let mut doc = PdfDocument::new("test_write_text_no_font");
    let page = PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            // Note: In new API, ShowText without SetFont will serialize but may produce empty/invalid text
            // This tests the edge case where font is implicit from context
            Op::StartTextSection,
            Op::ShowText {
                items: vec![TextItem::Text("Hello, World!".to_string())],
            },
            Op::EndTextSection,
        ],
    );
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    // Parse the PDF back
    let mut parse_warnings = Vec::new();
    match PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut parse_warnings) {
        Ok(dd) => {
            // Should have warnings about missing Tf operator
            assert!(!parse_warnings.is_empty(), "Expected warnings about missing font setup");
            
            if dd.pages.is_empty() || dd.pages[0].ops.is_empty() {
                panic!("empty pages");
            }
            
            // Should still parse the text section markers
            assert!(dd.pages[0].ops.len() >= 2);
            assert!(matches!(dd.pages[0].ops[0], Op::StartTextSection));
            // ShowText may be empty or present depending on serialization behavior
        }
        Err(e) => {
            panic!("{}", e);
        }
    }
}

#[test]
fn test_op_draw_line() {
    let line = Line {
        points: vec![
            LinePoint {
                p: Point {
                    x: Pt(10.0),
                    y: Pt(10.0),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: Pt(100.0),
                    y: Pt(100.0),
                },
                bezier: false,
            },
        ],
        is_closed: false,
    };

    assert!(test_op(Op::DrawLine { line }, "draw_line"));
}

#[test]
fn test_op_draw_polygon() {
    let polygon = Polygon {
        rings: vec![PolygonRing {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(10.0),
                        y: Pt(10.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(100.0),
                        y: Pt(100.0),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(100.0),
                        y: Pt(10.0),
                    },
                    bezier: false,
                },
            ],
        }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    };

    assert!(test_op(Op::DrawPolygon { polygon }, "draw_polygon"));
}

#[test]
fn test_op_color_settings() {
    assert!(test_op(
        Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        "set_fill_color"
    ));

    assert!(test_op(
        Op::SetOutlineColor {
            col: Color::Rgb(Rgb {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                icc_profile: None,
            })
        },
        "set_outline_color"
    ));

    assert!(test_op(
        Op::SetOutlineThickness { pt: Pt(2.0) },
        "set_outline_thickness"
    ));
}

#[test]
fn test_op_line_style() {
    assert!(test_op(
        Op::SetLineCapStyle {
            cap: LineCapStyle::Round
        },
        "set_line_cap_style"
    ));

    assert!(test_op(
        Op::SetLineJoinStyle {
            join: LineJoinStyle::Bevel
        },
        "set_line_join_style"
    ));

    assert!(test_op(
        Op::SetLineDashPattern {
            dash: LineDashPattern {
                offset: 0,
                dash_1: Some(5),
                gap_1: Some(5),
                ..Default::default()
            }
        },
        "set_line_dash_pattern"
    ));
}

#[test]
fn test_op_transform() {
    assert!(test_op(
        Op::SetTransformationMatrix {
            matrix: CurTransMat::Translate(Pt(50.0), Pt(50.0))
        },
        "set_transformation_matrix"
    ));

    assert!(test_op(
        Op::SetTextMatrix {
            matrix: TextMatrix::Translate(Pt(20.0), Pt(20.0))
        },
        "set_text_matrix"
    ));
}
