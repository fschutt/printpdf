//! The text and the drawings inside form XObjects are seen by `extract_text`,
//! `extract_text_boxes` and `to_svg`. Pages that macOS (Quartz), Word or print drivers write put
//! the whole page into one form: the page's content stream is `q ... /Fm1 Do Q`, and the text is
//! in the form.
//!
//! The documents are built with lopdf, so each case is exact. The last test reads real files, if
//! the machine has them (and `pdftotext`).
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

/// A one-page document of 600 x 800 pt: `content` is the page's content stream, `resources`
/// makes its `/Resources`.
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

/// A simple TrueType font with the program `ttf` and WinAnsi encoding.
fn truetype_font(doc: &mut Document, name: &str, ttf: &[u8]) -> ObjectId {
    let mut program = Stream::new(dictionary! { "Length1" => ttf.len() as i64 }, ttf.to_vec());
    let _ = program.compress();
    let program = doc.add_object(program);
    let descriptor = doc.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => name, "Flags" => 32,
        "FontBBox" => numbers(&[-200, -300, 1200, 1000]), "ItalicAngle" => 0,
        "Ascent" => 800, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
        "FontFile2" => reference(program),
    });
    doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => name,
        "FirstChar" => 32, "LastChar" => 126,
        "Widths" => vec![Object::Integer(600); 95],
        "Encoding" => "WinAnsiEncoding",
        "FontDescriptor" => reference(descriptor),
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

/// The text of the first page, a string for each text section.
fn text(doc: &PdfDocument) -> Vec<String> {
    doc.extract_text().into_iter().next().unwrap_or_default()
}

fn svg(doc: &PdfDocument) -> (String, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let svg = doc
        .page_to_svg(1, &PdfToSvgOptions::default(), &mut warnings)
        .expect("page 1");
    (svg, warnings)
}

/// The words of the first page with their boxes.
fn words(doc: &PdfDocument) -> Vec<printpdf::text_boxes::TextWord> {
    doc.extract_text_boxes()
        .into_iter()
        .next()
        .map(|page| page.lines.into_iter().flat_map(|line| line.words).collect())
        .unwrap_or_default()
}

/// The six numbers of the `transform="matrix(...)"` of the first `<text>` of an SVG.
fn first_text_matrix(svg: &str) -> [f32; 6] {
    let text = &svg[svg.find("<text ").expect("a <text>")..];
    let start = text.find("transform=\"matrix(").expect("a matrix") + "transform=\"matrix(".len();
    let numbers: Vec<f32> = text[start..text[start..].find(')').expect(")") + start]
        .split_whitespace()
        .map(|n| n.parse().expect("a number"))
        .collect();
    numbers.try_into().expect("six numbers")
}

fn assert_close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.01, "{what}: {a} and {b} differ");
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
fn the_text_of_a_form_is_the_text_of_the_page() {
    let (doc, _) = parse(&hello_pdf());
    assert_eq!(text(&doc), vec!["Hello form".to_string()]);

    // The boxes and the SVG see it too.
    let words = words(&doc);
    assert_eq!(
        words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
        vec!["Hello", "form"]
    );
    assert_close(words[0].bbox[0], 50.0, "left edge of the first word");
    let (svg, _) = svg(&doc);
    assert!(svg.contains("Hello form"), "{svg}");
}

#[test]
fn the_text_survives_a_save() {
    let (doc, _) = parse(&hello_pdf());
    let saved = save(&doc);
    let (again, _) = parse(&saved);
    assert_eq!(text(&again), text(&doc));
    assert_eq!(words(&again).len(), 2);
}

/// The matrix of a form and the `cm` before its `Do` move its text like a `cm` in the page would.
#[test]
fn matrix_and_placement_position_the_text_like_a_cm() {
    let direct = build(
        b"q 2 0 0 2 100 50 cm BT /F1 10 Tf 20 30 Td (Moved) Tj ET Q",
        |doc| {
            let font = standard_font(doc, "Helvetica");
            dictionary! { "Font" => dictionary! { "F1" => reference(font) } }
        },
    );
    let in_form = build(b"/Fm1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm1 = form(
            doc,
            dictionary! {
                "BBox" => numbers(&[0, 0, 300, 400]),
                "Matrix" => numbers(&[2, 0, 0, 2, 100, 50]),
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(font) } },
            },
            b"BT /F1 10 Tf 20 30 Td (Moved) Tj ET",
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    // The placement of the `Do` goes between the matrix of the form and the page.
    let placed = build(b"q 1 0 0 1 7 11 cm /Fm1 Do Q", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm1 = form(
            doc,
            dictionary! {
                "BBox" => numbers(&[0, 0, 300, 400]),
                "Matrix" => numbers(&[2, 0, 0, 2, 100, 50]),
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(font) } },
            },
            b"BT /F1 10 Tf 20 30 Td (Moved) Tj ET",
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });

    let direct = parse(&direct).0;
    let in_form = parse(&in_form).0;
    let placed = parse(&placed).0;

    let (a, b) = (words(&direct), words(&in_form));
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].text, "Moved");
    // x is 2 * 20 + 100, the baseline 2 * 30 + 50 above the bottom of an 800 pt page.
    assert_close(b[0].bbox[0], 140.0, "left edge");
    for i in 0..4 {
        assert_close(a[0].bbox[i], b[0].bbox[i], "box of the word in the form and with a cm");
    }
    assert_close(a[0].font_size, b[0].font_size, "font size");

    // `q 1 0 0 1 7 11 cm` moves everything by 7 and 11.
    let c = words(&placed);
    assert_close(c[0].bbox[0], b[0].bbox[0] + 7.0, "x with a placement");
    assert_close(c[0].bbox[1], b[0].bbox[1] - 11.0, "y with a placement");

    // The SVG puts the text where the `cm` puts it.
    let (svg_direct, _) = svg(&direct);
    let (svg_form, _) = svg(&in_form);
    let (m, n) = (first_text_matrix(&svg_direct), first_text_matrix(&svg_form));
    for i in 0..6 {
        assert_close(m[i], n[i], "transform of the <text>");
    }
}

/// `cm` operators multiply: `1 0 0 1 10 10 cm 2 0 0 2 0 0 cm` is `2 0 0 2 10 10 cm`.
#[test]
fn nested_cm_operators_in_the_svg_multiply() {
    let text = b"BT /F1 10 Tf 5 5 Td (x) Tj ET";
    let page = |content: &[u8]| {
        parse(&build(content, |doc| {
            let font = standard_font(doc, "Helvetica");
            dictionary! { "Font" => dictionary! { "F1" => reference(font) } }
        }))
        .0
    };
    let mut one = b"q 2 0 0 2 10 10 cm ".to_vec();
    one.extend_from_slice(text);
    one.extend_from_slice(b" Q");
    let mut two = b"q 1 0 0 1 10 10 cm 2 0 0 2 0 0 cm ".to_vec();
    two.extend_from_slice(text);
    two.extend_from_slice(b" Q");
    // `2 0 0 2 0 0` applies first, then `1 0 0 1 10 10`: (x, y) -> (2x + 10, 2y + 10).
    let (a, b) = (svg(&page(&one)).0, svg(&page(&two)).0);
    let (m, n) = (first_text_matrix(&a), first_text_matrix(&b));
    for i in 0..6 {
        assert_close(m[i], n[i], "transform of the <text>");
    }
}

/// The page's `/F1` is Courier and the form's `/F1` is Times: each text is set in its font.
#[test]
fn a_name_in_a_form_does_not_clash_with_the_same_name_on_the_page() {
    let bytes = build(b"BT /F1 12 Tf 50 750 Td (page text) Tj ET /Fm1 Do", |doc| {
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
    });
    let check = |doc: &PdfDocument| {
        assert_eq!(text(doc), vec!["page text".to_string(), "form text".to_string()]);
        let words = words(doc);
        let font_of = |text: &str| words.iter().find(|w| w.text == text).and_then(|w| w.builtin);
        assert_eq!(font_of("page"), Some(BuiltinFont::Courier));
        assert_eq!(font_of("form"), Some(BuiltinFont::TimesRoman));
    };
    let (doc, _) = parse(&bytes);
    check(&doc);
    // Both fonts are in the saved file.
    let saved = save(&doc);
    let lopdf_doc = Document::load_mem(&saved).unwrap();
    let base_fonts: Vec<String> = lopdf_doc
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"Font"))
        .filter_map(|d| d.get(b"BaseFont").and_then(|n| n.as_name()).ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect();
    assert!(base_fonts.iter().any(|f| f == "Courier"), "{base_fonts:?}");
    assert!(base_fonts.iter().any(|f| f == "Times-Roman"), "{base_fonts:?}");
    check(&parse(&saved).0);
}

/// The same name for two embedded fonts, one on the page and one in the form: both are written.
#[test]
fn two_embedded_fonts_with_the_same_name_survive_a_save() {
    let courier = include_bytes!("../examples/assets/fonts/Courier.ttf");
    let helvetica = include_bytes!("../examples/assets/fonts/Helvetica.ttf");
    let bytes = build(b"BT /F1 12 Tf 50 750 Td (Page Text) Tj ET /Fm1 Do", |doc| {
        let page_font = truetype_font(doc, "ABCDEF+TestMono", courier);
        let form_font = truetype_font(doc, "GHIJKL+TestSans", helvetica);
        let fm1 = form(
            doc,
            dictionary! {
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(form_font) } },
            },
            b"BT /F1 12 Tf 50 700 Td (Form Text) Tj ET",
        );
        dictionary! {
            "Font" => dictionary! { "F1" => reference(page_font) },
            "XObject" => dictionary! { "Fm1" => reference(fm1) },
        }
    });
    let (doc, _) = parse(&bytes);
    assert_eq!(text(&doc), vec!["Page Text".to_string(), "Form Text".to_string()]);

    let saved = save(&doc);
    let (again, _) = parse(&saved);
    assert_eq!(text(&again), text(&doc));

    // Two font programs are in the file, and the form's has its own.
    let lopdf_doc = Document::load_mem(&saved).unwrap();
    let programs = lopdf_doc
        .objects
        .values()
        .filter(|o| matches!(o, Object::Dictionary(d) if d.has(b"FontFile2")))
        .count();
    assert_eq!(programs, 2, "one font program for each of the two fonts");
    let Some(XObject::Form(form)) = again.resources.xobjects.map.values().next() else {
        panic!("no form after the save");
    };
    let page_fonts = again.resources.fonts.map.len();
    let form_fonts = form.resources.as_ref().map_or(0, |r| r.fonts.map.len());
    assert_eq!((page_fonts, form_fonts), (1, 1));
    assert_ne!(
        again.resources.fonts.map.values().next().unwrap().parsed_font,
        form.resources.as_ref().unwrap().fonts.map.values().next().unwrap().parsed_font,
        "the two fonts are different programs"
    );
}

/// Three forms, each in the resources of the one that draws it.
#[test]
fn nested_forms_three_deep() {
    let bytes = build(b"/Fa Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fonts = dictionary! { "F1" => reference(font) };
        let fc = form(
            doc,
            dictionary! { "Resources" => dictionary! { "Font" => fonts.clone() } },
            b"BT /F1 9 Tf 10 100 Td (third) Tj ET",
        );
        let fb = form(
            doc,
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => fonts.clone(),
                    "XObject" => dictionary! { "Fc" => reference(fc) },
                },
            },
            b"BT /F1 9 Tf 10 200 Td (second) Tj ET /Fc Do",
        );
        let fa = form(
            doc,
            dictionary! {
                "Resources" => dictionary! {
                    "Font" => fonts,
                    "XObject" => dictionary! { "Fb" => reference(fb) },
                },
            },
            b"BT /F1 9 Tf 10 300 Td (first) Tj ET /Fb Do",
        );
        dictionary! { "XObject" => dictionary! { "Fa" => reference(fa) } }
    });
    let (doc, _) = parse(&bytes);
    let expected = vec!["first".to_string(), "second".to_string(), "third".to_string()];
    assert_eq!(text(&doc), expected);
    assert_eq!(words(&doc).len(), 3);
    assert_eq!(text(&parse(&save(&doc)).0), expected);
}

/// A form drawn twice shows its text twice, from one form in the document.
#[test]
fn a_form_drawn_twice() {
    let bytes = build(b"/Fm1 Do 1 0 0 1 0 -20 cm /Fm1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm1 = form(
            doc,
            dictionary! {
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => reference(font) } },
            },
            HELLO,
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    let (doc, _) = parse(&bytes);
    assert_eq!(doc.resources.xobjects.map.len(), 1);
    assert_eq!(text(&doc), vec!["Hello form".to_string(), "Hello form".to_string()]);
    // The second is 20 pt lower.
    let words = words(&doc);
    assert_eq!(words.len(), 4);
    assert_close(words[2].bbox[1] - words[0].bbox[1], 20.0, "distance of the two draws");
}

/// A form without `/Resources` shows its text in the fonts of the page.
#[test]
fn a_form_without_resources_uses_the_fonts_of_the_page() {
    let bytes = build(b"/Fm1 Do", |doc| {
        let font = standard_font(doc, "Courier");
        let fm1 = form(doc, dictionary! {}, HELLO);
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => dictionary! { "Fm1" => reference(fm1) },
        }
    });
    let (doc, _) = parse(&bytes);
    assert_eq!(text(&doc), vec!["Hello form".to_string()]);
    assert_eq!(words(&doc)[0].builtin, Some(BuiltinFont::Courier));
    assert_eq!(text(&parse(&save(&doc)).0), text(&doc));
}

#[test]
fn a_form_without_text_still_draws_and_survives_a_save() {
    let bytes = build(b"/Fm1 Do", |doc| {
        let fm1 = form(doc, dictionary! {}, b"0 0 1 rg 10 10 50 50 re f");
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    let (doc, _) = parse(&bytes);
    assert!(text(&doc).is_empty());
    let (svg, _) = svg(&doc);
    assert!(svg.contains("<path") && svg.contains("rgb(0, 0, 255)"), "{svg}");
    let (again, _) = parse(&save(&doc));
    let (svg, _) = self::svg(&again);
    assert!(svg.contains("rgb(0, 0, 255)"), "{svg}");
}

#[test]
fn an_image_in_a_form_is_drawn_and_survives_a_save() {
    let bytes = build(b"q 100 0 0 100 20 20 cm /Fm1 Do Q", |doc| {
        let image = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 2, "Height" => 2,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
            },
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        ));
        let fm1 = form(
            doc,
            dictionary! {
                "BBox" => numbers(&[0, 0, 1, 1]),
                "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => reference(image) } },
            },
            b"/Im0 Do",
        );
        dictionary! { "XObject" => dictionary! { "Fm1" => reference(fm1) } }
    });
    let (doc, _) = parse(&bytes);
    let check = |doc: &PdfDocument| {
        let Some(XObject::Form(form)) = doc.resources.xobjects.map.values().next() else {
            panic!("no form");
        };
        assert_eq!(form.resources.as_ref().unwrap().xobjects.map.len(), 1, "the image is in the form");
        if cfg!(feature = "images") {
            assert!(svg(doc).0.contains("<image"), "the form's image is drawn");
        }
    };
    check(&doc);
    check(&parse(&save(&doc)).0);
}

/// A form with no resources of its own that draws itself (its name is the page's name for it).
#[test]
fn a_form_that_draws_itself_is_drawn_once() {
    let bytes = build(b"/Fm0 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm0 = form(doc, dictionary! {}, b"BT /F1 12 Tf 10 10 Td (loop) Tj ET /Fm0 Do");
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => dictionary! { "Fm0" => reference(fm0) },
        }
    });
    let (doc, _) = parse(&bytes);
    let (text, words, (svg, warnings)) = bounded("a form that draws itself", move || {
        (text(&doc), words(&doc), svg(&doc))
    });
    assert_eq!(text, vec!["loop".to_string()]);
    assert_eq!(words.len(), 1);
    assert_eq!(svg.matches("loop").count(), 1, "{svg}");
    assert!(warnings.iter().any(|w| w.msg.contains("draws itself")), "{warnings:?}");
}

/// Two forms that draw each other.
#[test]
fn two_forms_that_draw_each_other_are_drawn_once() {
    let bytes = build(b"/Fa Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fa = form(doc, dictionary! {}, b"BT /F1 12 Tf 10 10 Td (a) Tj ET /Fb Do");
        let fb = form(doc, dictionary! {}, b"BT /F1 12 Tf 10 30 Td (b) Tj ET /Fa Do");
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => dictionary! { "Fa" => reference(fa), "Fb" => reference(fb) },
        }
    });
    let (doc, _) = parse(&bytes);
    let text = bounded("forms that draw each other", move || text(&doc));
    assert_eq!(text, vec!["a".to_string(), "b".to_string()]);
}

/// Ten forms; each draws the next one ten times: ten billion draws of the last one.
#[test]
fn a_chain_of_forms_that_draw_the_next_ten_times_is_cut() {
    let bytes = build(b"/L1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let mut xobjects = Dictionary::new();
        for level in (1..=10).rev() {
            let mut content = format!("BT /F1 8 Tf 10 {} Td (level {level}) Tj ET ", 10 * level);
            if level < 10 {
                for _ in 0..10 {
                    content.push_str(&format!("/L{} Do ", level + 1));
                }
            }
            let id = form(doc, dictionary! {}, content.as_bytes());
            xobjects.set(format!("L{level}"), reference(id));
        }
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => xobjects,
        }
    });
    let (doc, _) = parse(&bytes);
    let (chunks, boxes, (svg, warnings), saved) = bounded("the chain", move || {
        (text(&doc), words(&doc).len(), svg(&doc), save(&doc))
    });
    // Some of the draws are made, and the rest is left out.
    assert!(!chunks.is_empty() && chunks.len() <= 20_000, "{} chunks", chunks.len());
    assert!(boxes > 0);
    assert!(warnings.iter().any(|w| w.msg.contains("is not drawn")), "{warnings:?}");
    assert!(svg.len() < 64 * 1024 * 1024);
    // Saving does not draw: the file is small.
    assert!(saved.len() < 1024 * 1024, "{} bytes", saved.len());
}

/// The same chain, with each form in the resources of the one before.
#[test]
fn a_chain_of_nested_forms_that_draw_the_next_ten_times_is_cut() {
    let bytes = build(b"/L1 Do", |doc| {
        let font = standard_font(doc, "Helvetica");
        let mut inner: Option<ObjectId> = None;
        for level in (1..=10).rev() {
            let mut resources = dictionary! { "Font" => dictionary! { "F1" => reference(font) } };
            let mut content = format!("BT /F1 8 Tf 10 {} Td (level {level}) Tj ET ", 10 * level);
            if let Some(next) = inner {
                resources.set("XObject", dictionary! { format!("L{}", level + 1) => reference(next) });
                for _ in 0..10 {
                    content.push_str(&format!("/L{} Do ", level + 1));
                }
            }
            inner = Some(form(doc, dictionary! { "Resources" => resources }, content.as_bytes()));
        }
        dictionary! { "XObject" => dictionary! { "L1" => reference(inner.unwrap()) } }
    });
    let (doc, _) = parse(&bytes);
    let (chunks, boxes, (_, warnings)) = bounded("the nested chain", move || {
        (text(&doc), words(&doc).len(), svg(&doc))
    });
    assert!(!chunks.is_empty() && chunks.len() <= 20_000, "{} chunks", chunks.len());
    assert!(boxes > 0);
    assert!(warnings.iter().any(|w| w.msg.contains("is not drawn")), "{warnings:?}");
}

/// A `Q` too many in a form does not restore the state of the page.
#[test]
fn a_form_cannot_pop_the_state_of_the_page() {
    let bytes = build(b"q 1 0 0 1 100 0 cm /Fm1 Do BT /F1 12 Tf 10 10 Td (after) Tj ET Q", |doc| {
        let font = standard_font(doc, "Helvetica");
        let fm1 = form(doc, dictionary! {}, b"Q Q Q BT /F1 12 Tf 10 50 Td (in) Tj ET");
        dictionary! {
            "Font" => dictionary! { "F1" => reference(font) },
            "XObject" => dictionary! { "Fm1" => reference(fm1) },
        }
    });
    let (doc, _) = parse(&bytes);
    let words = words(&doc);
    // Both are shifted by the page's `cm`: the form's `Q` did not undo it.
    let in_form = words.iter().find(|w| w.text == "in").unwrap();
    let after = words.iter().find(|w| w.text == "after").unwrap();
    assert_close(in_form.bbox[0], 110.0, "x of the form's text");
    assert_close(after.bbox[0], 110.0, "x of the text after the form");
    // Every group the form opened is closed.
    let (svg, _) = svg(&doc);
    let opened = svg.matches("<g>").count() + svg.matches("<g ").count();
    assert_eq!(opened, svg.matches("</g>").count(), "{svg}");
}

fn draft_ops() -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
            size: Pt(36.0),
        },
        Op::SetTextCursor { pos: Point { x: Pt(0.0), y: Pt(8.0) } },
        Op::ShowText { items: vec![TextItem::Text("DRAFT".to_string())] },
        Op::EndTextSection,
    ]
}

fn place(id: &XObjectId, x: f32, y: f32) -> Op {
    Op::UseXobject {
        id: id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(x)),
            translate_y: Some(Pt(y)),
            ..Default::default()
        },
    }
}

/// A form made in code is painted where the placement puts it, and saved.
#[test]
fn a_form_made_in_code_is_painted_and_saved() {
    let mut doc = PdfDocument::new("watermark");
    let id = doc.add_form(&FormXObject::new(Rect::from_wh(Pt(200.0), Pt(40.0)), draft_ops()));
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![place(&id, 100.0, 400.0), place(&id, 100.0, 100.0)],
    ));

    assert_eq!(text(&doc), vec!["DRAFT".to_string(), "DRAFT".to_string()]);
    let boxes = words(&doc);
    assert_eq!(boxes.len(), 2);
    // The placement moves the form 300 pt, and y grows downwards in the boxes.
    assert_close(boxes[1].bbox[1] - boxes[0].bbox[1], 300.0, "distance of the two");
    assert_close(boxes[0].bbox[0], 100.0, "left edge");
    let (svg, _) = svg(&doc);
    assert_eq!(svg.matches("DRAFT").count(), 2);

    let saved = save(&doc);
    let again = parse(&saved).0;
    assert_eq!(text(&again), text(&doc));
    let Some(XObject::Form(form)) = again.resources.xobjects.map.values().next() else {
        panic!("no form after the save");
    };
    assert_eq!(form.bbox, Rect::from_wh(Pt(200.0), Pt(40.0)));

    // The saved file has the form with its /BBox.
    let lopdf_doc = Document::load_mem(&saved).unwrap();
    let bbox: Vec<f32> = lopdf_doc
        .objects
        .values()
        .filter_map(|o| match o {
            Object::Stream(s) if s.dict.get(b"Subtype").and_then(|n| n.as_name()).ok() == Some(b"Form") => {
                s.dict.get(b"BBox").ok()?.as_array().ok().map(|a| {
                    a.iter().map(|n| n.as_float().unwrap_or(f32::NAN)).collect()
                })
            }
            _ => None,
        })
        .next()
        .expect("a form with a /BBox");
    assert_eq!(bbox, vec![0.0, 0.0, 200.0, 40.0]);
}

#[test]
fn the_matrix_of_a_form_made_in_code_scales_its_text() {
    let mut doc = PdfDocument::new("scaled");
    let plain = doc.add_form(&FormXObject::new(Rect::from_wh(Pt(200.0), Pt(40.0)), draft_ops()));
    let scaled = doc.add_form(
        &FormXObject::new(Rect::from_wh(Pt(200.0), Pt(40.0)), draft_ops())
            .with_matrix(CurTransMat::Raw([2.0, 0.0, 0.0, 2.0, 0.0, 0.0])),
    );
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![place(&plain, 0.0, 0.0), place(&scaled, 0.0, 0.0)],
    ));
    let boxes = words(&doc);
    assert_eq!(boxes.len(), 2);
    let width = |w: &printpdf::text_boxes::TextWord| w.bbox[2] - w.bbox[0];
    assert_close(width(&boxes[1]) / width(&boxes[0]), 2.0, "scale of the text");
}

/// A form without resources of its own uses the fonts of the document, and the glyphs it uses are
/// part of the font that is written.
#[test]
fn a_form_made_in_code_uses_a_font_of_the_document() {
    let bytes = include_bytes!("../examples/assets/fonts/Courier.ttf");
    let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("a font");
    let mut doc = PdfDocument::new("font of the document");
    let font_id = doc.add_font(&font);
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::External(font_id), size: Pt(14.0) },
        Op::SetTextCursor { pos: Point { x: Pt(10.0), y: Pt(10.0) } },
        Op::ShowText { items: vec![TextItem::Text("Courier text".to_string())] },
        Op::EndTextSection,
    ];
    let form = doc.add_form(&FormXObject::new(Rect::from_wh(Pt(300.0), Pt(40.0)), ops));
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), vec![place(&form, 50.0, 700.0)]));

    assert_eq!(text(&doc), vec!["Courier text".to_string()]);
    let again = parse(&save(&doc)).0;
    assert_eq!(text(&again), vec!["Courier text".to_string()]);
}

/// A form with resources of its own carries its font, and the document has none.
#[test]
fn a_form_made_in_code_can_have_a_font_of_its_own() {
    let bytes = include_bytes!("../examples/assets/fonts/Helvetica.ttf");
    let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("a font");
    let mut resources = PdfResources::default();
    resources
        .fonts
        .map
        .insert(FontId("Own".to_string()), PdfFont::new(font));
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::External(FontId("Own".to_string())), size: Pt(14.0) },
        Op::SetTextCursor { pos: Point { x: Pt(10.0), y: Pt(10.0) } },
        Op::ShowText { items: vec![TextItem::Text("Own font".to_string())] },
        Op::EndTextSection,
    ];
    let mut doc = PdfDocument::new("own font");
    let form = doc.add_form(
        &FormXObject::new(Rect::from_wh(Pt(300.0), Pt(40.0)), ops).with_resources(resources),
    );
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), vec![place(&form, 50.0, 700.0)]));
    assert!(doc.resources.fonts.map.is_empty());

    assert_eq!(text(&doc), vec!["Own font".to_string()]);
    let again = parse(&save(&doc)).0;
    assert_eq!(text(&again), vec!["Own font".to_string()]);
    assert!(again.resources.fonts.map.is_empty(), "the font is the form's");
    let Some(XObject::Form(form)) = again.resources.xobjects.map.values().next() else {
        panic!("no form after the save");
    };
    assert_eq!(form.resources.as_ref().unwrap().fonts.map.len(), 1);
}

#[test]
fn a_form_from_the_bytes_of_a_content_stream() {
    let form = FormXObject::from_content_stream(
        Rect::from_wh(Pt(100.0), Pt(100.0)),
        b"0 0 1 rg 10 10 50 50 re f",
    )
    .expect("a content stream");
    assert!(matches!(form.ops.first(), Some(Op::SetFillColor { .. })), "{:?}", form.ops);
    let mut doc = PdfDocument::new("raw");
    let id = doc.add_form(&form);
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), vec![place(&id, 0.0, 0.0)]));
    assert!(svg(&doc).0.contains("rgb(0, 0, 255)"));
    // A document with such a form is JSON.
    let json = serde_json::to_string(&doc).expect("to json");
    let back: PdfDocument = serde_json::from_str(&json).expect("from json");
    assert_eq!(back, doc);
}

/// The words of `pdftotext -layout`, with everything but letters and digits dropped.
fn pdftotext_words(path: &str) -> Option<Vec<String>> {
    let out = std::process::Command::new("pdftotext")
        .args(["-layout", path, "-"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
            .filter(|w| !w.is_empty())
            .collect(),
    )
}

/// How many of `words` appear in the text of the document, which is joined without separators:
/// producers like Quartz show a word in several pieces.
fn found(words: &[String], doc: &PdfDocument) -> usize {
    let haystack: String = doc
        .extract_text()
        .iter()
        .flatten()
        .flat_map(|s| s.chars())
        .filter(|c| c.is_alphanumeric())
        .collect();
    words.iter().filter(|w| haystack.contains(w.as_str())).count()
}

/// Real pages of macOS Quartz: the whole page is one form. Needs the files of the system and
/// `pdftotext` (poppler); skips itself without them.
#[test]
fn quartz_pages_have_the_words_poppler_finds() {
    for name in ["bashbug", "builtins", "rbash"] {
        let path = format!("/usr/share/doc/bash/{name}.pdf");
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("{path}: not on this machine, skipped");
            continue;
        };
        let Some(words) = pdftotext_words(&path) else {
            eprintln!("pdftotext is not available, skipped");
            return;
        };
        let doc = bounded(&path, {
            let bytes = bytes.clone();
            move || parse(&bytes).0
        });
        let before = found(&words, &doc);
        let saved = save(&doc);
        let again = parse(&saved).0;
        let after = found(&words, &again);
        eprintln!(
            "{name}: {before} of {} words of pdftotext found in the text of the document ({:.0}%), \
             {after} after a save ({:.0}%)",
            words.len(),
            100.0 * before as f64 / words.len() as f64,
            100.0 * after as f64 / words.len() as f64,
        );
        assert!(!words.is_empty());
        assert!(
            before * 10 >= words.len() * 8,
            "{name}: only {before} of {} words found",
            words.len()
        );
        assert!(
            after * 10 >= words.len() * 8,
            "{name}: only {after} of {} words found after a save",
            words.len()
        );
    }
}
