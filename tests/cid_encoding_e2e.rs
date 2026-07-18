//! End-to-end Type0 /Encoding test: a hand-built PDF (via lopdf, independent
//! of printpdf's writer) whose content stream uses 1-BYTE codes through an
//! embedded CMap, against the CID-keyed mock CFF font.
//!
//! The full decode chain under test:
//!
//! ```text
//! bytes --codespace--> 1-byte codes --cidchar--> CIDs --CFF charset⁻¹--> gids
//! ```
//!
//! `mock_cff_cid.otf` (scripts/gen_mock_fonts.py) has charset
//! gid 2..11 -> CID 901, 811, 821, …, 891 and DISTINCT defined advances
//! (A=300, B=350, C=400 at 1000 upm), so glyph identity is asserted through
//! the extracted selection-box widths — arithmetic, not another parser.

#![cfg(feature = "text_layout")]

use lopdf::{dictionary, Document, Object, Stream, StringFormat};
use printpdf::{PdfDocument, PdfParseOptions};

const MOCK_CFF_CID: &[u8] = include_bytes!("./assets/fonts/mock/mock_cff_cid.otf");

/// CMap: 1-byte codes 'A'/'B'/'C' -> the mock charset's CIDs for glyphs A/B/C.
const ENCODING_CMAP: &str = r#"%!PS-Adobe-3.0 Resource-CMap
/CMapName /Mock-1Byte-H def
/WMode 0 def
1 begincodespacerange
<20> <7F>
endcodespacerange
3 begincidchar
<41> 901
<42> 811
<43> 821
endcidchar
endcmap
"#;

const TO_UNICODE: &str = r#"begincmap
3 beginbfchar
<41> <0041>
<42> <0042>
<43> <0043>
endbfchar
endcmap
"#;

fn build_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.7");

    let font_file = doc.add_object(Stream::new(
        dictionary! { "Subtype" => "OpenType" },
        MOCK_CFF_CID.to_vec(),
    ));
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "MockFont-CID",
        "Flags" => 4,
        "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 800.into()],
        "ItalicAngle" => 0,
        "Ascent" => 800,
        "Descent" => -200,
        "CapHeight" => 700,
        "StemV" => 80,
        "FontFile3" => font_file,
    });
    let descendant = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType0",
        "BaseFont" => "MockFont-CID",
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::String(b"Adobe".to_vec(), StringFormat::Literal),
            "Ordering" => Object::String(b"Identity".to_vec(), StringFormat::Literal),
            "Supplement" => 0,
        },
        // Widths keyed by CID (the codes map to CIDs 901/811/821).
        "W" => vec![
            811.into(), vec![Object::from(350), 400.into()].into(),
            901.into(), vec![Object::from(300)].into(),
        ],
        "DW" => 1000,
        "FontDescriptor" => descriptor,
    });
    let encoding = doc.add_object(Stream::new(
        dictionary! { "Type" => "CMap", "CMapName" => "Mock-1Byte-H" },
        ENCODING_CMAP.as_bytes().to_vec(),
    ));
    let to_unicode = doc.add_object(Stream::new(
        dictionary! {},
        TO_UNICODE.as_bytes().to_vec(),
    ));
    let font = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MockFont-CID",
        "Encoding" => encoding,
        "DescendantFonts" => vec![descendant.into()],
        "ToUnicode" => to_unicode,
    });

    // 1-byte codes: "ABC" is literally the bytes 41 42 43.
    let content = b"BT /F1 10 Tf 50 700 Td (ABC) Tj ET".to_vec();
    let contents = doc.add_object(Stream::new(dictionary! {}, content));

    let page_id = doc.new_object_id();
    let pages_id = doc.new_object_id();
    doc.objects.insert(
        page_id,
        Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Contents" => contents,
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font },
            },
        }),
    );
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save");
    bytes
}

#[test]
fn one_byte_codes_through_embedded_cmap_decode_and_measure() {
    let pdf = build_pdf();
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings)
        .expect("parse hand-built PDF");

    // Text decodes through the CMap + ToUnicode.
    let text: String = doc
        .extract_text()
        .iter()
        .flatten()
        .map(|c| c.trim())
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(text, "ABC", "1-byte codes must decode via the embedded CMap");

    // Geometry proves the codes resolved to the RIGHT GLYPHS: the mock font's
    // defined advances are A=300, B=350, C=400 (/1000 em) — at 10pt the glyph
    // boxes must be 3.0 / 3.5 / 4.0 pt wide. A decoder that treated the bytes
    // as 2-byte Identity codes (or skipped the charset) cannot produce these.
    let boxes = doc.extract_text_boxes();
    let line = &boxes[0].lines[0];
    assert_eq!(line.words.len(), 1, "{line:#?}");
    let glyphs = &line.words[0].glyphs;
    assert_eq!(glyphs.len(), 3);
    for (g, expected_w) in glyphs.iter().zip([3.0f32, 3.5, 4.0]) {
        let w = g.bbox[2] - g.bbox[0];
        assert!(
            (w - expected_w).abs() < 0.05,
            "glyph {:?}: width {w} != {expected_w}",
            g.text
        );
    }
}
