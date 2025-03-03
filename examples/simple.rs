use printpdf::*;

static ROBOTO_TTF: &[u8] = include_bytes!("./assets/fonts/RobotoMedium.ttf");
static SVG: &str = include_str!("./assets/svg/tiger.svg");

fn main() {
    let mut doc = PdfDocument::new("My first document");

    let mut ops = vec![];

    let svg = Svg::parse(SVG, &mut Vec::new()).unwrap();
    let rotation_center_x = Px((svg.width.unwrap_or_default().0 as f32 / 2.0) as usize);
    let rotation_center_y = Px((svg.height.unwrap_or_default().0 as f32 / 2.0) as usize);
    let xobject_id = doc.add_xobject(&svg);
    ops.extend_from_slice(&[Op::UseXobject {
        id: xobject_id.clone(),
        transform: XObjectTransform::default(),
    }]);
    // collect pages
    let pages = vec![
        PdfPage::new(Mm(210.0), Mm(297.0), ops.clone()),
        // PdfPage::new(Mm(400.0), Mm(400.0), ops)
    ];

    let bytes = doc.with_pages(pages).save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write("./simple.pdf", bytes).unwrap();
}
