use api::*;
use std::fs::File;

#[test]
fn test_simple_empty_file() {

    let (doc, _, _) = PdfDocument::new(PdfPage::new(247.0, 210.0,PdfLayer::new("Layer 1")), 
                                       "Hello World PDF!", "superprogram_v1.1");

    doc.save(&mut File::create("test_simple_empty_file.pdf").unwrap()).unwrap();

}