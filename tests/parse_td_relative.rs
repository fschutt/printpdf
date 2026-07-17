//! Regression: relative `Td` continuation moves parse to ABSOLUTE SetTextCursor
//! positions. `50 700 Td … 0 -20 Td` must give (50,700) then (50,680), not
//! (50,700) then (0,-20).
use printpdf::*;
use lopdf::{dictionary, Document, Object, Stream, content::{Content, Operation}};

fn make_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources = doc.add_object(dictionary!{"Font"=>dictionary!{"F1"=>font_id}});
    let content = Content { operations: vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["F1".into(), 12.into()]),
        Operation::new("Td", vec![50.into(), 700.into()]),
        Operation::new("Tj", vec![Object::string_literal("Line1")]),
        Operation::new("Td", vec![0.into(), (-20).into()]),
        Operation::new("Tj", vec![Object::string_literal("Line2")]),
        Operation::new("ET", vec![]),
    ]};
    let content_id = doc.add_object(Stream::new(dictionary!{}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary!{
        "Type"=>"Page","Parent"=>pages_id,"Contents"=>content_id,"Resources"=>resources,
        "MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()],
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary!{
        "Type"=>"Pages","Kids"=>vec![page_id.into()],"Count"=>1,
    }));
    let catalog = doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages_id});
    doc.trailer.set("Root", catalog);
    let mut buf = Vec::new(); doc.save_to(&mut buf).unwrap(); buf
}

#[test]
fn td_continuation_is_absolute() {
    let pdf = make_pdf();
    let mut w = Vec::new();
    let d = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut w).unwrap();
    let cursors: Vec<(f32,f32)> = d.pages[0].ops.iter().filter_map(|op| match op {
        Op::SetTextCursor { pos } => Some((pos.x.0, pos.y.0)),
        _ => None,
    }).collect();
    eprintln!("cursors = {cursors:?}");
    assert_eq!(cursors.len(), 2, "two Td moves");
    assert_eq!(cursors[0], (50.0, 700.0));
    assert_eq!(cursors[1], (50.0, 680.0), "second line keeps x=50 and accumulates y");
}
