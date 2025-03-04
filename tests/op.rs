// tests/op_tests.rs

use printpdf::{
    BuiltinFont, Color, CurTransMat, LayerInternalId, Line, LineCapStyle, LineDashPattern,
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
        },
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
        Op::BeginLayer {
            layer_id: layer_id.clone()
        },
        "begin_layer"
    ));
}

#[test]
fn test_op_text_section() {
    assert!(test_op(Op::StartTextSection, "start_text_section"));
    assert!(test_op(Op::EndTextSection, "end_text_section"));
}

#[test]
fn test_op_write_text() {
    assert!(test_op(
        Op::WriteTextBuiltinFont {
            items: vec![TextItem::Text("Hello, World!".to_string())],
            font: BuiltinFont::Helvetica,
        },
        "write_text"
    ));
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
