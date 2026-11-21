//! `ParsedFont::from_shared_bytes`: a face parsed from bytes the caller holds shared keeps that
//! allocation (no copy) and is the face `from_bytes` makes of the same bytes; it embeds.
#![cfg(feature = "text_layout")]

use std::sync::Arc;

use printpdf::{
    units::{Mm, Pt},
    Op, ParsedFont, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions, TextItem,
};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// The face keeps the caller's allocation: its source bytes are those bytes, at that address
/// (mutation: `FontBytes::Owned(Arc::from(bytes.to_vec()))` fails it)
#[test]
fn a_face_from_shared_bytes_keeps_them() {
    let bytes: Arc<[u8]> = Arc::from(ROBOTO_TTF);
    let face = ParsedFont::from_shared_bytes(bytes.clone(), 0, &mut Vec::new()).unwrap();
    let source = face.source_bytes().expect("source bytes");
    assert_eq!(source.as_slice().as_ptr(), bytes.as_ptr(), "the caller's allocation, not a copy");
    assert_eq!(source.as_slice(), &bytes[..]);
    assert!(Arc::strong_count(&bytes) >= 2, "the face holds it");
}

/// The same face as `from_bytes`: glyph count, PDF metrics, glyph indices and advances, the bytes
/// to embed (mutation: another font index fails it on a collection; here a face of other bytes)
#[test]
fn a_face_from_shared_bytes_is_the_face_from_bytes() {
    let shared = ParsedFont::from_shared_bytes(Arc::from(ROBOTO_TTF), 0, &mut Vec::new()).unwrap();
    let copied = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    assert_eq!(shared.num_glyphs, copied.num_glyphs);
    assert_eq!(shared.pdf_font_metrics, copied.pdf_font_metrics);
    for c in "Привет, Höhe 400 AVWay".chars() {
        let (a, b) = (shared.lookup_glyph_index(c as u32), copied.lookup_glyph_index(c as u32));
        assert_eq!(a, b, "{c}");
        if let Some(g) = a {
            assert_eq!(shared.get_horizontal_advance(g), copied.get_horizontal_advance(g), "{c}");
        }
    }
    assert_eq!(shared.source_bytes().unwrap().as_slice(), copied.source_bytes().unwrap().as_slice());
}

/// A face from shared bytes embeds: a document with it saved and parsed again has the font
/// (the invariant `test_custom_font_roundtrip` checks for `from_bytes`)
#[test]
fn a_face_from_shared_bytes_embeds() {
    let font = ParsedFont::from_shared_bytes(Arc::from(ROBOTO_TTF), 0, &mut Vec::new()).unwrap();
    let mut pdf = PdfDocument::new("Test");
    let font_id = pdf.add_font(&font);
    let bytes = pdf
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(210.0),
            vec![
                Op::StartTextSection,
                Op::SetFont { font: printpdf::ops::PdfFontHandle::External(font_id.clone()), size: Pt(20.0) },
                Op::ShowText { items: vec![TextItem::Text("Привет, как дела?".to_string())] },
                Op::EndTextSection,
            ],
        )])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions { fail_on_error: false }, &mut Vec::new()).unwrap();
    assert!(!parsed.resources.fonts.map.is_empty(), "the font is embedded");
}
