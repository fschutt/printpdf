#![cfg(feature = "text_layout")]

//! Reproduction / verification tests for open GitHub issues (triage toward 0.12).
//!
//! Run with:
//! ```sh
//! cargo test --test issue_repros                 # non-SVG issues
//! cargo test --test issue_repros --features svg  # + SVG issues (#211, #113, #184)
//! ```
//!
//! Each test names the issue it covers. Tests asserting *expected* behavior that is
//! still broken on master are the bug repros: they FAIL until the issue is fixed.
//!
//! Verified-fixed on 0.11.2 (tests pass):
//! - #253: fractional media/trim/crop box sizes must survive save (no integer rounding)
//! - #244: CMYK `k`/`K` operators must be parsed, not dropped (black-page bug)
//! - #216: XObject entries that are stream references must not be rejected
//! - #212: OTF/CFF (NotoSansJP) fonts: glyphs parse, FontBBox is not zero-height
//! - #213: `<br/>` in HTML input must not error
//! - #239: line breaking splits after an explicit hyphen
//! - #254: canonical custom-font usage (`SetFont` + `ShowText`) produces extractable text
//!
//! Repros that FAIL on 0.11.2 (fix in progress / needed):
//! - #211: SVG XObjects must define the `cs0` colorspace resource their content uses
//! - #113: SVG gradients need Pattern/Shading resources carried into the XObject
//! - #184: SVG `<text>` is silently dropped unless the CSS family exactly matches the
//!   fontdb family of a supplied font (legacy family names and the no-font-family
//!   default "Times New Roman" both drop the text)

use std::collections::BTreeMap;

use lopdf::{dictionary, Object, Stream};
use printpdf::*;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Builds a minimal single-page PDF with the given content stream and extra
/// resource entries, using lopdf directly (so the input is independent of
/// printpdf's serializer).
fn raw_pdf_with_content(content: &str, xobject: Option<(&str, Stream)>) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content.as_bytes().to_vec(),
    ));

    let mut resources = dictionary! {};
    if let Some((name, stream)) = xobject {
        let xobj_id = doc.add_object(stream);
        resources.set(
            "XObject",
            Object::Dictionary(dictionary! { name => Object::Reference(xobj_id) }),
        );
    }

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
        "Contents" => Object::Reference(content_id),
        "Resources" => Object::Dictionary(resources),
    });

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn parse_doc(bytes: &[u8]) -> (PdfDocument, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(
        bytes,
        &PdfParseOptions {
            fail_on_error: false,
        },
        &mut warnings,
    )
    .expect("PDF must parse");
    (doc, warnings)
}

#[cfg(feature = "html")]
fn pdftotext_available() -> bool {
    std::process::Command::new("pdftotext")
        .arg("-v")
        .output()
        .is_ok()
}

/// Extract text (layout mode) from PDF bytes via poppler's pdftotext.
#[cfg(feature = "html")]
fn pdftotext_layout(pdf: &[u8]) -> String {
    use std::io::Write;
    let dir = std::env::temp_dir().join(format!("printpdf_issue_repros_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("in.pdf");
    let mut f = std::fs::File::create(&pdf_path).unwrap();
    f.write_all(pdf).unwrap();
    drop(f);
    let out = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(&pdf_path)
        .arg("-")
        .output()
        .expect("pdftotext runs");
    String::from_utf8_lossy(&out.stdout).to_string()
}

// ---------------------------------------------------------------------------
// #253 — media boxes must not be rounded to integers
// ---------------------------------------------------------------------------

#[test]
fn issue_253_mediabox_survives_fractional_page_size() {
    // 31.304373 mm == 88.73631... pt. Integer rounding (the 0.8.x bug) would
    // store 88 or 89 pt (an error of >= 0.26 pt).
    let mm = 31.304373_f32;
    let expected_pt = mm * 72.0 / 25.4;

    let mut doc = PdfDocument::new("mediabox");
    let page = PdfPage::new(Mm(mm), Mm(mm), vec![]);
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    // Inspect with lopdf (independent of printpdf's parser).
    let parsed = lopdf::Document::load_mem(&bytes).unwrap();
    let (_, page_id) = parsed.get_pages().into_iter().next().unwrap();
    let page_dict = parsed.get_dictionary(page_id).unwrap();
    let media_box = page_dict.get(b"MediaBox").unwrap().as_array().unwrap();
    let w = match &media_box[2] {
        Object::Real(r) => *r as f32,
        Object::Integer(i) => *i as f32,
        o => panic!("unexpected MediaBox entry: {o:?}"),
    };
    assert!(
        (w - expected_pt).abs() < 0.01,
        "MediaBox width distorted: got {w}, expected {expected_pt} (issue #253)"
    );

    // And the printpdf parser must round-trip it too.
    let (roundtrip, _) = parse_doc(&bytes);
    let rt_w = roundtrip.pages[0].media_box.width.0;
    assert!(
        (rt_w - expected_pt).abs() < 0.01,
        "round-tripped MediaBox width distorted: got {rt_w}, expected {expected_pt}"
    );
}

// ---------------------------------------------------------------------------
// #244 — CMYK color operators must survive parsing
// ---------------------------------------------------------------------------

#[test]
fn issue_244_cmyk_fill_and_stroke_ops_are_parsed() {
    let bytes = raw_pdf_with_content(
        "0.1 0.2 0.3 0.4 k\n0 0 10 10 re\nf\n0.5 0.6 0.7 0.8 K\n10 10 m\n50 50 l\nS\n",
        None,
    );
    let (doc, warnings) = parse_doc(&bytes);

    for w in &warnings {
        assert!(
            !w.msg.contains("unhandled operator 'k'") && !w.msg.contains("unhandled operator 'K'"),
            "CMYK operators must be handled, got warning: {} (issue #244)",
            w.msg
        );
    }

    let ops = &doc.pages[0].ops;
    let fill = ops.iter().find_map(|op| match op {
        Op::SetFillColor {
            col: Color::Cmyk(c),
        } => Some(c.clone()),
        _ => None,
    });
    let stroke = ops.iter().find_map(|op| match op {
        Op::SetOutlineColor {
            col: Color::Cmyk(c),
        } => Some(c.clone()),
        _ => None,
    });

    let fill = fill.expect("'k' operator must produce Op::SetFillColor(Cmyk) (issue #244)");
    assert!((fill.c - 0.1).abs() < 1e-5 && (fill.k - 0.4).abs() < 1e-5);
    let stroke = stroke.expect("'K' operator must produce Op::SetOutlineColor(Cmyk) (issue #244)");
    assert!((stroke.c - 0.5).abs() < 1e-5 && (stroke.k - 0.8).abs() < 1e-5);
}

#[test]
fn issue_244_cmyk_roundtrip_through_printpdf() {
    // printpdf-written CMYK ops must be re-parseable by printpdf.
    let cmyk = Cmyk {
        c: 0.9,
        m: 0.1,
        y: 0.2,
        k: 0.05,
        icc_profile: None,
    };
    let mut doc = PdfDocument::new("cmyk");
    let page = PdfPage::new(
        Mm(50.0),
        Mm(50.0),
        vec![Op::SetFillColor {
            col: Color::Cmyk(cmyk.clone()),
        }],
    );
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    let (parsed, warnings) = parse_doc(&bytes);
    assert!(
        parsed.pages[0].ops.iter().any(|op| matches!(
            op,
            Op::SetFillColor {
                col: Color::Cmyk(c)
            } if (c.c - 0.9).abs() < 1e-4
        )),
        "CMYK fill did not survive the printpdf round-trip; warnings: {warnings:#?}"
    );
}

// ---------------------------------------------------------------------------
// #216 — XObject entries referencing stream objects must not be rejected
// ---------------------------------------------------------------------------

#[test]
fn issue_216_image_xobject_stream_reference_is_not_rejected() {
    // A 1x1 8-bit grayscale image XObject. Image XObjects are *streams*; the old
    // code resolved every XObject entry expecting a Dictionary and produced
    // 'Invalid dictionary reference ... expected "Dictionary", found "Stream"'.
    let img = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1,
            "Height" => 1,
            "ColorSpace" => "DeviceGray",
            "BitsPerComponent" => 8,
        },
        vec![0xFF],
    );
    let bytes = raw_pdf_with_content("q\n1 0 0 1 0 0 cm\n/Im1 Do\nQ\n", Some(("Im1", img)));
    let (doc, warnings) = parse_doc(&bytes);

    for w in &warnings {
        assert!(
            !w.msg.contains("Invalid dictionary reference"),
            "stream-typed XObject reference was rejected: {} (issue #216)",
            w.msg
        );
    }

    // The `Do` op must survive as UseXobject.
    assert!(
        doc.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, Op::UseXobject { id, .. } if id.0 == "Im1")),
        "expected UseXobject(Im1) op after parsing (issue #216); ops: {:#?}",
        doc.pages[0].ops
    );
}

// ---------------------------------------------------------------------------
// #212 — OTF/CFF fonts (NotoSansJP): glyphs must parse, FontBBox must have height
// ---------------------------------------------------------------------------

#[test]
fn issue_212_otf_cff_font_parses_with_glyphs() {
    let noto = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");
    let mut warnings = Vec::new();
    let font = ParsedFont::from_bytes(noto, 0, &mut warnings)
        .expect("NotoSansJP-Regular.otf must parse (issue #212)");
    assert!(
        font.num_glyphs > 100,
        "CFF font should report its glyphs, got {}",
        font.num_glyphs
    );
    // The reported bug: glyph outlines came back None for CFF -> FontBBox height 0.
    let gid = font
        .lookup_glyph_index('日' as u32)
        .expect("CJK codepoint must map to a glyph");
    assert!(gid != 0, "'日' must not map to .notdef");
}

#[test]
fn issue_212_cff_fontbbox_is_not_zero_height() {
    // kyasu1's finding: /FontBBox [0 0 1000 0] (zero height) because get_glyph_size
    // returned None for CFF outlines. Assert the embedded descriptor has a real bbox.
    let noto = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");
    let font = ParsedFont::from_bytes(noto, 0, &mut Vec::new()).unwrap();
    let mut doc = PdfDocument::new("cff bbox");
    let font_id = doc.add_font(&font);
    let page = PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font_id.clone()),
                size: Pt(20.0),
            },
            Op::SetTextCursor {
                pos: Point::new(Mm(10.0), Mm(50.0)),
            },
            Op::ShowText {
                items: vec![TextItem::Text("日本語".to_string())],
            },
            Op::EndTextSection,
        ],
    );
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());

    let parsed = lopdf::Document::load_mem(&bytes).unwrap();
    let mut checked = 0;
    for (_, obj) in parsed.objects.iter() {
        let Ok(dict) = obj.as_dict() else { continue };
        if dict.get(b"Type").and_then(|t| t.as_name()).ok() != Some(b"FontDescriptor") {
            continue;
        }
        let bbox = dict.get(b"FontBBox").unwrap().as_array().unwrap();
        let vals: Vec<f64> = bbox
            .iter()
            .map(|o| match o {
                Object::Integer(i) => *i as f64,
                Object::Real(r) => *r as f64,
                o => panic!("unexpected FontBBox entry {o:?}"),
            })
            .collect();
        assert!(
            (vals[3] - vals[1]).abs() > 1.0,
            "FontBBox has zero height: {vals:?} (issue #212)"
        );
        checked += 1;
    }
    assert!(checked > 0, "no FontDescriptor found in output PDF");
}

// ---------------------------------------------------------------------------
// #211 — SVG XObject must define the colorspace resource its content refers to
// ---------------------------------------------------------------------------

#[cfg(feature = "svg")]
mod svg_repros {
    use super::*;

    fn dict_get<'a>(map: &'a BTreeMap<String, DictItem>, key: &str) -> Option<&'a DictItem> {
        map.get(key)
    }

    #[test]
    fn issue_211_svg_xobject_defines_cs0_colorspace_resource() {
        // svg2pdf emits `/cs0 cs` (select named colorspace) in the content stream.
        // The XObject's /Resources must therefore contain
        //   /ColorSpace << /cs0 /DeviceRGB >>
        // Mapping /ColorSpace directly to a *name* leaves `/cs0` undefined ->
        // Adobe Acrobat refuses to render the page (issue #211).
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40" stroke="black" stroke-width="3" fill="red"/>
        </svg>"##;
        let mut warnings = Vec::new();
        let xobj = Svg::parse(svg, &mut warnings).expect("svg parses");

        let content = String::from_utf8_lossy(&xobj.stream.content).to_string();
        // Every colorspace name selected in the content stream...
        let used_cs: Vec<&str> = content
            .split_whitespace()
            .filter(|tok| tok.starts_with("/cs"))
            .map(|tok| tok.trim_start_matches('/'))
            .collect();

        if used_cs.is_empty() {
            // Content doesn't reference a named colorspace; nothing to check.
            return;
        }

        let resources = match dict_get(&xobj.stream.dict, "Resources") {
            Some(DictItem::Dict { map }) => map,
            other => panic!("XObject /Resources missing or not a dict: {other:?}"),
        };
        let colorspace = match dict_get(resources, "ColorSpace") {
            Some(DictItem::Dict { map }) => map,
            other => panic!(
                "issue #211: /Resources /ColorSpace must be a dictionary mapping resource \
                 names (e.g. cs0) to colorspaces so that `/cs0 cs` resolves; got: {other:?}"
            ),
        };
        for cs in used_cs {
            assert!(
                colorspace.contains_key(cs),
                "content stream selects /{cs} but /ColorSpace dict has no {cs} entry \
                 (issue #211); dict keys: {:?}",
                colorspace.keys().collect::<Vec<_>>()
            );
        }
    }

    // -----------------------------------------------------------------------
    // #184 — SVG <text> must actually produce drawing operations
    // -----------------------------------------------------------------------

    /// Count "real" drawing ops (paths/glyphs), ignoring the q/cm/Q scaffolding
    /// that translate_operations always emits even for an empty page.
    fn drawing_ops(ops: &[Op]) -> usize {
        ops.iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::DrawPolygon { .. }
                        | Op::DrawLine { .. }
                        | Op::ShowText { .. }
                        | Op::MoveToNextLineShowText { .. }
                )
            })
            .count()
    }

    #[test]
    fn issue_184_svg_text_with_matching_family_produces_paths() {
        // Control: RobotoMedium.ttf registers in fontdb under family "Roboto"
        // (its typographic family). With the *matching* family name the text
        // must survive as path outlines.
        let font = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60">
            <text x="10" y="40" font-family="Roboto" font-size="30">Hji</text>
        </svg>"#;
        let mut fonts = BTreeMap::new();
        fonts.insert("roboto".to_string(), font.to_vec());
        let xobj = Svg::parse_with_fonts(svg, &fonts, &mut Vec::new()).unwrap();
        let ops = xobj.stream.get_ops().unwrap();
        assert!(
            drawing_ops(&ops) > 0,
            "SVG <text> with matching family must produce path/glyph ops, got only \
             scaffolding: {ops:#?}"
        );
    }

    #[test]
    fn issue_184_svg_text_with_legacy_family_name_produces_paths() {
        // The font's *full name* / legacy family is "Roboto Medium" — this is what
        // users naturally write (and what tests/svg.rs uses). usvg only matches the
        // fontdb family ("Roboto"), logs "No match for 'Roboto Medium'" via `log`,
        // and silently DROPS the whole <text> node (issue #184).
        let font = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60">
            <text x="10" y="40" font-family="Roboto Medium" font-size="30">Hji</text>
        </svg>"#;
        let mut fonts = BTreeMap::new();
        fonts.insert("roboto".to_string(), font.to_vec());
        let xobj = Svg::parse_with_fonts(svg, &fonts, &mut Vec::new()).unwrap();
        let ops = xobj.stream.get_ops().unwrap();
        assert!(
            drawing_ops(&ops) > 0,
            "SVG <text font-family=\"Roboto Medium\"> was silently dropped even though \
             the font was supplied (issue #184)"
        );
    }

    #[test]
    fn issue_184_svg_text_without_family_uses_supplied_font() {
        // The original issue's SVG has NO font-family. usvg then falls back to
        // usvg::Options::font_family, whose default is "Times New Roman" — absent
        // on typical Linux servers and always absent on wasm — so the text is
        // silently dropped. When the caller supplies fonts, printpdf should make
        // them reachable as the default family instead.
        let font = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle cx="50" cy="50" r="40" stroke="black" stroke-width="3" fill="red"/>
            <text x="50" y="55" font-size="20" text-anchor="middle" fill="white">A</text>
        </svg>"#;
        let mut fonts = BTreeMap::new();
        fonts.insert("roboto".to_string(), font.to_vec());
        let xobj = Svg::parse_with_fonts(svg, &fonts, &mut Vec::new()).unwrap();
        let ops = xobj.stream.get_ops().unwrap();
        // circle fill + circle stroke = 2 polygons; the glyph "A" must add more.
        assert!(
            drawing_ops(&ops) > 2,
            "the <text> element (no font-family) was dropped; only the circle was \
             drawn (issue #184). ops: {}",
            drawing_ops(&ops)
        );
    }

    // -----------------------------------------------------------------------
    // #113 — gradients: pattern/shading resources must be carried into the XObject
    // -----------------------------------------------------------------------

    #[test]
    fn issue_113_svg_gradient_resources_survive() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs>
                <linearGradient id="g" x1="0" y1="0" x2="1" y2="0">
                    <stop offset="0" stop-color="#ff0000"/>
                    <stop offset="1" stop-color="#0000ff"/>
                </linearGradient>
            </defs>
            <rect x="0" y="0" width="100" height="100" fill="url(#g)"/>
        </svg>"##;
        let mut warnings = Vec::new();
        let xobj = Svg::parse(svg, &mut warnings).expect("svg parses");
        let content = String::from_utf8_lossy(&xobj.stream.content).to_string();

        // svg2pdf paints gradients via a pattern (`/px scn`) or a shading (`/sx sh`),
        // both of which need matching entries in the XObject's /Resources.
        // A translation that drops them yields a blank/solid rect (issue #113).
        let uses_pattern = content.contains(" scn") && content.contains("/Pattern");
        let uses_shading = content
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[1] == "sh");

        assert!(
            uses_pattern || uses_shading,
            "issue #113: gradient fill was dropped entirely from the translated content \
             stream.\n--- content ---\n{content}"
        );

        let resources = match xobj.stream.dict.get("Resources") {
            Some(DictItem::Dict { map }) => map,
            other => panic!("XObject /Resources missing or not a dict: {other:?}"),
        };
        assert!(
            resources.contains_key("Pattern") || resources.contains_key("Shading"),
            "issue #113: content uses a gradient but /Resources carries no /Pattern or \
             /Shading entry; keys: {:?}",
            resources.keys().collect::<Vec<_>>()
        );
    }
}

// ---------------------------------------------------------------------------
// #213 — `<br/>` must not fail HTML -> PDF conversion
// ---------------------------------------------------------------------------

#[cfg(feature = "html")]
#[test]
fn issue_213_br_tag_does_not_error() {
    let html = r#"<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>test</title></head>
<body>html test <br/> this is printpdf</body></html>"#;

    let mut warnings = Vec::new();
    let res = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    );
    let doc = res.expect("issue #213: <br/> must not abort HTML conversion");
    assert!(!doc.pages.is_empty(), "conversion must produce a page");
}

// ---------------------------------------------------------------------------
// #239 — line breaking should split after an explicit hyphen
// ---------------------------------------------------------------------------

#[cfg(feature = "html")]
#[test]
fn issue_239_line_break_after_explicit_hyphen() {
    if !pdftotext_available() {
        eprintln!("skipping: pdftotext not available");
        return;
    }

    // 39 mm wide text column (as in the issue). "For demonstration-purpose this
    // should break." must wrap *after* the hyphen: "For demonstration-" /
    // "purpose this should" / "break." — not overflow the column.
    let roboto = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
    let mut fonts = BTreeMap::new();
    fonts.insert(
        "Roboto Medium".to_string(),
        Base64OrRaw::Raw(roboto.to_vec()),
    );

    let html = r#"<!DOCTYPE html>
<html><head><style>
body { margin: 0; font-family: "Roboto Medium"; font-size: 12pt; width: 39mm; }
</style></head>
<body><p>For demonstration-purpose this should break.</p></body></html>"#;

    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions {
            page_width: Some(45.0),
            page_height: Some(200.0),
            ..Default::default()
        },
        &mut warnings,
    )
    .expect("html renders");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());

    let text = pdftotext_layout(&bytes);
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    assert!(
        lines.len() >= 2,
        "issue #239: text in a 39mm column must wrap onto multiple lines, got: {lines:?}"
    );
    // The first line must end at the hyphen — the word "demonstration-purpose"
    // is wider than 39mm and contains a legal break opportunity after '-'.
    assert!(
        lines[0].ends_with('-'),
        "issue #239: expected the first line to break after the explicit hyphen \
         (\"For demonstration-\"), got lines: {lines:?}"
    );
}

// ---------------------------------------------------------------------------
// #254 — canonical custom-font text flow must produce visible/extractable text
// ---------------------------------------------------------------------------

#[cfg(feature = "html")]
#[test]
fn issue_254_custom_font_canonical_usage_is_extractable() {
    if !pdftotext_available() {
        eprintln!("skipping: pdftotext not available");
        return;
    }
    let roboto = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
    let font = ParsedFont::from_bytes(roboto, 0, &mut Vec::new()).unwrap();
    let mut doc = PdfDocument::new("custom font");
    let font_id = doc.add_font(&font);
    let page = PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font_id.clone()),
                size: Pt(14.0),
            },
            Op::SetTextCursor {
                pos: Point::new(Mm(10.0), Mm(50.0)),
            },
            Op::ShowText {
                items: vec![TextItem::Text("dolor sit amet".to_string())],
            },
            Op::EndTextSection,
        ],
    );
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut Vec::new());
    let text = pdftotext_layout(&bytes);
    assert!(
        text.contains("dolor sit amet"),
        "issue #254: canonical StartTextSection/SetFont/ShowText flow must produce \
         extractable text, got: {text:?}"
    );
}
