use printpdf::*;

static SVG: &str = include_str!("./assets/svg/tiger.svg");

fn main() {
    let mut doc = PdfDocument::new("My first document");

    let mut ops = vec![];

    let svg = Svg::parse(SVG, &mut Vec::new()).unwrap();
    let xobject_id = doc.add_xobject(&svg);
    ops.extend_from_slice(&[Op::UseXobject {
        id: xobject_id.clone(),
        transform: XObjectTransform::default(),
    }]);
    // collect pages
    let pages = vec![PdfPage::new(Mm(210.0), Mm(297.0), ops.clone())];

    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write("./simple.pdf", bytes).unwrap();
}
