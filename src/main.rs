extern crate printpdf;

use printpdf::*;
use std::fs::File;
use std::borrow::BorrowMut;
use std::sync::Arc;

fn main() {

    ::std::env::set_current_dir(::std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();

    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");

    // printpdf support 2d graphics only (currently) - Lines, Points, Polygons and SVG Symbols
    let (page2, layer1) = { doc.lock().unwrap().add_page(10.0, 250.0,"Page 2, Layer 1") };
/*
    let layer2 = { doc.lock().unwrap().get_page_mut(page2).add_layer("Layer 2") };
    let layer3 = { doc.lock().unwrap().get_page_mut(page2).add_layer("Layer 3") };

    // Write the text with font + font size
    // printpdf is made for PDF-X/1A conform documents. 
    // As such, using the default fonts is not permitted. You have to use your own fonts here
    let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";
    let roboto_font_file = File::open("assets/fonts/RobotoMedium.ttf").unwrap();
    let roboto_font = { doc.lock().unwrap().add_font(roboto_font_file).unwrap() };
    // println!("{:?}", doc);
    { doc.lock().unwrap().get_page_mut(page1).get_layer_mut(layer1).use_text(text, 48, 200.0, 200.0, roboto_font); }
    
    let point1  = Point::new(200.0, 200.0);
    let point2  = Point::new(200.0, 200.0);
    let point3  = Point::new(200.0, 200.0);
    let point4  = Point::new(200.0, 200.0);

    let points = vec![(point1, false),
                      (point2, false),
                      (point3, false),
                      (point4, false)];

    use api::types::plugins::graphics::Outline;
    use api::types::plugins::graphics::*;
    let outline = Outline::new(Color::Cmyk(Cmyk::new(1.0, 0.75, 0.0, 0.0, None)), 5);
    doc.add_line(points, Some(&outline), None, layer2).unwrap();

    // A special thing is transcoding SVG files directly into PDF (for mapping symbols)    
    // Specify the lower left corner of the SVG
    let svg = doc.add_svg(File::open("assets/svg/sample.svg").unwrap()).unwrap();
    doc.add_svg_at(svg, 20.0, 20.0, 700.0, 700.0, layer2);
*/
    // There is no support for comments, images, annotations, 3D objects, signatures, gradients, etc. yet.
    // Save the PDF file
    Arc::try_unwrap(doc).unwrap().into_inner().unwrap().save(&mut File::create("test_working.pdf").unwrap()).unwrap();
}