

extern crate image;
extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

    ::std::env::set_current_dir(::std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();

    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 210.0, 297.0, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let text = "lksad";
    let text2 = "AB CDEFG";
    let font = doc.add_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
    current_layer.use_text(text.clone(), 48, 0.0, 200.0, &font);
    
    let font2 = doc.add_font(File::open("assets/fonts/leaguespartan-bold.ttf").unwrap()).unwrap();
    
    current_layer.begin_text_section();
        current_layer.set_font(&font2, 33);
        current_layer.set_text_cursor(10.0, 10.0);
        current_layer.set_line_height(33);
        current_layer.set_word_spacing(3000);
        current_layer.set_text_rendering_mode(TextRenderingMode::Stroke);
        current_layer.write_text(text2, &font2);
        current_layer.set_line_offset(10);
        current_layer.write_text(text, &font2);
    current_layer.end_text_section();

    // current_layer.use_text(text.clone(), 24, None, 200.0, 150.0, &font);
    // current_layer.use_text(text.clone(), 12, None, 200.0, 100.0, &font);
/*
    // currently, the only reliable file format is bmp (jpeg works, but not in release mode)
    // this is an issue of the image library, not a fault of printpdf
    let mut image_file = File::open("assets/img/BMP_test.bmp").unwrap();
    let image = Image::try_from(image::bmp::BMPDecoder::new(&mut image_file)).unwrap();
    // translate x, translate y, rotate, scale x, scale y
    // by default, an image is optimized to 300 DPI (if scale is None)
    // rotations and translations are always in relation to the lower left corner
    image.add_to_layer(current_layer.clone(), None, None, None, None, None, None);
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
