//! Patterns survive the round trip printpdf -> bytes -> parse -> bytes -> parse: the `/Pattern`
//! resources, the `/Pattern cs /Name scn` operators and the colors of uncolored patterns. Also a
//! PDF built with lopdf, independent of the writer of this crate, that uses patterns.

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use printpdf::*;

fn save(doc: &PdfDocument) -> (Vec<u8>, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    (bytes, warnings)
}

fn parse(bytes: &[u8]) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings).expect("parse");
    (doc, warnings)
}

/// The JSON of a value. `Point` compares exact zeros as unequal, so values that hold polygons
/// are compared as JSON.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("json")
}

fn fill(x: f32, y: f32, w: f32, h: f32) -> Op {
    Op::DrawRectangle {
        rectangle: Rect {
            mode: Some(PaintMode::Fill),
            ..Rect::from_xywh(Pt(x), Pt(y), Pt(w), Pt(h))
        },
    }
}

fn alpha(value: f32) -> ExtendedGraphicsState {
    let mut state = ExtendedGraphicsState::default();
    state.set_current_fill_alpha(value);
    state
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::Rgb(Rgb::new(r, g, b, None))
}

fn gradient() -> Shading {
    Shading {
        geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 100.0, 0.0] },
        stops: vec![
            GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
            GradientStop { offset: 0.5, color: [0.0, 1.0, 0.0] },
            GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
        ],
        extend: (true, false),
    }
}

/// A document with a colored tiling pattern, an uncolored one, and a shading pattern, each used
/// as a fill and the shading and the uncolored one also as a stroke.
fn pattern_doc() -> PdfDocument {
    let mut doc = PdfDocument::new("patterns");
    let patterns = &mut doc.resources.patterns.map;

    patterns.insert(
        PatternId("Checks".into()),
        Pattern::colored_tiling(
            Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(10.0), Pt(10.0)),
            12.0,
            14.0,
            vec![
                Op::SetFillColor { col: rgb(1.0, 0.0, 0.0) },
                fill(0.0, 0.0, 5.0, 5.0),
                fill(5.0, 5.0, 5.0, 5.0),
            ],
        )
        .with_matrix([1.0, 0.0, 0.0, 1.0, 3.0, 4.0]),
    );
    patterns.insert(
        PatternId("Dots".into()),
        Pattern::uncolored_tiling(
            Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(8.0), Pt(8.0)),
            8.0,
            8.0,
            vec![fill(2.0, 2.0, 4.0, 4.0)],
        ),
    );
    patterns.insert(
        PatternId("Grad".into()),
        Pattern::Shading {
            shading: gradient(),
            matrix: [1.0, 0.0, 0.0, 1.0, 20.0, 30.0],
            ext_gstate: None,
        },
    );

    let ops = vec![
        Op::SetFillPattern { id: PatternId("Checks".into()), color: None },
        fill(10.0, 10.0, 100.0, 50.0),
        Op::SetFillPattern {
            id: PatternId("Dots".into()),
            color: Some(Color::Cmyk(Cmyk::new(0.1, 0.2, 0.3, 0.4, None))),
        },
        fill(10.0, 70.0, 100.0, 50.0),
        Op::SetFillPattern { id: PatternId("Grad".into()), color: None },
        fill(10.0, 130.0, 100.0, 50.0),
        Op::SetOutlinePattern { id: PatternId("Grad".into()), color: None },
        Op::SetOutlineThickness { pt: Pt(8.0) },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point { x: Pt(10.0), y: Pt(200.0) }, bezier: false },
                    LinePoint { p: Point { x: Pt(110.0), y: Pt(200.0) }, bezier: false },
                ],
                is_closed: false,
            },
        },
        Op::SetOutlinePattern {
            id: PatternId("Dots".into()),
            color: Some(Color::Greyscale(Greyscale::new(0.25, None))),
        },
        Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint { p: Point { x: Pt(10.0), y: Pt(220.0) }, bezier: false },
                    LinePoint { p: Point { x: Pt(110.0), y: Pt(220.0) }, bezier: false },
                ],
                is_closed: false,
            },
        },
    ];
    doc.pages.push(PdfPage::new(Mm(100.0), Mm(100.0), ops));
    doc
}

fn pattern_ops(ops: &[Op]) -> Vec<&Op> {
    ops.iter()
        .filter(|op| matches!(op, Op::SetFillPattern { .. } | Op::SetOutlinePattern { .. }))
        .collect()
}

#[test]
fn patterns_and_pattern_ops_survive_the_round_trip() {
    let doc = pattern_doc();
    let (bytes, saved) = save(&doc);
    assert!(
        !saved.iter().any(|w| w.severity == PdfParseErrorSeverity::Error),
        "{saved:?}"
    );

    let (parsed, warnings) = parse(&bytes);
    assert_eq!(parsed.resources.patterns.map.len(), 3, "{warnings:#?}");

    // The same pattern ops, in the same order, with the same names and colors.
    assert_eq!(
        pattern_ops(&parsed.pages[0].ops),
        pattern_ops(&doc.pages[0].ops),
        "{warnings:#?}"
    );
    // The `/Pattern cs` operators are part of the pattern ops, not ops of their own.
    assert!(
        !parsed.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::SetColorSpaceFill { .. } | Op::SetColorSpaceStroke { .. })),
        "{:?}",
        parsed.pages[0].ops
    );

    // The tiling patterns keep their numbers.
    match &parsed.resources.patterns.map[&PatternId("Checks".into())] {
        Pattern::Tiling {
            paint_type,
            bbox,
            xstep,
            ystep,
            matrix,
            ops,
            resources,
        } => {
            assert_eq!(*paint_type, PatternPaintType::Colored);
            assert_eq!((bbox.x.0, bbox.y.0, bbox.width.0, bbox.height.0), (0.0, 0.0, 10.0, 10.0));
            assert_eq!((*xstep, *ystep), (12.0, 14.0));
            assert_eq!(*matrix, [1.0, 0.0, 0.0, 1.0, 3.0, 4.0]);
            assert!(ops.iter().any(|op| matches!(op, Op::SetFillColor { .. })));
            assert_eq!(ops.iter().filter(|op| matches!(op, Op::DrawPolygon { .. })).count(), 2);
            // A pattern without resources is read without resources.
            assert!(resources.is_none(), "{resources:?}");
        }
        other => panic!("not a tiling pattern: {other:?}"),
    }
    match &parsed.resources.patterns.map[&PatternId("Dots".into())] {
        Pattern::Tiling { paint_type, .. } => {
            assert_eq!(*paint_type, PatternPaintType::Uncolored);
        }
        other => panic!("not a tiling pattern: {other:?}"),
    }
    match &parsed.resources.patterns.map[&PatternId("Grad".into())] {
        Pattern::Shading { shading, matrix, ext_gstate } => {
            assert_eq!(shading.geometry, ShadingGeometry::Axial { coords: [0.0, 0.0, 100.0, 0.0] });
            assert_eq!(shading.extend, (true, false));
            assert_eq!(shading.stops.len(), 3);
            assert_eq!(*matrix, [1.0, 0.0, 0.0, 1.0, 20.0, 30.0]);
            assert!(ext_gstate.is_none());
        }
        other => panic!("not a shading pattern: {other:?}"),
    }

    // A second round trip changes nothing.
    let (bytes2, _) = save(&parsed);
    let (parsed2, _) = parse(&bytes2);
    assert_eq!(json(&parsed2.resources.patterns), json(&parsed.resources.patterns));
    assert_eq!(json(&parsed2.pages[0].ops), json(&parsed.pages[0].ops));
}

#[test]
fn the_written_operators_are_cs_and_scn() {
    let (bytes, _) = save(&pattern_doc());
    let lopdf_doc = Document::load_mem(&bytes).expect("load");
    let page_id = *lopdf_doc.get_pages().values().next().expect("a page");
    let content = lopdf_doc.get_page_content(page_id);
    let text = String::from_utf8_lossy(&content).into_owned();
    assert!(text.contains("/Pattern cs"), "{text}");
    assert!(text.contains("/Checks scn"), "{text}");
    assert!(text.contains("/Pattern CS"), "{text}");
    assert!(text.contains("/Grad SCN"), "{text}");
    // The uncolored pattern names a color space with its underlying color space.
    assert!(text.contains("/PdfPatternCmyk cs"), "{text}");
    assert!(text.contains("0.1 0.2 0.3 0.4 /Dots scn"), "{text}");
    assert!(text.contains("/PdfPatternGray CS"), "{text}");
    assert!(text.contains("0.25 /Dots SCN"), "{text}");

    // The resources: patterns as objects, with the dictionary entries ISO 32000-1 asks for.
    let page = lopdf_doc.get_dictionary(page_id).expect("page");
    let resources = lopdf_doc
        .get_dictionary(page.get(b"Resources").expect("res").as_reference().expect("ref"))
        .expect("resources");
    let patterns = lopdf_doc
        .get_dictionary(resources.get(b"Pattern").expect("pattern").as_reference().expect("ref"))
        .expect("patterns");
    assert_eq!(patterns.len(), 3);
    let checks = lopdf_doc
        .get_object(patterns.get(b"Checks").expect("Checks").as_reference().expect("ref"))
        .expect("object")
        .as_stream()
        .expect("a tiling pattern is a stream");
    let d = &checks.dict;
    assert_eq!(d.get(b"PatternType").expect("PatternType").as_i64().expect("int"), 1);
    assert_eq!(d.get(b"PaintType").expect("PaintType").as_i64().expect("int"), 1);
    assert_eq!(d.get(b"TilingType").expect("TilingType").as_i64().expect("int"), 1);
    for key in [&b"BBox"[..], b"XStep", b"YStep", b"Matrix", b"Resources"] {
        assert!(d.has(key), "missing /{}", String::from_utf8_lossy(key));
    }
    let grad = lopdf_doc
        .get_dictionary(patterns.get(b"Grad").expect("Grad").as_reference().expect("ref"))
        .expect("a shading pattern is a dictionary");
    assert_eq!(grad.get(b"PatternType").expect("PatternType").as_i64().expect("int"), 2);
    assert!(grad.get(b"Shading").expect("Shading").as_dict().is_ok());
}

#[test]
fn a_tiling_pattern_keeps_its_own_resources() {
    let mut doc = PdfDocument::new("pattern resources");
    let gs = ExtendedGraphicsStateId("Half".into());
    let mut pattern_resources = PdfResources::default();
    pattern_resources.extgstates.map.insert(
        gs.clone(),
        alpha(0.5),
    );
    pattern_resources.shadings.map.insert(ShadingId("Sh1".into()), gradient());
    let cell = vec![
        Op::LoadGraphicsState { gs: gs.clone() },
        Op::SetFillColor { col: rgb(0.0, 0.5, 0.0) },
        fill(0.0, 0.0, 20.0, 20.0),
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::Builtin(BuiltinFont::Helvetica), size: Pt(8.0) },
        Op::SetTextCursor { pos: Point { x: Pt(1.0), y: Pt(2.0) } },
        Op::ShowText { items: vec![TextItem::Text("ab".into())] },
        Op::EndTextSection,
    ];
    doc.resources.patterns.map.insert(
        PatternId("Rich".into()),
        Pattern::colored_tiling(Rect::from_wh(Pt(20.0), Pt(20.0)), 20.0, 20.0, cell)
            .with_resources(pattern_resources),
    );
    doc.pages.push(PdfPage::new(
        Mm(50.0),
        Mm(50.0),
        vec![
            Op::SetFillPattern { id: PatternId("Rich".into()), color: None },
            fill(0.0, 0.0, 100.0, 100.0),
        ],
    ));

    let (bytes, _) = save(&doc);
    let (parsed, warnings) = parse(&bytes);
    match &parsed.resources.patterns.map[&PatternId("Rich".into())] {
        Pattern::Tiling { resources: Some(resources), ops, .. } => {
            assert_eq!(resources.extgstates.map.len(), 1, "{warnings:#?}");
            assert_eq!(resources.shadings.map.len(), 1);
            assert!(ops.iter().any(|op| matches!(op, Op::LoadGraphicsState { .. })));
            assert!(ops.iter().any(|op| matches!(op, Op::ShowText { .. })));
        }
        other => panic!("the pattern resources are lost: {other:?} {warnings:#?}"),
    }
    let (bytes2, _) = save(&parsed);
    let (parsed2, _) = parse(&bytes2);
    assert_eq!(json(&parsed2.resources.patterns), json(&parsed.resources.patterns));
}

#[test]
fn a_shading_pattern_keeps_its_graphics_state() {
    let mut doc = PdfDocument::new("pattern gs");
    let state = alpha(0.25);
    doc.resources.patterns.map.insert(
        PatternId("Faint".into()),
        Pattern::Shading { shading: gradient(), matrix: IDENTITY_MATRIX, ext_gstate: Some(state) },
    );
    doc.pages.push(PdfPage::new(
        Mm(50.0),
        Mm(50.0),
        vec![
            Op::SetFillPattern { id: PatternId("Faint".into()), color: None },
            fill(0.0, 0.0, 100.0, 100.0),
        ],
    ));
    let (bytes, _) = save(&doc);
    let (parsed, warnings) = parse(&bytes);
    match &parsed.resources.patterns.map[&PatternId("Faint".into())] {
        Pattern::Shading { ext_gstate: Some(state), .. } => {
            assert!(state.has_changed(ChangedField::CurrentFillAlpha));
            assert_eq!(*state, alpha(0.25));
        }
        other => panic!("the graphics state is lost: {other:?} {warnings:#?}"),
    }
}

#[test]
fn a_form_has_patterns_of_its_own() {
    let mut doc = PdfDocument::new("form patterns");
    let mut form_resources = PdfResources::default();
    form_resources.patterns.map.insert(
        PatternId("P1".into()),
        Pattern::colored_tiling(
            Rect::from_wh(Pt(6.0), Pt(6.0)),
            6.0,
            6.0,
            vec![Op::SetFillColor { col: rgb(0.0, 0.0, 1.0) }, fill(0.0, 0.0, 3.0, 3.0)],
        ),
    );
    let form = FormXObject::new(
        Rect::from_wh(Pt(100.0), Pt(100.0)),
        vec![
            Op::SetFillPattern { id: PatternId("P1".into()), color: None },
            fill(0.0, 0.0, 100.0, 100.0),
        ],
    )
    .with_resources(form_resources);
    let id = doc.add_form(&form);
    doc.pages.push(PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![Op::UseXobject {
            id: id.clone(),
            transform: XObjectTransform { no_auto_scale: true, ..Default::default() },
        }],
    ));

    let (bytes, _) = save(&doc);
    let (parsed, warnings) = parse(&bytes);
    let Some(XObject::Form(parsed_form)) = parsed.resources.xobjects.map.get(&id) else {
        panic!("the form is not modelled: {warnings:#?}");
    };
    let resources = parsed_form.resources.as_ref().expect("form resources");
    assert!(matches!(
        resources.patterns.map.get(&PatternId("P1".into())),
        Some(Pattern::Tiling { .. })
    ));
    assert_eq!(
        pattern_ops(&parsed_form.ops),
        vec![&Op::SetFillPattern { id: PatternId("P1".into()), color: None }]
    );
    // The page has no patterns: the form's are its own.
    assert!(parsed.resources.patterns.map.is_empty());
}

// ---------------------------------------------------------------------------------------------
// A PDF that was not written by this crate.

fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().map(|v| Object::Real(*v)).collect())
}

/// A one-page document (200 x 200). `build` adds objects and returns the page's `/Resources`.
fn handmade(content: &[u8], build: impl FnOnce(&mut Document) -> Dictionary) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let resources = build(&mut doc);
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => numbers(&[0.0, 0.0, 200.0, 200.0]),
        "Resources" => resources,
        "Contents" => Object::Reference(content_id),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Count" => 1, "Kids" => vec![Object::Reference(page)],
        }),
    );
    let catalog =
        doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", Object::Reference(catalog));
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("save");
    out
}

fn tiling_stream(doc: &mut Document, dict: Dictionary, content: &[u8]) -> ObjectId {
    let mut dict = dict;
    dict.set("Type", "Pattern");
    dict.set("PatternType", 1);
    doc.add_object(Stream::new(dict, content.to_vec()))
}

#[test]
fn a_pdf_with_a_tiling_pattern_is_read() {
    // A pattern as Cairo or Quartz write it: `/Pattern cs /P1 scn`, then a named color space
    // `[/Pattern /DeviceRGB]` for an uncolored pattern.
    let content = b"/Pattern cs /P1 scn 10 10 80 80 re f \
                    /Cs1 cs 1 0 0 /P2 scn 100 10 80 80 re f \
                    /Pattern CS /P1 SCN 4 w 10 120 m 190 120 l S";
    let bytes = handmade(content, |doc| {
        let p1 = tiling_stream(
            doc,
            dictionary! {
                "PaintType" => 1, "TilingType" => 1,
                "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
                "XStep" => 10, "YStep" => 10,
                "Resources" => dictionary! {},
            },
            b"0 0 1 rg 0 0 5 5 re f",
        );
        let p2 = tiling_stream(
            doc,
            dictionary! {
                "PaintType" => 2, "TilingType" => 1,
                "BBox" => numbers(&[0.0, 0.0, 4.0, 4.0]),
                "XStep" => 4.5, "YStep" => 4.5,
                "Matrix" => numbers(&[2.0, 0.0, 0.0, 2.0, 1.0, 1.0]),
                "Resources" => dictionary! {},
            },
            b"0 0 2 2 re f",
        );
        dictionary! {
            "Pattern" => dictionary! { "P1" => Object::Reference(p1), "P2" => Object::Reference(p2) },
            "ColorSpace" => dictionary! {
                "Cs1" => vec![Object::Name(b"Pattern".to_vec()), Object::Name(b"DeviceRGB".to_vec())],
            },
        }
    });

    let (parsed, warnings) = parse(&bytes);
    assert_eq!(parsed.resources.patterns.map.len(), 2, "{warnings:#?}");
    match &parsed.resources.patterns.map[&PatternId("P2".into())] {
        Pattern::Tiling { paint_type, xstep, matrix, .. } => {
            assert_eq!(*paint_type, PatternPaintType::Uncolored);
            assert_eq!(*xstep, 4.5);
            assert_eq!(*matrix, [2.0, 0.0, 0.0, 2.0, 1.0, 1.0]);
        }
        other => panic!("not a tiling pattern: {other:?}"),
    }
    assert_eq!(
        pattern_ops(&parsed.pages[0].ops),
        vec![
            &Op::SetFillPattern { id: PatternId("P1".into()), color: None },
            &Op::SetFillPattern { id: PatternId("P2".into()), color: Some(rgb(1.0, 0.0, 0.0)) },
            &Op::SetOutlinePattern { id: PatternId("P1".into()), color: None },
        ]
    );
    // None of the three fills became a plain gray.
    assert!(
        !parsed.pages[0].ops.iter().any(|op| matches!(op, Op::SetFillColor { .. })),
        "{:?}",
        parsed.pages[0].ops
    );
    // The patterns are written again.
    let (again, _) = save(&parsed);
    let (parsed_again, _) = parse(&again);
    assert_eq!(json(&parsed_again.resources.patterns), json(&parsed.resources.patterns));
    assert_eq!(json(&parsed_again.pages[0].ops), json(&parsed.pages[0].ops));
}

#[test]
fn a_pattern_that_has_no_model_is_kept() {
    // A tiling pattern with TilingType 2 (no distortion), a nested XObject in its resources,
    // and a shading pattern with a mesh shading (a stream).
    let bytes = handmade(
        b"/Pattern cs /Tile scn 0 0 100 100 re f /Mesh scn 100 0 100 100 re f",
        |doc| {
            let tile = tiling_stream(
                doc,
                dictionary! {
                    "PaintType" => 1, "TilingType" => 2,
                    "BBox" => numbers(&[0.0, 0.0, 10.0, 10.0]),
                    "XStep" => 10, "YStep" => 10,
                    "Resources" => dictionary! {},
                },
                b"1 0 0 rg 0 0 5 5 re f",
            );
            let mesh = doc.add_object(Stream::new(
                dictionary! {
                    "ShadingType" => 4, "ColorSpace" => "DeviceRGB", "BitsPerCoordinate" => 8,
                    "BitsPerComponent" => 8, "BitsPerFlag" => 8,
                    "Decode" => numbers(&[0.0, 100.0, 0.0, 100.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]),
                },
                vec![0u8; 18],
            ));
            let mesh_pattern = doc.add_object(dictionary! {
                "Type" => "Pattern", "PatternType" => 2, "Shading" => Object::Reference(mesh),
            });
            dictionary! {
                "Pattern" => dictionary! {
                    "Tile" => Object::Reference(tile), "Mesh" => Object::Reference(mesh_pattern),
                },
            }
        },
    );
    let (parsed, warnings) = parse(&bytes);
    assert_eq!(parsed.resources.patterns.map.len(), 2, "{warnings:#?}");
    for name in ["Tile", "Mesh"] {
        assert!(
            matches!(
                parsed.resources.patterns.map.get(&PatternId(name.into())),
                Some(Pattern::External(_))
            ),
            "{name}: {:?}",
            parsed.resources.patterns.map.get(&PatternId(name.into()))
        );
    }
    assert!(warnings.iter().any(|w| w.msg.contains("kept as it is")), "{warnings:#?}");

    // They come back in the saved file as they were.
    let (again, _) = save(&parsed);
    let (parsed_again, _) = parse(&again);
    assert_eq!(json(&parsed_again.resources.patterns), json(&parsed.resources.patterns));
    let lopdf_doc = Document::load_mem(&again).expect("load");
    let mut mesh_streams = 0;
    for object in lopdf_doc.objects.values() {
        if let Object::Stream(stream) = object {
            if stream.dict.get(b"ShadingType").is_ok() {
                mesh_streams += 1;
                assert_eq!(stream.dict.get(b"ShadingType").expect("type").as_i64().expect("int"), 4);
            }
        }
    }
    assert_eq!(mesh_streams, 1);
}
