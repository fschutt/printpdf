extern crate image;
extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

    ::std::env::set_current_dir(::std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();

    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 210.0, 297.0, "My custom layer");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let text = "Lorem ipsum dolor";
    let font = doc.add_font(File::open("assets/fonts/FreeSerif.ttf").unwrap()).unwrap();    
    let font2 = doc.add_font(File::open("assets/fonts/FreeSans.ttf").unwrap()).unwrap();    

    current_layer.begin_text_section();
        current_layer.set_font(&font, 48);
        current_layer.set_text_cursor(0.0, 200.0);
        current_layer.set_line_height(48);
        current_layer.set_text_rendering_mode(TextRenderingMode::Fill);
        current_layer.write_text(text.clone(), &font2);
    current_layer.end_text_section();

    let layer2 = doc.get_page(page1).add_layer("Test 2 Layer");
    
    layer2.begin_text_section();
        layer2.set_font(&font2, 48);
        layer2.set_text_cursor(0.0, 100.0);
        layer2.set_line_height(48);
        layer2.set_text_rendering_mode(TextRenderingMode::Fill);
        layer2.write_text(text.clone(), &font2);
    layer2.end_text_section();

    // There is no support for comments, images, annotations, 3D objects, signatures, gradients, etc. yet.
    // Save the PDF file
    doc.save(&mut File::create("test_working.pdf").unwrap()).unwrap();
}
