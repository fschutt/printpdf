extern crate printpdf;

use image::bmp::BmpDecoder;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::io::Cursor;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // currently, the only reliable file format is bmp (jpeg works, but not in release mode)
    // this is an issue of the image library, not a fault of printpdf

    let image_bytes = include_bytes!("../assets/img/BMP_test.bmp");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = BmpDecoder::new(&mut reader).unwrap();
    let image2 = Image::try_from(decoder).unwrap();

    // layer,
    image2.add_to_layer(current_layer.clone(), None, None, None, None, None, None);

    doc.save(&mut BufWriter::new(File::create("test_image.pdf").unwrap()))
        .unwrap();
}
