//! Uses the extend_with feature to add a link to the page.
extern crate printpdf;

use lopdf::{Dictionary, Object, StringFormat::Literal};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let page = doc.get_page(page1);
    let current_layer = page.get_layer(layer1);

    let action = Dictionary::from_iter(vec![
        ("Type", "Action".into()),
        ("S", Object::Name(b"URI".to_vec())),
        (
            "URI",
            Object::String(b"https://github.com/fschutt/printpdf".to_vec(), Literal),
        ),
    ]);

    let annotation = Dictionary::from_iter(vec![
        ("Type", "Annot".into()),
        ("Subtype", Object::Name(b"Link".to_vec())),
        (
            "Rect",
            vec![20.into(), 580.into(), 300.into(), 560.into()].into(),
        ),
        ("C", vec![].into()),
        ("Contents", Object::String("Hello World".into(), Literal)),
        ("A", action.into()),
    ]);

    let annotations =
        Dictionary::from_iter(vec![("Annots", Object::Array(vec![annotation.into()]))]);

    page.extend_with(annotations);

    let text = "There's an invisible annotation with a link covering this text.";
    let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
    current_layer.use_text(text, 10.0, Mm(10.0), Mm(200.0), &font);

    doc.save(&mut BufWriter::new(
        File::create("test_annotations.pdf").unwrap(),
    ))
    .unwrap();
}
