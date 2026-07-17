//! Regression tests for lenient parsing of structurally unusual (but real-world)
//! PDFs — the issue #216 class: objects that may be either dictionaries or
//! streams, inheritable page attributes, optional keys, malformed page trees.
//!
//! All documents are hand-crafted with lopdf so each case is precise.

use lopdf::{dictionary, Object, Stream};
use printpdf::*;

/// Build a minimal single-page PDF. `customize` gets (doc, page_dict, pages_id)
/// after the standard skeleton exists and may mutate the page dictionary.
fn build_pdf(
    media_box_on_page: bool,
    contents: Option<Vec<u8>>,
    customize: impl FnOnce(&mut lopdf::Document, &mut lopdf::Dictionary),
) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.4");
    let pages_id = doc.new_object_id();

    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
    };
    if media_box_on_page {
        page.set(
            "MediaBox",
            vec![0.into(), 0.into(), 595.into(), 842.into()],
        );
    }
    if let Some(c) = contents {
        let content_id = doc.add_object(Stream::new(dictionary! {}, c));
        page.set("Contents", Object::Reference(content_id));
    }
    customize(&mut doc, &mut page);
    let page_id = doc.add_object(page);

    let mut pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => 1,
        "Kids" => vec![Object::Reference(page_id)],
    };
    if !media_box_on_page {
        // Inheritable attribute on the Pages *tree node*, not the page.
        pages_dict.set(
            "MediaBox",
            vec![0.into(), 0.into(), 595.into(), 842.into()],
        );
    }
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn parse_ok(bytes: &[u8]) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings)
        .expect("parse must succeed");
    (doc, warnings)
}

fn parse_errors(warnings: &[PdfWarnMsg]) -> Vec<&PdfWarnMsg> {
    warnings
        .iter()
        .filter(|w| w.severity == PdfParseErrorSeverity::Error)
        .collect()
}

/// MediaBox specified only on the parent Pages node (inheritable per
/// ISO 32000-1, 7.7.3.4). Used to hard-fail with "Page missing MediaBox".
#[test]
fn media_box_inherited_from_pages_node() {
    let bytes = build_pdf(false, Some(b"0 0 100 100 re f".to_vec()), |_, _| {});
    let (doc, _) = parse_ok(&bytes);
    assert_eq!(doc.pages.len(), 1);
    let mb = &doc.pages[0].media_box;
    assert!((mb.width.0 - 595.0).abs() < 0.1, "inherited MediaBox width, got {:?}", mb);
}

/// /Contents is optional: a page without it is simply empty.
/// Used to hard-fail with "Page missing Contents".
#[test]
fn page_without_contents_is_empty_not_error() {
    let bytes = build_pdf(true, None, |_, _| {});
    let (doc, warnings) = parse_ok(&bytes);
    assert_eq!(doc.pages.len(), 1);
    assert!(doc.pages[0].ops.is_empty());
    assert!(parse_errors(&warnings).is_empty(), "{warnings:#?}");
}

/// /Contents as an array of streams; the split can only occur between tokens,
/// and the parser must join the streams with whitespace so the last token of
/// one stream doesn't glue onto the first of the next.
#[test]
fn contents_array_of_streams_joined_correctly() {
    let bytes = build_pdf(true, None, |doc, page| {
        let s1 = doc.add_object(Stream::new(dictionary! {}, b"q 1 0 0 RG".to_vec()));
        // "S" would glue onto "RG" without a separator
        let s2 = doc.add_object(Stream::new(
            dictionary! {},
            b"S Q".to_vec(),
        ));
        page.set(
            "Contents",
            vec![Object::Reference(s1), Object::Reference(s2)],
        );
    });
    let (doc, warnings) = parse_ok(&bytes);
    assert!(parse_errors(&warnings).is_empty(), "{warnings:#?}");
    // q ... Q must both have been seen
    assert!(doc.pages[0].ops.iter().any(|o| matches!(o, Op::SaveGraphicsState)));
    assert!(doc.pages[0].ops.iter().any(|o| matches!(o, Op::RestoreGraphicsState)));
}

/// A dangling /Contents element must warn and skip, not abort the document.
#[test]
fn dangling_content_stream_ref_is_warning_not_error() {
    let bytes = build_pdf(true, None, |doc, page| {
        let good = doc.add_object(Stream::new(dictionary! {}, b"q Q".to_vec()));
        page.set(
            "Contents",
            vec![Object::Reference(good), Object::Reference((9999, 0))],
        );
    });
    let (doc, _warnings) = parse_ok(&bytes);
    assert!(doc.pages[0].ops.iter().any(|o| matches!(o, Op::SaveGraphicsState)));
}

/// Pages-tree kid without the (required, but often missing) /Type key.
#[test]
fn pages_kid_without_type_treated_as_page() {
    let mut doc = lopdf::Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let content_id = doc.add_object(Stream::new(dictionary! {}, b"q Q".to_vec()));
    // NOTE: no /Type on this page dict.
    let page_id = doc.add_object(dictionary! {
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
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

    let (doc, _) = parse_ok(&bytes);
    assert_eq!(doc.pages.len(), 1);
}

/// A cyclic Pages tree (kid array referencing its own parent) must terminate.
#[test]
fn cyclic_pages_tree_terminates() {
    let mut doc = lopdf::Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 1,
            // second kid points back at the Pages node itself
            "Kids" => vec![Object::Reference(page_id), Object::Reference(pages_id)],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();

    let (doc, _) = parse_ok(&bytes);
    assert_eq!(doc.pages.len(), 1);
}

/// Issue #216 verbatim: the /XObject resource value is a reference to a
/// STREAM object. `get_dictionary` fails on streams; the parser must fall back
/// to the stream's dict instead of reporting
/// `Invalid dictionary reference (...): ObjectType { expected: "Dictionary", found: "Stream" }`.
#[test]
fn xobject_stream_reference_is_not_a_type_error() {
    let bytes = build_pdf(true, Some(b"q 100 0 0 100 0 0 cm /Im0 Do Q".to_vec()), |doc, page| {
        // 2x2 raw RGB bitmap, uncompressed
        let img = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 2,
                "Height" => 2,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        );
        let img_id = doc.add_object(img);
        page.set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
            },
        );
    });
    let (doc, warnings) = parse_ok(&bytes);
    for w in &warnings {
        assert!(
            !w.msg.contains("expected") || !w.msg.contains("Stream"),
            "216-style type error still emitted: {}",
            w.msg
        );
        assert!(
            !w.msg.contains("Invalid dictionary reference"),
            "216-style type error still emitted: {}",
            w.msg
        );
    }
    assert_eq!(doc.resources.xobjects.map.len(), 1, "image must be parsed");
    match doc.resources.xobjects.map.values().next().unwrap() {
        XObject::Image(img) => {
            assert!(cfg!(feature = "images"));
            assert_eq!((img.width, img.height), (2, 2));
        }
        // Without the images feature, raw bitmaps are preserved verbatim
        // (XObject::Image cannot be serialized without it).
        XObject::External(ext) => {
            assert!(!cfg!(feature = "images"));
            assert_eq!(
                ext.stream.content,
                vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]
            );
        }
        other => panic!("expected raw bitmap image, got {other:?}"),
    }
}

/// printpdf's own default image serialization: FlateDecode over raw pixels.
/// There are no container magic bytes, so the old byte-sniffing decoder could
/// never read them back; the parser must build the image from the stream dict
/// (with the `images` feature) or preserve the stream verbatim (without it).
/// Either way, the image must survive a save/parse round trip.
#[test]
fn flate_raw_bitmap_image_roundtrips() {
    let pixels: Vec<u8> = vec![
        10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180,
    ];
    let px = pixels.clone();
    let bytes = build_pdf(true, Some(b"q 30 0 0 20 0 0 cm /Im0 Do Q".to_vec()), move |doc, page| {
        let mut img = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 3,
                "Height" => 2,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            px,
        );
        // lopdf applies FlateDecode + sets /Filter
        let _ = img.compress();
        let img_id = doc.add_object(img);
        page.set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
            },
        );
    });

    let (parsed, warnings) = parse_ok(&bytes);
    assert_eq!(
        parsed.resources.xobjects.map.len(),
        1,
        "raw bitmap must be parsed; warnings: {:#?}",
        warnings
            .iter()
            .filter(|w| w.severity != PdfParseErrorSeverity::Info)
            .collect::<Vec<_>>()
    );
    match parsed.resources.xobjects.map.values().next().unwrap() {
        XObject::Image(parsed_img) => {
            assert!(
                cfg!(feature = "images"),
                "XObject::Image must only be produced with the images feature \
                 (serializing it panics otherwise)"
            );
            assert_eq!((parsed_img.width, parsed_img.height), (3, 2));
            assert_eq!(parsed_img.data_format, RawImageFormat::RGB8);
            if let RawImageData::U8(px) = &parsed_img.pixels {
                assert_eq!(px.as_slice(), pixels.as_slice());
            } else {
                panic!("expected U8 pixels");
            }
        }
        XObject::External(ext) => {
            assert!(
                !cfg!(feature = "images"),
                "with the images feature the raw bitmap should decode to XObject::Image"
            );
            // Preserved verbatim: decompressing the kept stream yields the pixels.
            assert_eq!(ext.stream.decompressed_content(), pixels);
        }
        other => panic!("expected image, got {other:?}"),
    }

    // And the image must still be there after a save/parse round trip.
    let bytes2 = parsed.save(&PdfSaveOptions::default(), &mut Vec::new());
    let (parsed2, _) = parse_ok(&bytes2);
    assert_eq!(parsed2.resources.xobjects.map.len(), 1, "image lost on round trip");
}

/// Form XObjects used to be dropped with "unknown xobject subtype: Form",
/// deleting all embedded vector content on re-save. They must be preserved
/// verbatim across the round trip.
#[test]
fn form_xobject_is_preserved_across_roundtrip() {
    let bytes = build_pdf(true, Some(b"q /Fm0 Do Q".to_vec()), |doc, page| {
        let form = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            },
            b"0 0 1 rg 10 10 50 50 re f".to_vec(),
        );
        let form_id = doc.add_object(form);
        page.set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "Fm0" => Object::Reference(form_id) },
            },
        );
    });

    let (doc, warnings) = parse_ok(&bytes);
    for w in &warnings {
        assert!(
            !w.msg.contains("unknown xobject subtype"),
            "form must not be rejected: {}",
            w.msg
        );
    }
    assert_eq!(doc.resources.xobjects.map.len(), 1);
    let preserved = match doc.resources.xobjects.map.values().next().unwrap() {
        XObject::External(ext) => ext,
        other => panic!("expected preserved external xobject, got {other:?}"),
    };
    assert_eq!(preserved.stream.content, b"0 0 1 rg 10 10 50 50 re f".to_vec());

    // Round trip: re-save and re-parse; the form must still exist and keep
    // its content stream.
    let bytes2 = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let (doc2, _) = parse_ok(&bytes2);
    assert_eq!(doc2.resources.xobjects.map.len(), 1, "form lost on round trip");
    match doc2.resources.xobjects.map.values().next().unwrap() {
        XObject::External(ext) => {
            let content = ext.stream.decompressed_content();
            assert_eq!(content, b"0 0 1 rg 10 10 50 50 re f".to_vec());
        }
        other => panic!("expected preserved form after roundtrip, got {other:?}"),
    }
}

/// A Form XObject whose dict references other objects (its /Resources) must
/// have those references *inlined* when preserved — a verbatim object id
/// would dangle in the re-saved document.
#[test]
fn form_xobject_resource_references_are_inlined() {
    let bytes = build_pdf(true, Some(b"q /Fm0 Do Q".to_vec()), |doc, page| {
        let gs_id = doc.add_object(dictionary! { "Type" => "ExtGState", "CA" => 0.5 });
        let form_res_id = doc.add_object(dictionary! {
            "ExtGState" => dictionary! { "G0" => Object::Reference(gs_id) },
        });
        let form = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                "Resources" => Object::Reference(form_res_id),
            },
            b"/G0 gs 0 0 50 50 re f".to_vec(),
        );
        let form_id = doc.add_object(form);
        page.set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "Fm0" => Object::Reference(form_id) },
            },
        );
    });

    let (doc, _) = parse_ok(&bytes);
    let ext = match doc.resources.xobjects.map.values().next().unwrap() {
        XObject::External(ext) => ext,
        other => panic!("expected external, got {other:?}"),
    };
    // Resources must be an inlined dict (not a Ref).
    match ext.stream.dict.get("Resources") {
        Some(printpdf::DictItem::Dict { map }) => {
            match map.get("ExtGState") {
                Some(printpdf::DictItem::Dict { map: gs_map }) => match gs_map.get("G0") {
                    Some(printpdf::DictItem::Dict { .. }) => {}
                    other => panic!("nested ref not inlined: {other:?}"),
                },
                other => panic!("ExtGState not inlined: {other:?}"),
            }
        }
        other => panic!("form /Resources not inlined: {other:?}"),
    }
}

/// An image with an unsupported filter must be preserved (and still usable on
/// re-save) instead of being silently dropped — the "even a simple parse +
/// save ruins this PDF" part of #216.
#[test]
fn undecodable_image_is_preserved_not_dropped() {
    let bytes = build_pdf(true, Some(b"q /Im0 Do Q".to_vec()), |doc, page| {
        let img = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 4,
                "Height" => 4,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "JPXDecode",
            },
            vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x01, 0x02, 0x03],
        );
        let img_id = doc.add_object(img);
        page.set(
            "Resources",
            dictionary! {
                "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
            },
        );
    });

    let (doc, _) = parse_ok(&bytes);
    assert_eq!(
        doc.resources.xobjects.map.len(),
        1,
        "undecodable image must be preserved as an external stream"
    );
    match doc.resources.xobjects.map.values().next().unwrap() {
        XObject::External(ext) => {
            assert_eq!(
                ext.stream.content,
                vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x01, 0x02, 0x03],
                "original (still-encoded) bytes must be kept"
            );
        }
        XObject::Image(_) => { /* decoder somehow understood it - also fine */ }
        other => panic!("unexpected xobject: {other:?}"),
    }

    // Re-save + re-parse: still present.
    let bytes2 = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let (doc2, _) = parse_ok(&bytes2);
    assert_eq!(doc2.resources.xobjects.map.len(), 1, "image lost on round trip");
}

/// Layers (optional content groups) must keep the *resource name* linkage
/// between `BDC /OC /Name` ops and the layer map, otherwise the re-saved file
/// references properties that no longer exist.
#[test]
fn layer_property_names_survive_roundtrip() {
    let mut doc = PdfDocument::new("layers");
    let layer_id = doc.add_layer(&Layer::new("My Layer"));
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            Op::BeginLayer {
                layer_id: layer_id.clone(),
            },
            Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    icc_profile: None,
                }),
            },
            Op::EndLayer,
        ],
    ));
    let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    let (parsed, _) = parse_ok(&bytes);

    // Every BeginLayer op (BDC /OC) must reference a key that exists in the
    // parsed layer map — the serializer builds the page's /Properties entries
    // from exactly this pairing.
    let mut found_op = false;
    for op in &parsed.pages[0].ops {
        if let Op::BeginLayer { layer_id } = op {
            found_op = true;
            assert!(
                parsed.resources.layers.map.contains_key(layer_id),
                "BDC layer id {:?} not in parsed layer map (keys: {:?})",
                layer_id,
                parsed.resources.layers.map.keys().collect::<Vec<_>>()
            );
        }
    }
    assert!(found_op, "BeginLayer op missing after parse");

    // Second round trip keeps the linkage too.
    let bytes2 = parsed.save(&PdfSaveOptions::default(), &mut Vec::new());
    let (parsed2, _) = parse_ok(&bytes2);
    let mut found_op2 = false;
    for op in &parsed2.pages[0].ops {
        if let Op::BeginLayer { layer_id } = op {
            found_op2 = true;
            assert!(
                parsed2.resources.layers.map.contains_key(layer_id),
                "roundtrip 2: BDC layer id {:?} not in layer map (keys: {:?})",
                layer_id,
                parsed2.resources.layers.map.keys().collect::<Vec<_>>()
            );
        }
    }
    assert!(found_op2, "BeginLayer op missing after second parse");
}

/// A simple font that is referenced but not embedded (no FontFile) must fall
/// back to a builtin substitute so its text still decodes and re-encodes.
#[test]
fn unembedded_simple_font_substitutes_builtin() {
    let bytes = build_pdf(
        true,
        Some(b"BT /FA 12 Tf 50 700 Td (Arial text here) Tj ET".to_vec()),
        |doc, page| {
            let font_id = doc.add_object(dictionary! {
                "Type" => "Font",
                "Subtype" => "TrueType",
                "BaseFont" => "Arial-BoldMT",
                "Encoding" => "WinAnsiEncoding",
            });
            page.set(
                "Resources",
                dictionary! {
                    "Font" => dictionary! { "FA" => Object::Reference(font_id) },
                },
            );
        },
    );
    let (doc, _) = parse_ok(&bytes);
    let text: String = doc
        .extract_text()
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        text.contains("Arial text here"),
        "text of unembedded font must decode via substitution, got {text:?}"
    );
    // The substitute must be a bold Helvetica-family builtin.
    let uses_bold_builtin = doc.pages[0].ops.iter().any(|op| {
        matches!(
            op,
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
                ..
            }
        )
    });
    assert!(uses_bold_builtin, "expected HelveticaBold substitution");
}
