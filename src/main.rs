#![feature(try_from)]

extern crate image;
extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

    ::std::env::set_current_dir(::std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();

    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 500.0, 500.0, "Layer 1");

    let current_layer = doc.get_page(page1).get_layer(layer1);

    // printpdf support 2d graphics only (currently) - Lines, Points, Polygons and SVG Symbols

    // Write the text with font + font size
    // printpdf is made for PDF-X/1A conform documents. 
    // As such, using the default fonts is not permitted. You have to use your own fonts here
/*
    let image = Image { image: ImageXObject { 
        bits_per_component: ColorBits::Bit1,
        clipping_bbox: None,
        color_space: ColorSpace::Greyscale,
        height: 8,
        image_filter: None,
        width: 8,
        interpolate: false,
        image_data: [0x40, 0x60, 0x70, 0x78, 0x78, 0x70, 0x60, 0x40].to_vec(),
    }};

    // translate(x, y), rotate, scale(x, y)
    image.add_to_layer(current_layer.clone(), None, None, Some(30.0), Some(10.0), Some(10.0));
*/

    use std::io::Cursor;
    use std::convert::TryFrom; 
    use image::bmp::BMPDecoder;
    use std::fs::File;
    
    let image_bytes = include_bytes!("../assets/img/BMP_test.bmp");
    let mut reader = Cursor::new(image_bytes.as_ref());

    let decoder = BMPDecoder::new(&mut reader);
    let image2 = Image::try_from(decoder).unwrap();

    // In debug mode
    image2.add_to_layer(current_layer.clone(), None, None, None, None, None);
/*
    let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";
    let roboto_font_file = File::open("assets/fonts/RobotoMedium.ttf").unwrap();
    let roboto_font = doc.add_font(roboto_font_file).unwrap();
    current_layer.use_text(text, 48, 0.0, 200.0, 200.0, roboto_font);
*/
    
/*
    // A special thing is transcoding SVG files directly into PDF (for mapping symbols)    
    // Specify the lower left corner of the SVG
    let svg = doc.add_svg(File::open("./assets/img/SVG_test.svg").unwrap()).unwrap();
    doc.get_page(page1).get_layer(layer1).use_svg(20.0, 20.0, 500.0, 400.0, svg);
*/

    // There is no support for comments, images, annotations, 3D objects, signatures, gradients, etc. yet.
    // Save the PDF file
    doc.save(&mut File::create("test_working.pdf").unwrap()).unwrap();
}
