use printpdf::*;

static SVG: &str = include_str!("./assets/svg/tiger.svg");
static CAMERA_SVG: &str = include_str!("./assets/svg/AJ_Digital_Camera.svg");

fn main() {
    // Create a new PDF document
    let mut doc = PdfDocument::new("SVG Example");
    let mut ops = Vec::new();

    // Parse and add the tiger SVG
    let tiger_svg = Svg::parse(SVG, &mut Vec::new()).unwrap();
    let tiger_id = doc.add_xobject(&tiger_svg);

    // Add the tiger SVG to the page
    ops.push(Op::UseXobject {
        id: tiger_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(50.0)),
            translate_y: Some(Pt(500.0)),
            scale_x: Some(0.5),
            scale_y: Some(0.5),
            ..Default::default()
        },
    });

    // Parse and add the camera SVG
    let camera_svg = Svg::parse(CAMERA_SVG, &mut Vec::new()).unwrap();
    let camera_id = doc.add_xobject(&camera_svg);

    // Add the camera SVG to the page
    ops.push(Op::UseXobject {
        id: camera_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(300.0)),
            translate_y: Some(Pt(300.0)),
            scale_x: Some(1.0),
            scale_y: Some(1.0),
            ..Default::default()
        },
    });

    // Add some text to explain the SVGs
    ops.extend_from_slice(&[
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(40.0)),
        },
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(12.0) },
        Op::ShowText {
            items: vec![TextItem::Text(
                "This PDF demonstrates embedding SVGs as vector graphics".to_string(),
            )],
        },
        Op::EndTextSection,
    ]);

    // Create a page with our operations
    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);

    // Save the PDF to a file
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    std::fs::write("./svg_example.pdf", bytes).unwrap();
    println!("Created svg_example.pdf");
}
