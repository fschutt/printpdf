

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
    
    let text = "abcdefghijklmnopqrstuvwxyz";
    let direct_font = doc.add_font(File::open("assets/fonts/FiraSans-Book.otf").unwrap()).unwrap();
    let indirect_font = current_layer.add_font(direct_font);
    current_layer.use_text(text, 48, 0.0, 200.0, 200.0, indirect_font);
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
