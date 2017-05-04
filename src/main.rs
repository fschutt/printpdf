extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

    // To prevent empty documents, you must specify at least one page
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let mut doc = PdfDocument::new(
                   PdfPage::new(247.0, 210.0), 
                  "Hello World PDF!",
                  "superprogram_v1.1");

    let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";

    // printpdf is made for PDF-X/1A conform documents. 
    // As such, using the default fonts is not permitted. You have to use your own fonts here
    let roboto_font_path = "assets/fonts/Roboto.ttf";
    let roboto_font = doc.add_font(File::open(roboto_font_path).unwrap()).unwrap();

    // It isn't allowed to create anything without specifying the layer and page
    // that the content should be on. If the page isn't valid, an error will be returned
    let layer1 = doc.add_layer("Layer 1", &0).unwrap();

    // Set the horizonal + vertical offset from the top left corner in pt
    // get_* functions do not change the state of the document
    let marker = doc.add_marker(100.0, 100.0, &layer1).unwrap();

    // Write the text with font + font size
    doc.add_text(text, roboto_font, 48, &marker).unwrap();

    // printpdf support 2d graphics only (currently) - Lines, Points, Polygons and SVG Symbols
    let page2 = doc.add_page(250.0, 250.0);
    let layer2 = doc.add_layer("Layer 2", &page2).unwrap();

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
    doc.add_line(points, &layer2, Some(&outline), None).unwrap();

    // A special thing is transcoding SVG files directly into PDF (for mapping symbols)    
    // Specify the lower left corner of the SVG
    let marker6 = doc.add_marker(700.0, 700.0, &layer2).unwrap();
    let svg = doc.add_svg(File::open("assets/svg/sample.svg").unwrap()).unwrap();
    doc.add_svg_at(&svg, 20.0, 20.0, &marker6);

    // There is no support for comments, images, annotations, 3D objects, signatures, gradients, etc. yet.
    // Save the PDF file
    doc.save(&mut File::create("output.pdf").unwrap()).unwrap();
}