use printpdf::*;

static ROBOTO_TTF: &[u8] = include_bytes!("./assets/fonts/RobotoMedium.ttf");

fn main() {
    let mut doc = PdfDocument::new("My first document");
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let fid = doc.add_font(&font);
    let ops = vec![Op::WriteText {
        items: vec!["Hello World!".into()],
        size: Pt(20.0),
        font: fid,
    }];
    let page = PdfPage::new(Mm(100.0), Mm(100.0), ops);
    let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::web(), &mut Vec::new());
    std::fs::write("./helloworld.svg", svg).unwrap();
    std::fs::write(
        "./helloworld.pdf",
        doc.with_pages(vec![page])
            .save(&PdfSaveOptions::default(), &mut Vec::new()),
    )
    .unwrap();
}
