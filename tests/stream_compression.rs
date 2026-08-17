//! Regression coverage for PDF stream compression.
//!
//! The writer used to advertise `PdfSaveOptions::optimize`, but left
//! `Document::compress()` disabled. Font streams were also excluded because an old
//! malformed-font regression was attributed to compression. These tests verify the
//! actual contract: page and font streams use Flate when optimization is enabled,
//! while `/Length1` keeps the decoded TrueType program size required by ISO 32000.

#![cfg(feature = "text_layout")]

use lopdf::{Document, Object, Stream};
use printpdf::{
    ops::PdfFontHandle,
    units::{Mm, Pt},
    FontId, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, TextItem,
};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
const NOTO_JP_OTF: &[u8] = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");

fn resolve<'a>(doc: &'a Document, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => doc.get_object(*id).expect("dangling PDF reference"),
        other => other,
    }
}

fn name(object: &Object) -> String {
    String::from_utf8_lossy(object.as_name().expect("expected PDF name")).into_owned()
}

fn first_page_content_stream(doc: &Document) -> &Stream {
    let page_id = *doc
        .get_pages()
        .values()
        .next()
        .expect("PDF must contain one page");
    let page = doc.get_dictionary(page_id).expect("page dictionary");
    resolve(doc, page.get(b"Contents").expect("page /Contents"))
        .as_stream()
        .expect("page content stream")
}

fn first_font_program_stream<'a>(doc: &'a Document) -> (&'static str, &'a Stream) {
    let page_id = *doc
        .get_pages()
        .values()
        .next()
        .expect("PDF must contain one page");
    let page = doc.get_dictionary(page_id).expect("page dictionary");
    let resources = resolve(doc, page.get(b"Resources").expect("page /Resources"))
        .as_dict()
        .expect("resources dictionary");
    let fonts = resolve(doc, resources.get(b"Font").expect("resources /Font"))
        .as_dict()
        .expect("font dictionary");
    let font = resolve(doc, fonts.iter().next().expect("one font resource").1)
        .as_dict()
        .expect("Type0 font dictionary");
    let descendants = resolve(doc, font.get(b"DescendantFonts").expect("/DescendantFonts"))
        .as_array()
        .expect("descendant font array");
    let descendant = resolve(doc, descendants.first().expect("one descendant font"))
        .as_dict()
        .expect("descendant font dictionary");
    let descriptor = resolve(
        doc,
        descendant.get(b"FontDescriptor").expect("/FontDescriptor"),
    )
    .as_dict()
    .expect("font descriptor dictionary");

    for (key, bytes) in [
        ("FontFile2", b"FontFile2".as_slice()),
        ("FontFile3", b"FontFile3".as_slice()),
        ("FontFile", b"FontFile".as_slice()),
    ] {
        if let Ok(font_file) = descriptor.get(bytes) {
            return (
                key,
                resolve(doc, font_file)
                    .as_stream()
                    .expect("font program stream"),
            );
        }
    }

    panic!("font descriptor contains no embedded font program");
}

fn filter_name(stream: &Stream) -> Option<String> {
    stream.dict.get(b"Filter").ok().map(name)
}

fn pdf_with_font(font_bytes: &[u8], text: &str, optimize: bool) -> Vec<u8> {
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).expect("font must parse");
    let mut doc = PdfDocument::new("stream-compression-test");
    let font_id: FontId = doc.add_font(&font);
    let repeated_text = text.repeat(80);
    let operations = vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::External(font_id),
            size: Pt(10.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(repeated_text)],
        },
        Op::EndTextSection,
    ];

    doc.with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), operations)])
        .save(
            &PdfSaveOptions {
                optimize,
                subset_fonts: true,
                ..Default::default()
            },
            &mut Vec::new(),
        )
}

#[test]
fn optimized_truetype_page_and_font_streams_are_flate_compressed() {
    let pdf = pdf_with_font(ROBOTO_TTF, "Fatura ", true);
    let doc = Document::load_mem(&pdf).expect("optimized PDF must parse");

    let page = first_page_content_stream(&doc);
    assert_eq!(filter_name(page).as_deref(), Some("FlateDecode"));
    assert!(
        page.content.len() < page.decompressed_content().expect("decode page").len(),
        "compressed page stream must be smaller than its decoded operations"
    );

    let (font_file_key, font) = first_font_program_stream(&doc);
    assert_eq!(font_file_key, "FontFile2");
    assert_eq!(filter_name(font).as_deref(), Some("FlateDecode"));
    let decoded_font = font.decompressed_content().expect("decode TrueType font");
    assert!(font.content.len() < decoded_font.len());
    assert_eq!(
        font.dict
            .get(b"Length1")
            .expect("TrueType /Length1")
            .as_i64()
            .expect("integer /Length1"),
        decoded_font.len() as i64,
        "/Length1 must be the decoded TrueType program size"
    );
    ParsedFont::from_bytes(&decoded_font, 0, &mut Vec::new())
        .expect("decoded FontFile2 must remain a valid font");
}

#[test]
fn optimized_cff_font_stream_is_flate_compressed() {
    let pdf = pdf_with_font(NOTO_JP_OTF, "日本語 ", true);
    let doc = Document::load_mem(&pdf).expect("optimized PDF must parse");
    let (font_file_key, font) = first_font_program_stream(&doc);

    assert_eq!(font_file_key, "FontFile3");
    assert_eq!(filter_name(font).as_deref(), Some("FlateDecode"));
    assert!(
        matches!(
            font.dict.get(b"Subtype").map(name).as_deref(),
            Ok("CIDFontType0C") | Ok("OpenType")
        ),
        "FontFile3 must retain its CFF/OpenType subtype"
    );
    let decoded_font = font.decompressed_content().expect("decode CFF font");
    assert!(!decoded_font.is_empty());
    assert!(font.content.len() < decoded_font.len());
}

#[test]
fn optimize_false_keeps_streams_plain_but_truetype_length1_valid() {
    let pdf = pdf_with_font(ROBOTO_TTF, "Fatura ", false);
    let doc = Document::load_mem(&pdf).expect("plain PDF must parse");
    let page = first_page_content_stream(&doc);
    let (_, font) = first_font_program_stream(&doc);

    assert_eq!(filter_name(page), None);
    assert_eq!(filter_name(font), None);
    assert_eq!(
        font.dict
            .get(b"Length1")
            .expect("TrueType /Length1")
            .as_i64()
            .expect("integer /Length1"),
        font.content.len() as i64
    );
}

#[test]
fn optimization_materially_reduces_the_same_document() {
    let plain = pdf_with_font(ROBOTO_TTF, "Fatura ", false);
    let optimized = pdf_with_font(ROBOTO_TTF, "Fatura ", true);

    assert!(
        optimized.len() * 4 < plain.len() * 3,
        "optimized PDF should be at least 25% smaller: plain={} optimized={}",
        plain.len(),
        optimized.len()
    );
}
