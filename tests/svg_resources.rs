//! Structural audit for the SVG -> PDF pipeline (issues #113 and #211).
//!
//! Issue #113: content streams produced from SVGs reference named resources
//! (`/cs0 cs`, `/gs0 gs`, `/p0 scn`, `/x0 Do`, `/sh0 sh`, ...) whose
//! definitions must be carried into the Form XObject's `/Resources`
//! dictionary. Earlier versions dropped the whole subtree, which Acrobat
//! rejects and which turns gradients into flat fills.
//!
//! Issue #211: `/Resources /ColorSpace` was written as a bare name
//! (`/ColorSpace /DeviceRGB`) instead of a dictionary mapping resource names
//! to color spaces.
//!
//! These tests build a real PDF (tiger + camera + a gradient/pattern/text
//! SVG), reload it with lopdf and verify:
//!   1. every name referenced by a content-stream operator inside an
//!      SVG-derived Form XObject resolves in that XObject's own /Resources
//!      (recursively, including nested XObjects, tiling patterns and
//!      soft-mask groups),
//!   2. every indirect reference inside those subtrees resolves to a real
//!      object (no dangling references),
//!   3. /ColorSpace resource entries are dictionaries, not bare names,
//!   4. gradient paints survive as pattern/shading paints instead of being
//!      flattened to grayscale.

#![cfg(feature = "svg")]

use std::collections::{BTreeMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};
use printpdf::*;

const TIGER_SVG: &str = include_str!("./tiger.svg");
const CAMERA_SVG: &str = include_str!("../examples/assets/svg/AJ_Digital_Camera.svg");

/// linearGradient + radialGradient + pattern + clipPath + group opacity + text
const GRADIENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="300" viewBox="0 0 400 300">
  <defs>
    <linearGradient id="lin" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#ff0000"/>
      <stop offset="50%" stop-color="#ffff00"/>
      <stop offset="100%" stop-color="#00aa00"/>
    </linearGradient>
    <radialGradient id="rad" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#0000ff"/>
      <stop offset="100%" stop-color="#00ffff" stop-opacity="0.3"/>
    </radialGradient>
    <pattern id="dots" width="20" height="20" patternUnits="userSpaceOnUse">
      <rect width="20" height="20" fill="#eeeeee"/>
      <circle cx="10" cy="10" r="6" fill="#aa00aa"/>
    </pattern>
    <clipPath id="clip">
      <circle cx="320" cy="80" r="50"/>
    </clipPath>
  </defs>
  <rect x="10" y="10" width="180" height="80" fill="url(#lin)" stroke="black" stroke-width="2"/>
  <circle cx="100" cy="180" r="60" fill="url(#rad)"/>
  <rect x="200" y="140" width="120" height="100" fill="url(#dots)" stroke="#333333"/>
  <rect x="270" y="30" width="100" height="100" fill="url(#lin)" clip-path="url(#clip)"/>
  <g opacity="0.5">
    <rect x="150" y="60" width="100" height="60" fill="#ff6600"/>
    <circle cx="200" cy="90" r="25" fill="#0066ff" fill-opacity="0.7"/>
  </g>
  <text x="20" y="280" font-family="Roboto" font-size="28" fill="#222222">Grad Text</text>
  <text x="200" y="280" font-family="Roboto" font-size="28" fill="url(#lin)">RGB</text>
</svg>"##;

/// Builds a PDF with all three SVGs, saves it, and reloads it with lopdf.
fn build_test_pdf() -> Vec<u8> {
    let mut warnings = Vec::new();
    let mut doc = PdfDocument::new("svg-structural-audit");
    let mut ops = Vec::new();

    let mut fonts = BTreeMap::new();
    fonts.insert(
        "roboto-medium".to_string(),
        include_bytes!("../examples/assets/fonts/RobotoMedium.ttf").to_vec(),
    );

    for (i, svg) in [TIGER_SVG, CAMERA_SVG, GRADIENT_SVG].iter().enumerate() {
        let parsed = Svg::parse_with_fonts(svg, &fonts, &mut warnings)
            .unwrap_or_else(|e| panic!("failed to parse test svg #{i}: {e}"));
        let id = doc.add_xobject(&parsed);
        ops.push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(Pt(30.0)),
                translate_y: Some(Pt(40.0 + 250.0 * i as f32)),
                scale_x: Some(0.5),
                scale_y: Some(0.5),
                ..Default::default()
            },
        });
    }

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // For visual inspection: PRINTPDF_SVG_TEST_OUT=/some/dir cargo test ...
    if let Ok(dir) = std::env::var("PRINTPDF_SVG_TEST_OUT") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("svg_structural_audit.pdf"),
            &bytes,
        );
    }

    bytes
}

fn resolve<'a>(doc: &'a Document, mut obj: &'a Object) -> Result<&'a Object, String> {
    for _ in 0..16 {
        match obj {
            Object::Reference(id) => {
                obj = doc
                    .get_object(*id)
                    .map_err(|e| format!("dangling reference {id:?}: {e}"))?;
            }
            other => return Ok(other),
        }
    }
    Err("reference chain too deep".to_string())
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Result<&'a Dictionary, String> {
    match resolve(doc, obj)? {
        Object::Dictionary(d) => Ok(d),
        Object::Stream(s) => Ok(&s.dict),
        other => Err(format!("expected dictionary, got {other:?}")),
    }
}

/// Names referenced by resource-consuming operators of one content stream.
#[derive(Default, Debug)]
struct UsedNames {
    xobjects: Vec<String>,
    ext_g_states: Vec<String>,
    fonts: Vec<String>,
    color_spaces: Vec<String>,
    patterns: Vec<String>,
    shadings: Vec<String>,
}

fn scan_content(content: &[u8]) -> Result<UsedNames, String> {
    let decoded = lopdf::content::Content::decode(content)
        .map_err(|e| format!("content stream decode failed: {e}"))?;

    let mut used = UsedNames::default();
    let name_of = |o: &Object| -> Option<String> {
        o.as_name()
            .ok()
            .map(|n| String::from_utf8_lossy(n).to_string())
    };

    for op in &decoded.operations {
        match op.operator.as_str() {
            "Do" => {
                if let Some(n) = op.operands.first().and_then(|o| name_of(o)) {
                    used.xobjects.push(n);
                }
            }
            "gs" => {
                if let Some(n) = op.operands.first().and_then(|o| name_of(o)) {
                    used.ext_g_states.push(n);
                }
            }
            "Tf" => {
                if let Some(n) = op.operands.first().and_then(|o| name_of(o)) {
                    used.fonts.push(n);
                }
            }
            "cs" | "CS" => {
                if let Some(n) = op.operands.first().and_then(|o| name_of(o)) {
                    // device spaces and /Pattern may be used without a resource entry
                    if !matches!(
                        n.as_str(),
                        "DeviceRGB" | "DeviceGray" | "DeviceCMYK" | "Pattern"
                    ) {
                        used.color_spaces.push(n);
                    }
                }
            }
            "scn" | "SCN" => {
                // a name operand selects a pattern from /Pattern
                for o in &op.operands {
                    if let Some(n) = name_of(o) {
                        used.patterns.push(n);
                    }
                }
            }
            "sh" => {
                if let Some(n) = op.operands.first().and_then(|o| name_of(o)) {
                    used.shadings.push(n);
                }
            }
            _ => {}
        }
    }
    Ok(used)
}

fn subdict_has_key(doc: &Document, resources: &Dictionary, sub: &[u8], key: &str) -> bool {
    resources
        .get(sub)
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok())
        .map(|d| d.has(key.as_bytes()))
        .unwrap_or(false)
}

/// Audits one content stream against its /Resources dictionary; records
/// every referenced-but-undefined name in `failures`.
fn audit_content_against_resources(
    doc: &Document,
    path: &str,
    content: &[u8],
    resources: Option<&Dictionary>,
    failures: &mut Vec<String>,
) {
    let used = match scan_content(content) {
        Ok(u) => u,
        Err(e) => {
            failures.push(format!("{path}: {e}"));
            return;
        }
    };

    let empty = Dictionary::new();
    let res = resources.unwrap_or(&empty);

    let checks: &[(&str, &[u8], &Vec<String>)] = &[
        ("XObject", b"XObject", &used.xobjects),
        ("ExtGState", b"ExtGState", &used.ext_g_states),
        ("Font", b"Font", &used.fonts),
        ("ColorSpace", b"ColorSpace", &used.color_spaces),
        ("Pattern", b"Pattern", &used.patterns),
        ("Shading", b"Shading", &used.shadings),
    ];

    for (label, sub, names) in checks {
        for name in names.iter() {
            if !subdict_has_key(doc, res, sub, name) {
                failures.push(format!(
                    "{path}: content references /{name} ({label}) but /Resources/{label} has no \
                     such entry"
                ));
            }
        }
    }
}

/// Recursively audits a content-bearing object (form XObject or tiling
/// pattern): its own stream against its own resources, then every nested
/// form XObject, tiling pattern and soft-mask group.
fn audit_form_recursive(
    doc: &Document,
    path: &str,
    stream: &lopdf::Stream,
    visited: &mut HashSet<ObjectId>,
    failures: &mut Vec<String>,
    depth: usize,
) {
    if depth > 16 {
        failures.push(format!("{path}: nesting deeper than 16 levels"));
        return;
    }

    let content = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());

    let resources = stream
        .dict
        .get(b"Resources")
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok());

    audit_content_against_resources(doc, path, &content, resources, failures);

    let Some(res) = resources else { return };

    // nested form XObjects
    if let Some(xdict) = res
        .get(b"XObject")
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok())
    {
        for (name, val) in xdict.iter() {
            let child_path = format!("{path}/XObject/{}", String::from_utf8_lossy(name));
            audit_child_stream(doc, &child_path, val, visited, failures, depth);
        }
    }

    // tiling patterns (PatternType 1) have their own content + resources
    if let Some(pdict) = res
        .get(b"Pattern")
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok())
    {
        for (name, val) in pdict.iter() {
            let child_path = format!("{path}/Pattern/{}", String::from_utf8_lossy(name));
            audit_child_stream(doc, &child_path, val, visited, failures, depth);
        }
    }

    // soft masks: /ExtGState -> /SMask -> /G is a form XObject
    if let Some(gdict) = res
        .get(b"ExtGState")
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok())
    {
        for (name, val) in gdict.iter() {
            let Ok(gs) = resolve_dict(doc, val) else {
                failures.push(format!(
                    "{path}: /ExtGState/{} is not a dictionary",
                    String::from_utf8_lossy(name)
                ));
                continue;
            };
            if let Ok(smask) = gs.get(b"SMask") {
                if let Ok(smask_dict) = resolve_dict(doc, smask) {
                    if let Ok(g) = smask_dict.get(b"G") {
                        let child_path =
                            format!("{path}/ExtGState/{}/SMask/G", String::from_utf8_lossy(name));
                        audit_child_stream(doc, &child_path, g, visited, failures, depth);
                    }
                }
            }
        }
    }
}

fn audit_child_stream(
    doc: &Document,
    path: &str,
    obj: &Object,
    visited: &mut HashSet<ObjectId>,
    failures: &mut Vec<String>,
    depth: usize,
) {
    if let Object::Reference(id) = obj {
        if !visited.insert(*id) {
            return; // already audited
        }
    }
    let resolved = match resolve(doc, obj) {
        Ok(o) => o,
        Err(e) => {
            failures.push(format!("{path}: {e}"));
            return;
        }
    };
    if let Object::Stream(s) = resolved {
        let subtype = s.dict.get(b"Subtype").and_then(|o| o.as_name()).ok();
        let is_form = subtype == Some(b"Form");
        let is_tiling_pattern = s
            .dict
            .get(b"PatternType")
            .and_then(|o| o.as_i64())
            .map(|v| v == 1)
            .unwrap_or(false);
        if is_form || is_tiling_pattern {
            audit_form_recursive(doc, path, s, visited, failures, depth + 1);
        }
    }
}

/// Collects the SVG-derived Form XObjects of the first page.
fn svg_form_xobjects(doc: &Document) -> Vec<(String, ObjectId)> {
    let mut out = Vec::new();
    let page_id = doc.page_iter().next().expect("no page");
    let (resources, res_ids) = doc.get_page_resources(page_id).expect("page resources");
    let resources: &Dictionary = match resources {
        Some(r) => r,
        None => doc
            .get_dictionary(res_ids[0])
            .expect("page resource dict by id"),
    };
    if let Some(xobjs) = resources
        .get(b"XObject")
        .ok()
        .and_then(|o| resolve_dict(doc, o).ok())
    {
        for (name, val) in xobjs.iter() {
            if let Ok(Object::Stream(s)) = resolve(doc, val) {
                if s.dict.get(b"Subtype").and_then(|o| o.as_name()).ok() == Some(b"Form") {
                    if let Object::Reference(id) = val {
                        out.push((String::from_utf8_lossy(name).to_string(), *id));
                    }
                }
            }
        }
    }
    out
}

/// Walks the whole object tree hanging off one object; every reference must
/// resolve inside the document (issue #113: no references into nowhere).
fn assert_no_dangling_refs(
    doc: &Document,
    path: &str,
    obj: &Object,
    seen: &mut HashSet<ObjectId>,
    failures: &mut Vec<String>,
) {
    match obj {
        Object::Reference(id) => {
            if !seen.insert(*id) {
                return;
            }
            match doc.get_object(*id) {
                Ok(o) => assert_no_dangling_refs(doc, path, o, seen, failures),
                Err(e) => failures.push(format!("{path}: dangling reference {id:?}: {e}")),
            }
        }
        Object::Array(items) => {
            for it in items {
                assert_no_dangling_refs(doc, path, it, seen, failures);
            }
        }
        Object::Dictionary(d) => {
            for (_, v) in d.iter() {
                assert_no_dangling_refs(doc, path, v, seen, failures);
            }
        }
        Object::Stream(s) => {
            for (_, v) in s.dict.iter() {
                assert_no_dangling_refs(doc, path, v, seen, failures);
            }
        }
        _ => {}
    }
}

#[test]
fn svg_dropped_text_is_reported_loudly() {
    // Issue #184's worst failure mode was *silent*: a <text> whose
    // font-family resolves to nothing simply vanished. It must produce a
    // warning now.
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40">
        <text x="0" y="20" font-family="No Such Font Family 987" font-size="12">hi</text>
    </svg>"#;
    let mut warnings = Vec::new();
    let _ = Svg::parse(svg, &mut warnings).expect("svg parses");
    assert!(
        warnings.iter().any(|w| w.msg.contains("<text>")),
        "dropping a <text> element must push a warning, got: {warnings:#?}"
    );
}

#[test]
fn svg_form_xobjects_are_self_contained() {
    let bytes = build_test_pdf();
    let doc = Document::load_mem(&bytes).expect("reload saved pdf");

    let forms = svg_form_xobjects(&doc);
    assert!(
        !forms.is_empty(),
        "no SVG Form XObjects found on the page — test setup is broken"
    );

    let mut failures = Vec::new();
    for (name, id) in &forms {
        let stream = doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_stream().ok())
            .expect("form xobject is a stream");
        let mut visited = HashSet::new();
        visited.insert(*id);
        audit_form_recursive(&doc, &format!("/XObject/{name}"), stream, &mut visited, &mut failures, 0);
    }

    assert!(
        failures.is_empty(),
        "SVG XObjects reference resources that are not defined (issue #113):\n{}",
        failures.join("\n")
    );
}

#[test]
fn svg_page_content_resources_exist() {
    let bytes = build_test_pdf();
    let doc = Document::load_mem(&bytes).expect("reload saved pdf");
    let page_id = doc.page_iter().next().expect("no page");
    let content = doc.get_page_content(page_id);
    let (resources, res_ids) = doc.get_page_resources(page_id).expect("page resources");
    let resources = match resources {
        Some(r) => Some(r),
        None => res_ids.first().and_then(|id| doc.get_dictionary(*id).ok()),
    };

    let mut failures = Vec::new();
    audit_content_against_resources(&doc, "page[0]", &content, resources, &mut failures);
    assert!(
        failures.is_empty(),
        "page content references undefined resources:\n{}",
        failures.join("\n")
    );
}

#[test]
fn svg_colorspace_resource_entries_are_dictionaries() {
    // Issue #211: /Resources /ColorSpace must be a *dictionary* mapping
    // resource names to color spaces — a bare name here breaks Acrobat.
    let bytes = build_test_pdf();
    let doc = Document::load_mem(&bytes).expect("reload saved pdf");

    let mut failures = Vec::new();
    for (name, id) in svg_form_xobjects(&doc) {
        let stream = doc
            .get_object(id)
            .ok()
            .and_then(|o| o.as_stream().ok())
            .expect("form xobject is a stream");
        let Some(res) = stream
            .dict
            .get(b"Resources")
            .ok()
            .and_then(|o| resolve_dict(&doc, o).ok())
        else {
            failures.push(format!("/XObject/{name}: missing /Resources dictionary"));
            continue;
        };
        if let Ok(cs) = res.get(b"ColorSpace") {
            if resolve_dict(&doc, cs).is_err() {
                failures.push(format!(
                    "/XObject/{name}: /Resources/ColorSpace is not a dictionary (found {cs:?}) — \
                     issue #211"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn svg_no_dangling_references_in_xobject_subtrees() {
    let bytes = build_test_pdf();
    let doc = Document::load_mem(&bytes).expect("reload saved pdf");

    let mut failures = Vec::new();
    for (name, id) in svg_form_xobjects(&doc) {
        let mut seen = HashSet::new();
        assert_no_dangling_refs(
            &doc,
            &format!("/XObject/{name}"),
            &Object::Reference(id),
            &mut seen,
            &mut failures,
        );
    }
    assert!(
        failures.is_empty(),
        "dangling references inside SVG XObject subtrees (issue #113):\n{}",
        failures.join("\n")
    );
}

#[test]
fn svg_gradients_survive_as_pattern_or_shading_paint() {
    // Before the fix, `/p0 scn` (pattern paint) was re-interpreted as a
    // grayscale color (`0 g`) and gradients rendered as flat black.
    let mut warnings = Vec::new();
    let parsed = Svg::parse(GRADIENT_SVG, &mut warnings).expect("parse gradient svg");

    let bytes = {
        let mut doc = PdfDocument::new("gradient-check");
        let id = doc.add_xobject(&parsed);
        let page = PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![Op::UseXobject {
                id,
                transform: XObjectTransform::default(),
            }],
        );
        doc.with_pages(vec![page])
            .save(&PdfSaveOptions::default(), &mut warnings)
    };

    if let Ok(dir) = std::env::var("PRINTPDF_SVG_TEST_OUT") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("svg_gradient_check.pdf"),
            &bytes,
        );
    }

    let doc = Document::load_mem(&bytes).expect("reload saved pdf");

    // union of all content streams in the SVG xobject subtree
    let mut pattern_paint_found = false;
    let mut stack: Vec<ObjectId> = svg_form_xobjects(&doc).into_iter().map(|(_, id)| id).collect();
    assert!(!stack.is_empty(), "no SVG form xobject found");
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Ok(Object::Stream(s)) = doc.get_object(id) else {
            continue;
        };
        let content = s
            .decompressed_content()
            .unwrap_or_else(|_| s.content.clone());
        if let Ok(decoded) = lopdf::content::Content::decode(&content) {
            for op in &decoded.operations {
                match op.operator.as_str() {
                    "scn" | "SCN" => {
                        if op.operands.iter().any(|o| o.as_name().is_ok()) {
                            pattern_paint_found = true;
                        }
                    }
                    "sh" => pattern_paint_found = true,
                    _ => {}
                }
            }
        }
        // descend into nested xobjects
        if let Some(xdict) = s
            .dict
            .get(b"Resources")
            .ok()
            .and_then(|o| resolve_dict(&doc, o).ok())
            .and_then(|res| res.get(b"XObject").ok())
            .and_then(|o| resolve_dict(&doc, o).ok())
        {
            for (_, v) in xdict.iter() {
                if let Object::Reference(rid) = v {
                    stack.push(*rid);
                }
            }
        }
    }

    assert!(
        pattern_paint_found,
        "no pattern/shading paint operator survived in the gradient SVG — gradients were \
         flattened (issue #113 regression)"
    );
}
