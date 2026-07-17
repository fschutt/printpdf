//! Regression tests for font-aware text decoding in the PDF parser.
//!
//! The pre-0.12 parser guessed the code width of every text-showing string
//! from its *byte length parity*: even-length strings were decoded as 2-byte
//! CIDs, odd-length strings as single bytes. That heuristic garbled builtin
//! (WinAnsi) text, wrote NUL-interleaved strings back out, and made the
//! builtin/external decision by resource *name* ("F1" == Times-Roman), which
//! mis-claimed foreign documents' embedded fonts.

use printpdf::*;

static ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

fn save(doc: &PdfDocument) -> Vec<u8> {
    doc.save(&PdfSaveOptions::default(), &mut Vec::new())
}

fn parse(bytes: &[u8]) -> PdfDocument {
    PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut Vec::new())
        .expect("parse failed")
}

fn parse_with_warnings(bytes: &[u8]) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut w = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut w).expect("parse failed");
    (doc, w)
}

fn all_text(doc: &PdfDocument) -> String {
    doc.extract_text()
        .iter()
        .flatten()
        .flat_map(|c| c.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_page(items: Vec<TextItem>, font: PdfFontHandle) -> PdfPage {
    let ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(270.0)),
        },
        Op::SetFont {
            font,
            size: Pt(12.0),
        },
        Op::ShowText { items },
        Op::EndTextSection,
    ];
    PdfPage::new(Mm(210.0), Mm(297.0), ops)
}

/// Builtin-font text whose byte length is EVEN. The parity heuristic used to
/// decode "Hi" as the single 16-bit glyph id 0x4869.
#[test]
fn builtin_font_even_length_text_roundtrips() {
    let mut doc = PdfDocument::new("even");
    doc.pages.push(text_page(
        vec![TextItem::Text("Hi".to_string())],
        PdfFontHandle::Builtin(BuiltinFont::Helvetica),
    ));
    let parsed = parse(&save(&doc));
    assert_eq!(all_text(&parsed), "Hi");

    // The decoded op must be Text (re-encodable as WinAnsi), not GlyphIds.
    let has_text_item = parsed.pages[0].ops.iter().any(|op| {
        matches!(op, Op::ShowText { items } if items.iter().any(|i| matches!(i, TextItem::Text(t) if t == "Hi")))
    });
    assert!(has_text_item, "expected TextItem::Text(\"Hi\"), got: {:?}",
        parsed.pages[0].ops.iter().filter(|o| matches!(o, Op::ShowText { .. })).collect::<Vec<_>>());
}

/// Builtin-font text with odd byte length used to become per-byte "glyph ids"
/// that were re-encoded as 2-byte hex strings — NUL-interleaved WinAnsi.
#[test]
fn builtin_font_odd_length_text_roundtrips_bytewise() {
    let mut doc = PdfDocument::new("odd");
    doc.pages.push(text_page(
        vec![TextItem::Text("Hello Helvetica".to_string())],
        PdfFontHandle::Builtin(BuiltinFont::Helvetica),
    ));
    let bytes1 = save(&doc);
    let parsed1 = parse(&bytes1);
    assert_eq!(all_text(&parsed1), "Hello Helvetica");

    // Second round trip must not lose or alter the text either.
    let bytes2 = save(&parsed1);
    let parsed2 = parse(&bytes2);
    assert_eq!(all_text(&parsed2), "Hello Helvetica");

    // The re-saved content stream must contain the WinAnsi literal (allowing
    // for stream compression, check via re-parse instead of raw bytes): no
    // U+0000 or U+FFFD may appear in the extracted text.
    assert!(!all_text(&parsed2).contains('\u{0}'));
    assert!(!all_text(&parsed2).contains('\u{FFFD}'));
}

/// WinAnsi codepoints outside ASCII (€, em-dash, umlauts) must survive
/// decode + re-encode.
#[test]
fn builtin_font_winansi_specials_roundtrip() {
    let text = "Caf\u{E9} \u{2014} \u{20AC}5 \u{201E}quote\u{201C}";
    let mut doc = PdfDocument::new("winansi");
    doc.pages.push(text_page(
        vec![TextItem::Text(text.to_string())],
        PdfFontHandle::Builtin(BuiltinFont::TimesRoman),
    ));
    let parsed = parse(&save(&doc));
    assert_eq!(all_text(&parsed), text.split_whitespace().collect::<Vec<_>>().join(" "));

    let parsed2 = parse(&save(&parsed));
    assert_eq!(all_text(&parsed2), all_text(&parsed));
}

/// TJ arrays with kerning offsets: offsets must stay offsets, strings must
/// decode per the current (builtin) font.
#[test]
fn builtin_font_tj_with_kerning_roundtrips() {
    let mut doc = PdfDocument::new("kerning");
    doc.pages.push(text_page(
        vec![
            TextItem::Text("AW".to_string()),
            TextItem::Offset(-120.0),
            TextItem::Text("AY".to_string()),
        ],
        PdfFontHandle::Builtin(BuiltinFont::Helvetica),
    ));
    let parsed = parse(&save(&doc));
    // Offset < -100 becomes a space in extraction
    assert_eq!(all_text(&parsed), "AW AY");
}

/// External (embedded, Type0/Identity-H) font: glyph runs must come back with
/// their Unicode attached (from the regenerated ToUnicode CMap), and a second
/// save/parse must preserve the text although the font gets re-subset and its
/// glyph ids renumbered.
#[test]
fn external_font_text_roundtrips_twice() {
    let roboto = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let mut doc = PdfDocument::new("external");
    let font_id = doc.add_font(&roboto);
    doc.pages.push(text_page(
        vec![TextItem::Text("External font text".to_string())],
        PdfFontHandle::External(font_id),
    ));

    let bytes1 = save(&doc);
    let parsed1 = parse(&bytes1);
    assert_eq!(all_text(&parsed1), "External font text");

    // The parsed run must be GlyphIds with per-glyph unicode (cid) attached.
    let cid_ok = parsed1.pages[0].ops.iter().any(|op| match op {
        Op::ShowText { items } => items.iter().any(|i| match i {
            TextItem::GlyphIds(glyphs) => {
                !glyphs.is_empty() && glyphs.iter().all(|g| g.cid.is_some())
            }
            _ => false,
        }),
        _ => false,
    });
    assert!(cid_ok, "expected GlyphIds with cid (unicode) populated");

    let bytes2 = save(&parsed1);
    let parsed2 = parse(&bytes2);
    assert_eq!(all_text(&parsed2), "External font text");
}

/// Build a document whose *embedded* font is stored under the resource name
/// "F1" — which collides with printpdf's F1-F14 builtin naming convention.
/// Foreign PDFs use F0/F1/F2... resource names for embedded fonts all the time.
fn doc_with_embedded_font_named_f1() -> Vec<u8> {
    let roboto = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let mut doc = PdfDocument::new("f1-collision");
    // Force the resource name "F1" (add_font would generate a random id).
    let fid = FontId("F1".to_string());
    doc.resources
        .fonts
        .map
        .insert(fid.clone(), printpdf::font::PdfFont::new(roboto));
    doc.pages.push(text_page(
        vec![TextItem::Text("Definitely not Times".to_string())],
        PdfFontHandle::External(fid),
    ));
    save(&doc)
}

/// PARSE side of the F1 collision: the old parser mapped the resource name
/// "F1" straight to builtin Times-Roman, garbling the decode. The document's
/// actual font resources must take priority over the F1-F14 naming heuristic.
#[test]
fn embedded_font_named_f1_is_not_misdetected_as_builtin() {
    let bytes = doc_with_embedded_font_named_f1();
    let parsed = parse(&bytes);

    // The font must come back as an external font resource named F1...
    assert!(
        parsed.resources.fonts.map.contains_key(&FontId("F1".to_string())),
        "external font F1 missing from parsed resources: {:?}",
        parsed.resources.fonts.map.keys().collect::<Vec<_>>()
    );
    // ...and the SetFont op must reference it (not builtin Times-Roman).
    let set_font_external = parsed.pages[0].ops.iter().any(|op| {
        matches!(op, Op::SetFont { font: PdfFontHandle::External(f), .. } if f.0 == "F1")
    });
    assert!(set_font_external, "SetFont must resolve F1 to the embedded font");
}

/// SAVE side of the F1 collision — executable repro of a bug in
/// src/serialize.rs (`resolve_current_font`, and the `global_font_dict`
/// key space): when the current font resource is called "F1",
/// `BuiltinFont::from_id("F1")` claims it as Times-Roman *before* the
/// document's own `font_infos` are consulted, so the text is written as
/// WinAnsi bytes against a Type0/Identity-H font (mojibake), and the builtin
/// font dict can overwrite the embedded font's entry in the global font
/// dictionary.
///
/// Fix (serialize.rs, out of scope for the parser): in
/// `resolve_current_font`, look up `font_infos.get(&FontId(r))` FIRST and
/// only fall back to `BuiltinFont::from_id(r)` when the resource is not an
/// actual font of the document; keep builtin font dict keys from clobbering
/// same-named embedded fonts in `global_font_dict`.
#[test]
// Un-ignored 2026-07-17: resolve_current_font now consults embedded fonts first
// and builtin dicts no longer clobber same-named embedded entries.
fn embedded_font_named_f1_text_survives_roundtrip() {
    let bytes = doc_with_embedded_font_named_f1();
    let parsed = parse(&bytes);
    assert_eq!(all_text(&parsed), "Definitely not Times");

    let parsed2 = parse(&save(&parsed));
    assert_eq!(all_text(&parsed2), "Definitely not Times");
}

/// `'` (move-to-next-line-show-text) used to parse into `Op::Unknown`, which
/// the secure serializer drops — deleting the text on round trip.
#[test]
fn apostrophe_operator_text_survives_roundtrip() {
    use lopdf::{dictionary, Object, Stream};
    // Hand-craft a PDF that uses the `'` operator with a builtin font.
    let mut doc = lopdf::Document::with_version("1.4");
    let font_dict_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let content = b"BT /F5 12 Tf 14 TL 50 700 Td (first line) Tj (quoted line) ' ET".to_vec();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let font_res = dictionary! { "F5" => Object::Reference(font_dict_id) };
    let resources = dictionary! { "Font" => Object::Dictionary(font_res) };
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => Object::Dictionary(resources),
        "Contents" => Object::Reference(content_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => vec![Object::Reference(page_id)],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();

    let parsed = parse(&bytes);
    let text = all_text(&parsed);
    assert!(text.contains("first line"), "missing Tj text: {text:?}");
    assert!(text.contains("quoted line"), "missing ' text: {text:?}");

    // And it must survive a printpdf round trip.
    let parsed2 = parse(&save(&parsed));
    let text2 = all_text(&parsed2);
    assert!(text2.contains("quoted line"), "' text lost on round trip: {text2:?}");
}

/// A Type0 font *without* a /ToUnicode CMap: the parser must synthesize one
/// from the font's own cmap table so extraction and the re-saved ToUnicode
/// still work (previously every glyph mapped to U+FFFD after a round trip).
#[test]
fn cid_font_without_tounicode_gets_synthesized_mapping() {
    let roboto = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).unwrap();
    let gid_h = roboto.lookup_glyph_index('H' as u32).expect("Roboto has H");
    let gid_i = roboto.lookup_glyph_index('i' as u32).expect("Roboto has i");

    use lopdf::{dictionary, Object, Stream};
    let mut doc = lopdf::Document::with_version("1.4");
    let font_file_id = doc.add_object(Stream::new(dictionary! {}, ROBOTO_TTF.to_vec()));
    let descriptor_id = doc.add_object(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "Roboto",
        "Flags" => 4,
        "FontFile2" => Object::Reference(font_file_id),
    });
    let descendant_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "Roboto",
        "CIDSystemInfo" => dictionary! {
            "Registry" => Object::String(b"Adobe".to_vec(), lopdf::StringFormat::Literal),
            "Ordering" => Object::String(b"Identity".to_vec(), lopdf::StringFormat::Literal),
            "Supplement" => 0,
        },
        "FontDescriptor" => Object::Reference(descriptor_id),
        "CIDToGIDMap" => "Identity",
    });
    // NOTE: no /ToUnicode here on purpose.
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "Roboto",
        "Encoding" => "Identity-H",
        "DescendantFonts" => vec![Object::Reference(descendant_id)],
    });
    let mut text_bytes = Vec::new();
    text_bytes.extend_from_slice(&gid_h.to_be_bytes());
    text_bytes.extend_from_slice(&gid_i.to_be_bytes());
    let mut content = b"BT /FX 12 Tf 50 700 Td <".to_vec();
    for b in &text_bytes {
        content.extend_from_slice(format!("{b:02X}").as_bytes());
    }
    content.extend_from_slice(b"> Tj ET");
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! { "FX" => Object::Reference(font_id) }),
        }),
        "Contents" => Object::Reference(content_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            "Kids" => vec![Object::Reference(page_id)],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();

    let (parsed, _warnings) = parse_with_warnings(&bytes);
    assert_eq!(all_text(&parsed), "Hi", "synthesized ToUnicode should decode gids");

    // Round trip: the re-saved document must keep the text extractable.
    let parsed2 = parse(&save(&parsed));
    assert_eq!(all_text(&parsed2), "Hi");
}
