use printpdf::*;

static ROBOTO_TTF: &[u8] = include_bytes!("./assets/fonts/RobotoMedium.ttf");
static SVG: &str = include_str!("./assets/svg/tiger.svg");

fn main() {

    let mut doc = PdfDocument::new("My first document");

    // shape 1 (line)
    let line1 = Line {
        points: vec![
            (Point::new(Mm(100.0), Mm(100.0)), false),
            (Point::new(Mm(100.0), Mm(200.0)), false),
            (Point::new(Mm(300.0), Mm(200.0)), false),
            (Point::new(Mm(300.0), Mm(100.0)), false),
        ],
        is_closed: true,
    };
    let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));

    let mut ops = vec![
        Op::SetOutlineColor { col: outline_color },
        Op::SetOutlineThickness { pt: Pt(10.0) },
        Op::DrawLine { line: line1 },
    ];

    // shape 2 (polygon)
    let line2 = Polygon {
        rings: vec![vec![
            (Point::new(Mm(150.0), Mm(150.0)), false),
            (Point::new(Mm(150.0), Mm(250.0)), false),
            (Point::new(Mm(350.0), Mm(250.0)), false),
        ]],
        mode: PaintMode::FillStroke,
        winding_order: WindingOrder::NonZero,
    };

    let fill_color_2 = Color::Cmyk(Cmyk::new(0.0, 0.0, 0.0, 0.0, None));
    let outline_color_2 = Color::Greyscale(Greyscale::new(0.45, None));
    let dash_pattern = LineDashPattern {
        dash_1: Some(20),
        ..Default::default()
    };

    let extgstate = ExtendedGraphicsStateBuilder::new()
    .with_overprint_stroke(true)
    .with_blend_mode(BlendMode::multiply())
    .build();

    ops.extend_from_slice(&[
        Op::SaveGraphicsState,
        Op::LoadGraphicsState { gs: doc.add_graphics_state(extgstate) },
        Op::SetLineDashPattern { dash: dash_pattern },
        Op::SetLineJoinStyle { join: LineJoinStyle::Round },
        Op::SetLineCapStyle { cap: LineCapStyle::Round },
        Op::SetFillColor { col: fill_color_2 },
        Op::SetOutlineThickness { pt: Pt(15.0) },
        Op::SetOutlineColor { col: outline_color_2 },
        Op::DrawPolygon { polygon: line2 },
        Op::RestoreGraphicsState,
    ]);

    // font loading
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0).unwrap();
    let font_id = doc.add_font(&font);
    
    let fontsize = Pt(33.0);
    let fontsize2 = Pt(18.0);
    ops.extend_from_slice(&[
        Op::StartTextSection,

        Op::SetTextCursor { pos: Point { x: Mm(10.0).into(), y: Mm(100.0).into() } }, // from bottom left
        Op::SetLineHeight { lh: fontsize },
        Op::SetWordSpacing { percent: 300.0 },
        Op::SetCharacterSpacing { multiplier: 10.0 },
        
        Op::WriteText { text: "Lorem ipsum".to_string(), font: font_id.clone(), size: fontsize },
        Op::AddLineBreak,
        Op::WriteText { text: "dolor sit amet".to_string(), font: font_id.clone(), size: fontsize },
        Op::AddLineBreak,

        Op::SetTextRenderingMode { mode: TextRenderingMode::FillStroke },
        Op::SetCharacterSpacing { multiplier: 0.0 },
        Op::SetTextMatrix { matrix: TextMatrix::Rotate(10.0 /* degrees ccw */) },

        Op::WriteText { text: "Lorem ipsum".to_string(), font: font_id.clone(), size: fontsize2 },
        Op::SetLineOffset { multiplier: 10.0 },
        Op::SetTextRenderingMode { mode: TextRenderingMode::Stroke },
        Op::WriteText { text: "dolor sit amet".to_string(), font: font_id.clone(), size: fontsize2 },

        Op::SetTextRenderingMode { mode: TextRenderingMode::FillStroke },
        Op::WriteTextBuiltinFont { text: "dolor sit amet".to_string(), size: Pt(45.0), font: BuiltinFont::Courier },

        Op::EndTextSection,
    ]);

    let svg = Svg::parse(SVG).unwrap();
    let rotation_center_x = Px((svg.width.unwrap_or_default().0 as f32 / 2.0) as usize);
    let rotation_center_y = Px((svg.height.unwrap_or_default().0 as f32 / 2.0) as usize);
    let xobject_id = doc.add_xobject(&svg);
    
    let svg_layer = doc.add_layer(&Layer::new("SVG content"));
    ops.push(Op::BeginLayer { layer_id: svg_layer.clone() });

    for i in 0..10 {
        
        let transform = XObjectTransform {
            rotate: Some(XObjectRotation {
                angle_ccw_degrees: i as f32 * 36.0,
                rotation_center_x: rotation_center_x,
                rotation_center_y: rotation_center_y,
            }),
            translate_x: Some(Mm(i as f32 * 20.0 % 50.0).into()),
            translate_y: Some(Mm(i as f32 * 30.0).into()),
            dpi: Some(300.0),
            scale_x: None,
            scale_y: None,
        };
    
        ops.extend_from_slice(&[
            Op::UseXObject { id: xobject_id.clone(), transform: transform }
        ]);
    }

    ops.push(Op::EndLayer { layer_id: svg_layer.clone() });
    
    let _bookmark_id = doc.add_bookmark("Chapter 1", /* page */ 0);

    // collect pages
    let pages = vec![
        PdfPage::new(Mm(210.0), Mm(297.0), ops.clone()),
        // PdfPage::new(Mm(400.0), Mm(400.0), ops)
    ];

    let bytes = doc.with_pages(pages).save(&PdfSaveOptions::default());
    std::fs::write("./simple.pdf", bytes).unwrap();
}