use std::fs::File;
use std::io::BufWriter;
use printpdf::BuiltinFont;
use printpdf::{Mm, PdfDocument, Point};

#[test]
fn pdf_document_with_table() {
    let (document, page_index, layer_index) = PdfDocument::new("Rechnung", Mm(210.0), Mm(297.0), "Ebene");
    let font = document.add_builtin_font(BuiltinFont::TimesRoman).unwrap();
    let current_layer = document.get_page(page_index).get_layer(layer_index);

    current_layer.add_table(3, 10, Point::new(Mm(20.0), Mm(150.0)), Mm(100.0), Mm(100.0));
    document.save(&mut BufWriter::new(File::create("test_table.pdf").unwrap())).unwrap();
}