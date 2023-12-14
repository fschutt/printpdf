extern crate printpdf;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf rect test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let rect = Rect::new(Mm(30.), Mm(250.), Mm(200.), Mm(290.));

    current_layer.add_rect(rect);

    let rect = Rect::new(Mm(50.), Mm(180.), Mm(120.), Mm(290.))
        .with_mode(PaintMode::Clip)
        .with_winding(WindingOrder::EvenOdd);

    current_layer.add_rect(rect);

    let mut font_reader =
        std::io::Cursor::new(include_bytes!("../assets/fonts/RobotoMedium.ttf").as_ref());
    let font = doc.add_external_font(&mut font_reader).unwrap();

    current_layer.use_text("hello world", 100.0, Mm(10.0), Mm(200.0), &font);
    doc.save(&mut BufWriter::new(File::create("test_rect.pdf").unwrap()))
        .unwrap();
}
