extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {
    // To prevent empty documents, you must specify at least one page with one layer
    // You can later on add more pages with the add_page() function
    // You also have to specify the title of the PDF and the document creator
    let (doc, _, _) = PdfDocument::new("printpdf page test", 210.0, 297.0, "Layer 1");
    
    // You can add more pages and layers to the PDF. 
    // Just make sure you don't lose the references, otherwise, you can't add things to the layer anymore
    let (page2, _) = doc.add_page(297.0, 210.0,"Page 2, Layer 1");
    let _ = doc.get_page(page2).add_layer("Layer 3");

    // If this is successful, you should see a PDF with two blank A4 pages
    doc.save(&mut File::create("test_pages.pdf").unwrap()).unwrap();
}