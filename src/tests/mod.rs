use *;
use std::fs::File;

#[test]
fn test_simple_empty_file() {

  /*  let (mut doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
    doc.save(&mut File::create("test_simple_empty_file.pdf").unwrap()).unwrap(); */
    
  use std::sync::Arc;

  let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
  Arc::try_unwrap(doc).unwrap().into_inner().unwrap().save(&mut File::create("test_simple_empty_file.pdf").unwrap()).unwrap();
}