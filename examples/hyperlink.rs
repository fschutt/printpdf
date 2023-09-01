extern crate printpdf;

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf graphics test", Mm(595.276), Mm(841.89), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let text = "Lorem ipsum";
    let font = doc.add_builtin_font(BuiltinFont::TimesBoldItalic).unwrap();
    current_layer.use_text(text, 48.0, Mm(10.0), Mm(200.0), &font);

    current_layer.add_link_annotation(LinkAnnotation::new(
        printpdf::Rect::new(Mm(10.0), Mm(200.0), Mm(100.0), Mm(212.0)),
        Some(printpdf::BorderArray::default()),
        Some(printpdf::ColorArray::default()),
        printpdf::Actions::uri("https://www.google.com/".to_string()),
        Some(printpdf::HighlightingMode::Invert),
    ));

    doc.save(&mut BufWriter::new(
        File::create("test_hyperlink.pdf").unwrap(),
    ))
    .unwrap();
}
