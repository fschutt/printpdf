extern crate printpdf;

use printpdf::*;
use std::io::Cursor;
use image::{bmp::BmpDecoder, jpeg::JpegDecoder, png::PngDecoder};
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // currently, the only reliable file format is bmp/jpeg/png
    // this is an issue of the image library, not a fault of printpdf

    let image_bytes = include_bytes!("../assets/img/BMP_test.bmp");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = BmpDecoder::new(&mut reader).unwrap();
    let image = Image::try_from(decoder).unwrap();

    // layer,     
    image.add_to_layer(current_layer.clone(), Some(Mm(10.0)), Some(Mm(10.0)), None, None, None, None);

    let image_bytes = include_bytes!("../assets/img/JPG_test.jpg");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = JpegDecoder::new(&mut reader).unwrap();
    let image = Image::try_from(decoder).unwrap();

    // layer,     
    image.add_to_layer(current_layer.clone(), Some(Mm(10.0)), Some(Mm(150.0)), None, None, None, None);

    let image_bytes = include_bytes!("../assets/img/PNG_test.png");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = PngDecoder::new(&mut reader).unwrap();
    let image = Image::try_from(decoder).unwrap();

    // layer,     
    image.add_to_layer(current_layer.clone(), Some(Mm(10.0)), Some(Mm(300.0)), None, None, None, None);

    doc.save(&mut BufWriter::new(File::create("test_image.pdf").unwrap())).unwrap();
}