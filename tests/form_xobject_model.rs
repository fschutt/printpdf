//! Form XObjects in the document model: reading a PDF produces `XObject::Form` with the ops of
//! the form's content stream and a scope of its own, a form that cannot be modelled stays as it is,
//! and saving writes the form again.
//!
//! The documents are built with lopdf, so each case is exact.
#![cfg(feature = "html")]

use std::{sync::mpsc, thread, time::Duration};

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

fn numbers(values: &[i64]) -> Object {
    Object::Array(values.iter().map(|v| Object::Integer(*v)).collect())
}

fn reference(id: ObjectId) -> Object {
    Object::Reference(id)
}

/// A one-page document: `content` is the page's content stream, `resources` makes its
/// `/Resources`.
fn build(content: &[u8], resources: impl FnOnce(&mut Document) -> Dictionary) -> Vec<u8> {
    let mut doc = Document::with_version("1.4");
    let pages_id = doc.new_object_id();
    let resources = resources(&mut doc);
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => reference(pages_id),
        "MediaBox" => numbers(&[0, 0, 600, 800]),
        "Resources" => resources,
        "Contents" => reference(content_id),
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

/// A standard font (`/Type1`, `/BaseFont` one of the 14).
fn standard_font(doc: &mut Document, base_font: &str) -> ObjectId {
    doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => base_font,
        "Encoding" => "WinAnsiEncoding",
    })
}

/// A form XObject with `dict` on top of its type, subtype and a 600 x 800 `/BBox`.
fn form(doc: &mut Document, mut dict: Dictionary, content: &[u8]) -> ObjectId {
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    if !dict.has(b"BBox") {
        dict.set("BBox", numbers(&[0, 0, 600, 800]));
    }
    doc.add_object(Stream::new(dict, content.to_vec()))
}

fn parse(bytes: &[u8]) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings).expect("parse");
    (doc, warnings)
}

fn save(doc: &PdfDocument) -> Vec<u8> {
    doc.save(&PdfSaveOptions::default(), &mut Vec::new())
}

fn form_of<'a>(doc: &'a PdfDocument, name: &str) -> &'a FormXObject {
    match doc.resources.xobjects.map.get(&XObjectId(name.to_string())) {
        Some(XObject::Form(form)) => form,
        other => panic!("{name} is not a form: {other:?}"),
    }
}

/// The plain text of the `ShowText` ops, one string per op.
fn texts(ops: &[Op]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match op {
            Op::ShowText { items } => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        TextItem::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

/// The font handles of the `SetFont` ops.
fn fonts(ops: &[Op]) -> Vec<PdfFontHandle> {
    ops.iter()
        .filter_map(|op| match op {
            Op::SetFont { font, .. } => Some(font.clone()),
            _ => None,
        })
        .collect()
}

/// Runs `f` on a thread and fails when it takes longer than a minute (debug builds are slow).
fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(60))
        .unwrap_or_else(|_| panic!("{what}: no result within a minute"))
}

const HELLO: &[u8] = b"BT /F1 12 Tf 50 700 Td (Hello form) Tj ET";

/// A page that draws `/Fm1`, a form with its own Helvetica as `/F1`.
fn hello_pdf() -> Vec<u8> {
    build(b"q /Fm1 Do Q", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm1 = form(
            doc,
            dictionary! {
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(font) } },
            },
            HELLO,
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    })
}

#[test]
fn a_form_is_read_with_its_content_and_its_own_resources() {
    let (doc, _) = parse(&hello_pdf());
    assert_eq!(doc.resources.xobjects.map.len(), 1);
    let form = form_of(&doc, "Fm1");
    assert_eq!(texts(&form.ops), vec!["Hello form".to_string()]);
    assert_eq!(
        fonts(&form.ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::Helvetica)]
    );
    assert_eq!(form.bbox, Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(600.0), Pt(800.0)));
    assert!(form.resources.is_some(), "the form has resources of its own");

    // The page only draws the form: the form's text and font are not part of its ops.
    let page_ops = &doc.pages[0].ops;
    assert!(texts(page_ops).is_empty());
    assert!(
        page_ops
            .iter()
            .any(|op| matches!(op, Op::UseXobject { id, .. } if id.0 == "Fm1")),
        "{page_ops:?}"
    );
}

/// The page's `/F1` is Courier and the form's `/F1` is Times: each is read in its own scope.
#[test]
fn a_name_in_a_form_does_not_clash_with_the_same_name_on_the_page() {
    let bytes = build(
        b"BT /F1 12 Tf 50 750 Td (page text) Tj ET /Fm1 Do",
        |doc| {
            let courier = standard_font(doc, "Courier");
            let times = standard_font(doc, "Times-Roman");
            let fm1 = form(
                doc,
                dictionary! {
                    "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(times) } },
                },
                b"BT /F1 12 Tf 50 700 Td (form text) Tj ET",
            );
            dictionary! {
                "Font" => dictionary! { "F1" => reference(courier) },
                "XObject" => dictionary! { "Fm1" => reference(fm1) },
            }
        },
    );
    let (doc, _) = parse(&bytes);
    assert_eq!(
        fonts(&doc.pages[0].ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::Courier)]
    );
    assert_eq!(
        fonts(&form_of(&doc, "Fm1").ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::TimesRoman)]
    );
    assert_eq!(texts(&doc.pages[0].ops), vec!["page text".to_string()]);
    assert_eq!(texts(&form_of(&doc, "Fm1").ops), vec!["form text".to_string()]);

    // Saved, each keeps its own font: the page's resources have Courier, the form's Times.
    let again = parse(&save(&doc)).0;
    assert_eq!(
        fonts(&again.pages[0].ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::Courier)]
    );
    assert_eq!(
        fonts(&form_of(&again, "Fm1").ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::TimesRoman)]
    );
}

/// A form without `/Resources` uses the resources of the page that draws it.
#[test]
fn a_form_without_resources_uses_the_ones_of_the_page() {
    let bytes = build(b"/Fm1 Do", |doc| {
        let font = standard_font(doc, "Courier");
        let fm1 = form(doc, dictionary! {}, HELLO);
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => dictionary! { "Fm1" => reference(fm1) },
        }
    });
    let (doc, _) = parse(&bytes);
    let form = form_of(&doc, "Fm1");
    assert!(form.resources.is_none());
    assert_eq!(
        fonts(&form.ops),
        vec![PdfFontHandle::Builtin(BuiltinFont::Courier)],
        "F1 is the page's font"
    );
    // And it is written without resources again.
    let again = parse(&save(&doc)).0;
    assert!(form_of(&again, "Fm1").resources.is_none());
}

/// Three forms, each in the resources of the one that draws it.
#[test]
fn nested_forms_have_nested_scopes() {
    let bytes = build(b"/Fa Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fonts = dictionary! { "F1" => reference(font) };
        let fc = form(
            doc,
            dictionary! { "Resources" => dictionary! { "Font" => fonts.clone() } },
            b"BT /F1 9 Tf 10 10 Td (third) Tj ET",
        );
        let fb = form(
            doc,
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => fonts.clone(),
                    "XObject" => dictionary! { "Fc" => reference(fc) },
                },
            },
            b"BT /F1 9 Tf 10 20 Td (second) Tj ET /Fc Do",
        );
        let fa = form(
            doc,
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => fonts,
                    "XObject" => dictionary! { "Fb" => reference(fb) },
                },
            },
            b"BT /F1 9 Tf 10 30 Td (first) Tj ET /Fb Do",
        );
        dictionary! { "XObject" => dictionary! { "Fa" => reference(fa) } }
    });
    let (doc, _) = parse(&bytes);
    let fa = form_of(&doc, "Fa");
    assert_eq!(texts(&fa.ops), vec!["first".to_string()]);
    let Some(XObject::Form(fb)) = fa.resources.as_ref().unwrap().xobjects.map.get(&XObjectId("Fb".into()))
    else {
        panic!("Fb is not a form in the scope of Fa");
    };
    assert_eq!(texts(&fb.ops), vec!["second".to_string()]);
    let Some(XObject::Form(fc)) = fb.resources.as_ref().unwrap().xobjects.map.get(&XObjectId("Fc".into()))
    else {
        panic!("Fc is not a form in the scope of Fb");
    };
    assert_eq!(texts(&fc.ops), vec!["third".to_string()]);

    // The nested forms are written inside the scopes they are in.
    let again = parse(&save(&doc)).0;
    assert_eq!(again.resources.xobjects.map.len(), 1);
    let fa = form_of(&again, "Fa");
    assert_eq!(texts(&fa.ops), vec!["first".to_string()]);
    let Some(XObject::Form(fb)) = fa.resources.as_ref().unwrap().xobjects.map.get(&XObjectId("Fb".into()))
    else {
        panic!("Fb is not a form in the scope of Fa after a save");
    };
    // `Do` is written between `q cm` and `Q`, so the ops are not the same ones.
    assert_eq!(texts(&fb.ops), vec!["second".to_string()]);
    let Some(XObject::Form(fc)) = fb.resources.as_ref().unwrap().xobjects.map.get(&XObjectId("Fc".into()))
    else {
        panic!("Fc is not a form in the scope of Fb after a save");
    };
    assert_eq!(texts(&fc.ops), vec!["third".to_string()]);
}

/// The same form object under two names is read once and is the same form twice.
#[test]
fn a_form_under_two_names_is_the_same_form() {
    let bytes = build(b"/Fm1 Do /Fm2 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm = form(
            doc,
            dictionary! {
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(font) } },
            },
            HELLO,
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm), "Fm2" => reference(fm) } }
    });
    let (doc, _) = parse(&bytes);
    assert_eq!(form_of(&doc, "Fm1"), form_of(&doc, "Fm2"));
}

fn assert_external(bytes: &[u8], content: &[u8], what: &str) -> Vec<PdfWarnMsg> {
    let (doc, warnings) = parse(bytes);
    match doc.resources.xobjects.map.get(&XObjectId("Fm1".into())) {
        Some(XObject::External(ext)) => {
            assert_eq!(ext.stream.content, content, "{what}: the stream is kept as it was");
        }
        other => panic!("{what}: expected the form to stay external, got {other:?}"),
    }
    assert!(
        warnings.iter().any(|w| w.msg.contains("kept as it is")),
        "{what}: no message: {warnings:?}"
    );
    // And it is saved again.
    let again = parse(&save(&doc)).0;
    assert_eq!(again.resources.xobjects.map.len(), 1, "{what}: lost on save");
    warnings
}

#[test]
fn a_form_with_something_unmodelled_stays_external() {
    // An entry that is not in ISO 32000-1.
    let content = b"0 0 10 10 re f";
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = form(doc, dictionary! { "Mystery" => 1 }, content);
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, content, "unknown dictionary entry");

    // A resource category that is not in ISO 32000-1.
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = form(
            doc,
            dictionary! { "Resources" => dictionary! { "Mystery" => dictionary! {} } },
            content,
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, content, "unknown resource category");

    // An operator that has no Op.
    let unknown = b"0 0 10 10 re f 1 2 3 mystery";
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = form(doc, dictionary! {}, unknown);
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, unknown, "unknown operator");

    // A pattern as the color.
    let pattern = b"/Pattern cs /P1 scn 0 0 10 10 re f";
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = form(doc, dictionary! {}, pattern);
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, pattern, "pattern color");

    // A font without a program: a Type 3 font.
    let text = b"BT /T3 12 Tf 10 10 Td (x) Tj ET";
    let bytes = build(b"/Fm1 Do", |doc| {
        let t3 = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type3" });
        let fm1 = form(
            doc,
            dictionary! { "Resources" => dictionary! { "Font" => dictionary! { "T3" => reference(t3) } } },
            text,
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, text, "font without a program");

    // No /BBox.
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = doc.add_object(Stream::new(
            dictionary! { "Type" => "XObject", "Subtype" => "Form" },
            content.to_vec(),
        ));
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    assert_external(&bytes, content, "no bounding box");
}

#[test]
fn colour_spaces_groups_and_graphics_states_survive_a_round_trip() {
    let bytes = build(b"/Fm1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let cs = vec![
            Object::from("CalRGB"),
            dictionary! { "WhitePoint" => vec![Object::Real(0.95), Object::Real(1.0), Object::Real(1.09)] }.into(),
        ];
        let fm1 = form(
            doc,
            dictionary! {
                "Group" => dictionary! { "Type" => "Group", "S" => "Transparency", "I" => true, "K" => false },
                "Matrix" => numbers(&[1, 0, 0, 1, 5, 7]),
                "Resources" => dictionary! {
                    "Font" => dictionary! { "F1" => reference(font) },
                    "ColorSpace" => dictionary! { "Cs1" => cs },
                    "ProcSet" => vec![Object::from("PDF"), Object::from("Text")],
                    "ExtGState" => dictionary! {
                        // `SMask` has no field in the model: it stays as it is. `ca` is typed.
                        "Gs1" => dictionary! { "Type" => "ExtGState", "ca" => Object::Real(0.5), "SMask" => "None" },
                        "Gs2" => dictionary! { "Type" => "ExtGState", "LW" => 3 },
                    },
                },
            },
            b"/Gs1 gs /Cs1 cs 1 0 0 sc BT /F1 12 Tf 10 10 Td (kept) Tj ET",
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    let (doc, _) = parse(&bytes);
    let check = |doc: &PdfDocument| {
        let form = form_of(doc, "Fm1");
        assert_eq!(form.matrix, Some(CurTransMat::Raw([1.0, 0.0, 0.0, 1.0, 5.0, 7.0])));
        assert_eq!(texts(&form.ops), vec!["kept".to_string()]);
        // The group has attributes: it is kept as it was.
        assert_eq!(form.group, None);
        let Some(DictItem::Dict { map: group }) = form.extra.get("Group") else {
            panic!("no /Group in {:?}", form.extra);
        };
        assert_eq!(group.get("I"), Some(&DictItem::Bool(true)));
        // The categories without a typed map, and the graphics state with an /SMask.
        let Some(DictItem::Dict { map: raw }) = form.extra.get("Resources") else {
            panic!("no raw resources in {:?}", form.extra);
        };
        assert!(raw.contains_key("ColorSpace"), "{raw:?}");
        assert!(raw.contains_key("ProcSet"), "{raw:?}");
        let Some(DictItem::Dict { map: states }) = raw.get("ExtGState") else {
            panic!("no raw graphics states in {raw:?}");
        };
        assert!(states.contains_key("Gs1") && !states.contains_key("Gs2"), "{states:?}");
        // The typed map has both.
        let typed = &form.resources.as_ref().unwrap().extgstates.map;
        assert_eq!(typed.len(), 2);
    };
    check(&doc);
    check(&parse(&save(&doc)).0);
}

#[test]
fn a_parsed_form_survives_a_save_unchanged() {
    let (doc, _) = parse(&hello_pdf());
    let saved = save(&doc);
    let again = parse(&saved).0;
    assert_eq!(form_of(&again, "Fm1").ops, form_of(&doc, "Fm1").ops);
    assert_eq!(form_of(&again, "Fm1").bbox, form_of(&doc, "Fm1").bbox);

    // The saved file has a form with a /BBox and its own /Resources.
    let lopdf_doc = Document::load_mem(&saved).unwrap();
    let forms: Vec<&Stream> = lopdf_doc
        .objects
        .values()
        .filter_map(|o| match o {
            Object::Stream(s) if s.dict.get(b"Subtype").and_then(|n| n.as_name()).ok() == Some(b"Form") => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(forms.len(), 1);
    assert!(forms[0].dict.has(b"BBox"));
    assert!(forms[0].dict.has(b"Resources"));
}

#[test]
fn a_document_with_a_form_is_json() {
    let (doc, _) = parse(&hello_pdf());
    let json = serde_json::to_string(&doc).expect("to json");
    let back: PdfDocument = serde_json::from_str(&json).expect("from json");
    assert_eq!(back, doc);
}

/// A form that has itself in its resources.
#[test]
fn a_form_in_its_own_resources_is_cut() {
    let bytes = bounded("self-referencing form", || {
        build(b"/Fm1 Do", |doc| {
            let font = standard_font(doc, "Helvetica");
            let fm1 = doc.new_object_id();
            let mut dict = dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0, 0, 600, 800]),
                "Resources" => dictionary! {
                    "Font" => dictionary! { "F1" => reference(font) },
                    "XObject" => dictionary! { "Fm1" => reference(fm1) },
                },
            };
            dict.set("Type", "XObject");
            doc.objects.insert(
                fm1,
                Object::Stream(Stream::new(dict, b"BT /F1 12 Tf 10 10 Td (again) Tj ET /Fm1 Do".to_vec())),
            );
            dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
        })
    });
    let (doc, warnings) = bounded("parse", move || parse(&bytes));
    let form = form_of(&doc, "Fm1");
    // Inside the form, `Fm1` is the cut: the form as it was, not another model of itself.
    let inner = form.resources.as_ref().unwrap().xobjects.map.get(&XObjectId("Fm1".into()));
    assert!(matches!(inner, Some(XObject::External(_))), "{inner:?}");
    assert!(warnings.iter().any(|w| w.msg.contains("itself")), "{warnings:?}");
    let saved = bounded("save", move || save(&doc));
    assert!(!saved.is_empty());
}

/// Ten forms in a chain, each in the resources of the one before and drawing the next ten times:
/// every form is read once, and the model is a chain, however many draws it describes.
#[test]
fn a_chain_of_forms_that_draw_the_next_ten_times_is_read_in_linear_time() {
    let bytes = build(b"/F1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let mut inner: Option<ObjectId> = None;
        for level in (1..=10).rev() {
            let mut resources = dictionary! { "Font" => dictionary! { "F1" => reference(font) } };
            let mut content = format!("BT /F1 8 Tf 10 {} Td (level {level}) Tj ET ", 10 * level);
            if let Some(next) = inner {
                resources.set(
                    "XObject",
                    dictionary! { format!("F{}", level + 1) => reference(next) },
                );
                for _ in 0..10 {
                    content.push_str(&format!("/F{} Do ", level + 1));
                }
            }
            inner = Some(form(doc, dictionary! { "Resources" => resources }, content.as_bytes()));
        }
        dictionary! { "XObject" => dictionary! { "F1" => reference(inner.unwrap()) } }
    });
    let (doc, _) = bounded("parse the chain", move || parse(&bytes));
    let mut depth = 0;
    let mut current = form_of(&doc, "F1");
    loop {
        depth += 1;
        let next = current
            .resources
            .as_ref()
            .and_then(|r| r.xobjects.map.get(&XObjectId(format!("F{}", depth + 1))));
        match next {
            Some(XObject::Form(next)) => current = next,
            _ => break,
        }
    }
    assert_eq!(depth, 10);
    let saved = bounded("save the chain", move || save(&doc));
    assert!(!saved.is_empty());
}

/// Forms nested deeper than the limit stay as they are.
#[test]
fn forms_nested_deeper_than_the_limit_stay_external() {
    let bytes = build(b"/F1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let mut inner: Option<ObjectId> = None;
        for level in (1..=40).rev() {
            let mut resources = dictionary! { "Font" => dictionary! { "F1" => reference(font) } };
            let mut content = format!("BT /F1 8 Tf 10 10 Td (level {level}) Tj ET ");
            if let Some(next) = inner {
                resources.set("XObject", dictionary! { format!("F{}", level + 1) => reference(next) });
                content.push_str(&format!("/F{} Do", level + 1));
            }
            inner = Some(form(doc, dictionary! { "Resources" => resources }, content.as_bytes()));
        }
        dictionary! { "XObject" => dictionary! { "F1" => reference(inner.unwrap()) } }
    });
    let (doc, warnings) = bounded("parse 40 nested forms", move || parse(&bytes));
    let mut depth = 0;
    let mut current = form_of(&doc, "F1");
    while let Some(XObject::Form(next)) = current
        .resources
        .as_ref()
        .and_then(|r| r.xobjects.map.get(&XObjectId(format!("F{}", depth + 2))))
    {
        depth += 1;
        current = next;
    }
    assert!(depth < 40, "the nesting is cut, got {depth} levels");
    assert!(warnings.iter().any(|w| w.msg.contains("nested in more than")), "{warnings:?}");
    let saved = bounded("save", move || save(&doc));
    assert!(!saved.is_empty());
}
