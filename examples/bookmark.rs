extern crate printpdf;

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, _) = PdfDocument::new("printpdf page test", Mm(210.0), Mm(297.0), "Layer 1");
    doc.add_bookmark("This is a bookmark", page1);

    let (page2, _) = doc.add_page(Mm(297.0), Mm(210.0), "Page 2, Layer 1");
    let _ = doc.get_page(page2).add_layer("Layer 3");
    doc.add_bookmark("This is another bookmark", page2);

    // If this is successful, you should see a PDF with two blank A4 pages and 2 bookmarks
    doc.save(&mut BufWriter::new(
        File::create("test_bookmark.pdf").unwrap(),
    ))
    .unwrap();
}
