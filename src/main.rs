extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (mut doc, page1, layer1) = PdfDocument::new(
                                      PdfPage::new(247.0, 210.0, 
                                          PdfLayer::new("Layer 1")), 
                                  "Hello World PDF!",
                                  "superprogram_v1.1");

/*
    // printpdf support 2d graphics only (currently) - Lines, Points, Polygons and SVG Symbols
    let (page2, layer2) = doc.add_page(10.0, 250.0, PdfLayer::new("Layer2"));
    let layer2 = doc.get_page(page2).add_layer(PdfLayer::new("Layer 2")).unwrap();

    // Write the text with font + font size
    // printpdf is made for PDF-X/1A conform documents. 
    // As such, using the default fonts is not permitted. You have to use your own fonts here
    let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";
    let roboto_font_file = File::open("assets/fonts/Roboto.ttf").unwrap();
    let roboto_font = doc.add_font(roboto_font_file).unwrap();
    doc.get_page(page1).get_layer(layer1).add_text(text, roboto_font, 48, 200.0, 200.0, layer1).unwrap();
    
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
    doc.save(&mut File::create("output.pdf").unwrap()).unwrap();
}