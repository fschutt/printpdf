#![feature(alloc_system)]
extern crate alloc_system;

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

    // You can add more pages and layers to the PDF. 
    // Just make sure you don't lose the references, otherwise, you can't add things to the layer anymore
    // let (page2, layer1) = doc.add_page(500.0, 500.0,"Page 2, Layer 1");
    // let layer3 = doc.get_page(page2).add_layer("Layer 3");

    // printpdf support 2d graphics only (currently) - Lines, Points, Polygons and SVG Symbols

    // Write the text with font + font size
    // printpdf is made for PDF-X/1A conform documents. 
    // As such, using the default fonts is not permitted. You have to use your own fonts here

    current_layer.add_image(ImageXObject { 
        bits_per_component: ColorBits::Bit1,
        clipping_bbox: None,
        color_space: ColorSpace::Greyscale,
        height: 8,
        image_filter: None,
        width: 8,
        interpolate: false,
        image_data: [0x40, 0x60, 0x70, 0x78, 0x78, 0x70, 0x60, 0x40].to_vec(),
    });

/*
    let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";
    let roboto_font_file = File::open("./assets/fonts/RobotoMedium.ttf").unwrap();
    let roboto_font = doc.add_font(roboto_font_file).unwrap();
    doc.get_page(page1).get_layer(layer1).use_text(text, 48, 0.0, 200.0, 200.0, roboto_font);
*/

    /// Adding images
    
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