//! A simple TrueType font whose program has no Unicode `cmap`, such as the subsets that macOS
//! Quartz writes: the codes of its text are the keys of the font's own glyph map, so the text
//! keeps the glyph ids and survives a parse and a save with the glyphs it is shown with.
//!
//! The program is `tests/assets/fonts/mock/mock_ttf.ttf` with its `cmap` replaced by a
//! Macintosh (1,0) subtable that maps the codes `!`, `"` and `#` to the glyphs of `A`, `B` and
//! `C`. The ToUnicode CMap of the font says the same.
#![cfg(feature = "html")]

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

const MOCK_TTF: &[u8] = include_bytes!("assets/fonts/mock/mock_ttf.ttf");

const TO_UNICODE: &[u8] = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
    /CMapName /Adobe-Identity-UCS def /CMapType 2 def \
    1 begincodespacerange <00> <FF> endcodespacerange \
    3 beginbfchar <21> <0041> <22> <0042> <23> <0043> endbfchar \
    endcmap end end";

/// The mock font with a Macintosh cmap only: code 0x21 is the glyph of `A` (glyph 2), 0x22 `B`
/// and 0x23 `C`.
fn quartz_style_font() -> Vec<u8> {
    let mut font = MOCK_TTF.to_vec();
    let tables = usize::from(u16::from_be_bytes([font[4], font[5]]));
    let (offset, length) = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|record| &font[*record..*record + 4] == b"cmap")
        .map(|record| {
            let word = |at: usize| {
                u32::from_be_bytes([font[at], font[at + 1], font[at + 2], font[at + 3]]) as usize
            };
            (word(record + 8), word(record + 12))
        })
        .expect("the mock font has a cmap");
    let mut table: Vec<u8> = Vec::new();
    table.extend([0, 0, 0, 1]); // version 0, one subtable
    table.extend([0, 1, 0, 0, 0, 0, 0, 12]); // platform 1, encoding 0, subtable at 12
    // Format 6: format, length, language, first code, entry count, glyph ids.
    table.extend([0, 6, 0, 16, 0, 0, 0, 0x21, 0, 3, 0, 2, 0, 3, 0, 4]);
    table.resize(length, 0);
    font[offset..offset + length].copy_from_slice(&table);
    font
}

fn numbers(values: &[i64]) -> Object {
    Object::Array(values.iter().map(|v| Object::Integer(*v)).collect())
}

fn reference(id: ObjectId) -> Object {
    Object::Reference(id)
}

/// The font dictionary for the program `ttf`, with `/ToUnicode` if `to_unicode` is set and, if
/// given, an `/Encoding`.
fn simple_font(doc: &mut Document, ttf: &[u8], to_unicode: bool, encoding: Option<&str>) -> ObjectId {
    let mut program = Stream::new(dictionary! { "Length1" => ttf.len() as i64 }, ttf.to_vec());
    let _ = program.compress();
    let program = doc.add_object(program);
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "ABCDEF+MockTT", "Flags" => 4,
        "FontBBox" => numbers(&[0, -200, 1000, 800]), "ItalicAngle" => 0,
        "Ascent" => 800, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
        "FontFile2" => reference(program),
    });
    let mut font = dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "ABCDEF+MockTT",
        "FirstChar" => 33, "LastChar" => 35, "Widths" => numbers(&[600, 600, 600]),
        "FontDescriptor" => reference(descriptor),
    };
    if to_unicode {
        let cmap = doc.add_object(Stream::new(dictionary! {}, TO_UNICODE.to_vec()));
        font.set("ToUnicode", reference(cmap));
    }
    if let Some(encoding) = encoding {
        font.set("Encoding", encoding);
    }
    doc.add_object(font)
}

/// A one-page document that shows `content` with the font `/F1`, directly or in a form.
fn build(in_form: bool, ttf: &[u8], to_unicode: bool, encoding: Option<&str>, content: &[u8]) -> Vec<u8> {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let font = simple_font(&mut doc, ttf, to_unicode, encoding);
    let fonts = dictionary! { "F1" => reference(font) };
    let (page_content, resources): (Vec<u8>, Dictionary) = if in_form {
        let form = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0, 0, 600, 800]),
                "Resources" => dictionary! { "Font" => fonts },
            },
            content.to_vec(),
        ));
        (
            b"/Fm1 Do".to_vec(),
            dictionary! { "XObject" => dictionary! { "Fm1" => reference(form) } },
        )
    } else {
        (content.to_vec(), dictionary! { "Font" => fonts })
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, page_content));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => reference(pages_id), "MediaBox" => numbers(&[0, 0, 600, 800]),
        "Resources" => resources, "Contents" => reference(content_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => vec![reference(page)],
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => reference(pages_id) });
    doc.trailer.set("Root", reference(catalog));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

const CONTENT: &[u8] = b"BT /F1 12 Tf 50 700 Td (!\"#) Tj ET";

fn parse(bytes: &[u8]) -> PdfDocument {
    PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut Vec::new()).expect("parse")
}

fn text(doc: &PdfDocument) -> String {
    doc.extract_text().iter().flatten().flat_map(|s| s.chars()).collect()
}

/// What `ShowText` ops of the document and of its forms hold.
fn shown(doc: &PdfDocument) -> Vec<TextItem> {
    let mut ops: Vec<&Op> = doc.pages[0].ops.iter().collect();
    for xobject in doc.resources.xobjects.map.values() {
        if let XObject::Form(form) = xobject {
            ops.extend(form.ops.iter());
        }
    }
    ops.into_iter()
        .filter_map(|op| match op {
            Op::ShowText { items } => Some(items.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn the_codes_of_a_font_without_a_unicode_cmap_keep_their_glyphs() {
    let doc = parse(&build(false, &quartz_style_font(), true, None, CONTENT));
    assert_eq!(text(&doc), "ABC");
    // The text is glyph ids (2, 3 and 4) with the characters of the ToUnicode CMap.
    let items = shown(&doc);
    let TextItem::GlyphIds(glyphs) = &items[0] else {
        panic!("expected glyph ids, got {items:?}");
    };
    let read: Vec<(u16, Option<&str>)> = glyphs.iter().map(|g| (g.gid, g.cid.as_deref())).collect();
    assert_eq!(read, vec![(2, Some("A")), (3, Some("B")), (4, Some("C"))]);
}

#[test]
fn such_a_font_survives_a_save() {
    for in_form in [false, true] {
        let doc = parse(&build(in_form, &quartz_style_font(), true, None, CONTENT));
        let saved = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
        let again = parse(&saved);
        assert_eq!(text(&again), "ABC", "in a form: {in_form}");
        // The glyphs are the ones the text was shown with: the program has glyphs 2, 3 and 4
        // (and the subset renumbers them in the same order).
        let gids: Vec<u16> = shown(&again)
            .iter()
            .filter_map(|item| match item {
                TextItem::GlyphIds(glyphs) => Some(glyphs.iter().map(|g| g.gid).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(gids.len(), 3, "{gids:?}");
        assert!(gids[0] < gids[1] && gids[1] < gids[2], "{gids:?}");
    }
}

/// A font with a Unicode cmap is read as before: its text is characters.
#[test]
fn a_font_with_a_unicode_cmap_keeps_its_text_as_characters() {
    let doc = parse(&build(false, MOCK_TTF, true, Some("WinAnsiEncoding"), b"BT /F1 12 Tf 50 700 Td (ABC) Tj ET"));
    assert_eq!(text(&doc), "ABC");
    let items = shown(&doc);
    assert_eq!(items, vec![TextItem::Text("ABC".to_string())]);
}

/// A font that has neither a ToUnicode CMap nor an `/Encoding` reads as WinAnsi, and keeps its glyphs.
#[test]
fn a_font_without_characters_for_its_codes_reads_as_winansi_and_keeps_its_glyphs() {
    for in_form in [false, true] {
        let doc = parse(&build(in_form, &quartz_style_font(), false, None, CONTENT));
        assert_eq!(text(&doc), "!\"#", "in a form: {in_form}");
        let TextItem::GlyphIds(glyphs) = &shown(&doc)[0] else {
            panic!("expected glyph ids");
        };
        let read: Vec<(u16, Option<&str>)> =
            glyphs.iter().map(|g| (g.gid, g.cid.as_deref())).collect();
        assert_eq!(read, vec![(2, Some("!")), (3, Some("\"")), (4, Some("#"))]);
        let again = parse(&doc.save(&PdfSaveOptions::default(), &mut Vec::new()));
        assert_eq!(text(&again), "!\"#", "in a form: {in_form}");
    }
}
