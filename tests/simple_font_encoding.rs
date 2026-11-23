//! Simple fonts with an embedded CFF program (`/Subtype /Type1`, `/FontFile3 /Subtype /Type1C`):
//! their text follows the `/Encoding` of the font dictionary, and the font survives a parse and a
//! save. The program is the CFF table of `tests/assets/fonts/mock/mock_cff_named.otf`, a
//! name-keyed font with the glyphs `space` and `A` to `J`.
#![cfg(feature = "html")]

use lopdf::{dictionary, Object, Stream};
use printpdf::*;

const MOCK_OTF: &[u8] = include_bytes!("assets/fonts/mock/mock_cff_named.otf");

/// The `CFF ` table of an OpenType font.
fn cff_table(otf: &[u8]) -> Vec<u8> {
    let count = usize::from(u16::from_be_bytes([otf[4], otf[5]]));
    for i in 0..count {
        let rec = &otf[12 + 16 * i..28 + 16 * i];
        if &rec[..4] == b"CFF " {
            let off = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]) as usize;
            let len = u32::from_be_bytes([rec[12], rec[13], rec[14], rec[15]]) as usize;
            return otf[off..off + len].to_vec();
        }
    }
    panic!("no CFF table");
}

/// A one-page PDF that shows `text` in a Type1C font with the given `/Encoding` entry.
fn pdf_with_type1c(encoding: Object, text: &[u8]) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let program = doc.add_object(Stream::new(dictionary! { "Subtype" => "Type1C" }, cff_table(MOCK_OTF)));
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "ABCDEF+MockFont",
        "Flags" => 32,
        "FontBBox" => vec![0.into(), 0.into(), 1000.into(), 1000.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
        "FontFile3" => Object::Reference(program),
    });
    let font = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+MockFont",
        "FirstChar" => 32,
        "LastChar" => 255,
        "Widths" => vec![Object::Integer(500); 224],
        "Encoding" => encoding,
        "FontDescriptor" => Object::Reference(descriptor),
    });
    let mut content = b"BT /F1 12 Tf 50 700 Td (".to_vec();
    content.extend_from_slice(text);
    content.extend_from_slice(b") Tj ET");
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => Object::Reference(font) } },
        "Contents" => Object::Reference(content_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)] }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", Object::Reference(catalog));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

fn text_of(doc: &PdfDocument) -> String {
    doc.extract_text().iter().flatten().flat_map(|s| s.chars()).filter(|c| !c.is_whitespace()).collect()
}

fn parse(bytes: &[u8]) -> PdfDocument {
    let mut w = Vec::new();
    PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut w).expect("parse")
}

#[test]
fn differences_name_the_glyphs_of_the_codes() {
    let encoding: Object = dictionary! {
        "Type" => "Encoding",
        "Differences" => vec![Object::Integer(65), "C".into(), "D".into(), Object::Integer(200), "E".into()],
    }
    .into();
    let doc = parse(&pdf_with_type1c(encoding, b"AB\xC8"));
    assert_eq!(text_of(&doc), "CDE");
}

#[test]
fn the_font_is_written_again_and_its_text_survives_a_save() {
    let encoding: Object = dictionary! {
        "Type" => "Encoding",
        "Differences" => vec![Object::Integer(65), "C".into(), "D".into()],
    }
    .into();
    let doc = parse(&pdf_with_type1c(encoding, b"ABBA"));
    assert_eq!(text_of(&doc), "CDDC");
    assert_eq!(doc.resources.fonts.map.len(), 1, "the font is kept");

    let mut w = Vec::new();
    let saved = doc.save(&PdfSaveOptions::default(), &mut w);
    let again = parse(&saved);
    assert_eq!(text_of(&again), "CDDC", "{w:?}");
    assert_eq!(again.resources.fonts.map.len(), 1, "the saved file has the font");
}

#[test]
fn a_base_encoding_decodes_the_upper_half() {
    // MacRoman 0x8E is é, 0xD5 is ’; the Standard encoding has ﬁ at 0xAE.
    let mac = parse(&pdf_with_type1c("MacRomanEncoding".into(), b"\x8E\xD5"));
    assert_eq!(text_of(&mac), "é\u{2019}");
    let std = parse(&pdf_with_type1c("StandardEncoding".into(), b"\xAE"));
    assert_eq!(text_of(&std), "\u{FB01}");
}
