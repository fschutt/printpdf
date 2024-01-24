extern crate printpdf;

use image_crate::codecs::bmp::BmpDecoder;
use printpdf::*;
use std::fs::File;
use std::io::{BufWriter, Cursor};

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // currently, the only reliable file formats are bmp/jpeg/png
    // this is an issue of the image library, not a fault of printpdf

    let image_bytes = include_bytes!("../assets/img/BMP_test.bmp");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = BmpDecoder::new(&mut reader).unwrap();
    let image = Image::try_from(decoder).unwrap();

    let rotation_center_x = Px((image.image.width.0 as f32 / 2.0) as usize);
    let rotation_center_y = Px((image.image.height.0 as f32 / 2.0) as usize);

    // layer,
    image.add_to_layer(
        current_layer.clone(),
        ImageTransform {
            rotate: Some(ImageRotation {
                angle_ccw_degrees: 45.0,
                rotation_center_x,
                rotation_center_y,
            }),
            translate_x: Some(Mm(10.0)),
            translate_y: Some(Mm(10.0)),
            ..Default::default()
        },
    );

    doc.save(&mut BufWriter::new(File::create("test_image.pdf").unwrap()))
        .unwrap();
}
