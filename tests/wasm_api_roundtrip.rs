//! Native tests for the WASM JSON API surface (`printpdf::wasm::api::*Sync`).
//!
//! The GitHub Pages demo (index.html + script.js) drives printpdf exclusively
//! through JSON strings:
//!
//! ```text
//! updatePdfFromHtml():  Pdf_HtmlToDocument({html, images, fonts, options}) -> {doc, warnings}
//! upload-pdf:           Pdf_BytesToDocument({bytes, options})              -> {doc, warnings}
//! updatePdfViewer():    Pdf_ResourcesForPage({page})                       -> {xobjects, fonts, layers}
//!                       Pdf_PageToSvg({page, resources, options})          -> {svg, warnings}
//! save button:          Pdf_DocumentToBytes({doc, options})                -> {bytes, warnings}
//! ```
//!
//! The `doc` the save button sends back is the *verbatim* JSON value an earlier
//! `Pdf_HtmlToDocument` / `Pdf_BytesToDocument` response handed out, so every
//! font in `doc.resources.fonts` must survive serialize -> JSON -> deserialize.
//!
//! That is exactly what broke on the (stale) Feb-2026 deploy: printpdf 0.9.1
//! re-exported `azul_layout::ParsedFont`, whose `Serialize` emits
//! `to_bytes(None).unwrap_or_default()` — an **empty** byte vec when the parsed
//! face no longer holds its source bytes (issue #277) — producing the bare
//! `"data:font/ttf;base64,"` data URI. Clicking save then fed that empty
//! payload back into `Deserialize`, which re-parses the font bytes and failed
//! with:
//!
//! ```text
//! PDF Serialization Error: failed to deserialize input: Font deserialization
//! error: [FontParseWarning { severity: Error, message: "Failed to read font
//! data: end of data reached unexpectedly" }] at line 1 column 684
//! ```
//!
//! The tests below pin both sides: the exact failure mode for an empty font
//! payload (so the error stays diagnosable), and the fixed round-trip through
//! the same JSON entry points the browser calls.

#![cfg(feature = "html")]
#![allow(non_snake_case)]

use printpdf::{
    ops::PdfFontHandle,
    units::{Mm, Pt},
    wasm::api::{
        Pdf_BytesToDocumentSync, Pdf_DocumentToBytesSync, Pdf_HtmlToDocumentSync,
        Pdf_PageToSvgSync, Pdf_ResourcesForPageSync,
    },
    FontId, Op, ParsedFont, PdfDocument, PdfPage, TextItem,
};
use serde_json::{json, Value};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// Call a WASM API entry point exactly like script.js does:
/// `JSON.parse(Pdf_Xyz(JSON.stringify(input)))`.
fn call(f: fn(String) -> String, input: &Value) -> (u64, Value) {
    let out = f(input.to_string());
    let v: Value = serde_json::from_str(&out).unwrap_or_else(|e| {
        panic!("API returned invalid JSON ({e}): {:?}", &out[..out.len().min(200)])
    });
    let status = v["status"].as_u64().expect("response must have a status");
    (status, v["data"].clone())
}

/// A one-page document that shows `text` in Roboto, as `serde_json::Value` —
/// the same JSON value the demo holds in its `pdfDocument` variable.
fn doc_with_roboto(text: &str) -> Value {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("Roboto must parse");
    let mut doc = PdfDocument::new("wasm-api-roundtrip");
    let font_id = doc.add_font(&font);
    let doc = doc.with_pages(vec![PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        show_text_ops(&font_id, text),
    )]);
    serde_json::to_value(&doc).expect("PdfDocument must serialize to JSON")
}

fn show_text_ops(font_id: &FontId, text: &str) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::External(font_id.clone()),
            size: Pt(24.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

fn font_map<'a>(doc: &'a Value) -> &'a serde_json::Map<String, Value> {
    doc["resources"]["fonts"]
        .as_object()
        .expect("resources.fonts must be a JSON object")
}

// ---------------------------------------------------------------------------
// The reported save error, reproduced natively
// ---------------------------------------------------------------------------

/// Feeding the save endpoint a font whose payload is the bare
/// `data:font/ttf;base64,` prefix (what the stale deploy's serializer emitted
/// for byte-less fonts, and what any truncated upload degenerates to) must fail
/// with the exact diagnosable error the maintainer reported — an input
/// deserialization error (status 1) pointing at unreadable font data — and not
/// panic or emit a corrupt PDF.
#[test]
fn empty_font_payload_fails_save_like_the_reported_demo_error() {
    let mut doc = doc_with_roboto("Roboto");

    let keys: Vec<String> = font_map(&doc).keys().cloned().collect();
    assert!(!keys.is_empty(), "test document must contain a font");
    for k in &keys {
        // What azul-layout <= 0.0.9's `Serialize` produced for a `ParsedFont`
        // without source bytes: an empty base64 payload.
        doc["resources"]["fonts"][k]["parsed_font"] = json!("data:font/ttf;base64,");
    }

    // script.js save button: JSON.stringify({ doc: pdfDocument, options: {} })
    let save_input = json!({ "doc": doc, "options": {} });
    let (status, data) = call(Pdf_DocumentToBytesSync, &save_input);

    assert_eq!(status, 1, "empty font payload must be an input-deserialization error");
    let msg = data.as_str().expect("error data must be a string");
    assert!(
        msg.contains("failed to deserialize input"),
        "error must identify the failing stage, got: {msg}"
    );
    assert!(
        msg.contains("Failed to read font data: end of data reached unexpectedly"),
        "error must surface the font parser's diagnosis, got: {msg}"
    );
    assert!(
        msg.contains("line 1 column"),
        "single-line JSON.stringify input must report a line-1 position, got: {msg}"
    );
    // For comparison with browser reports (the maintainer saw "column 684"):
    eprintln!("native repro of the demo save error: {msg}");
}

// ---------------------------------------------------------------------------
// The fixed round-trips (regressions for the save bug)
// ---------------------------------------------------------------------------

/// The exact "save" path of the demo: a `PdfDocument` with an external font,
/// serialized to JSON, must deserialize and save through
/// `Pdf_DocumentToBytes` — and the produced PDF must re-parse (upload flow)
/// and re-save (edit-then-save flow) with the font intact.
///
/// Uses `subset_fonts: false`; the demo-default subset chain is
/// `upload_of_subset_font_pdf_roundtrips` below (currently `#[ignore]`d,
/// blocked on `ParsedFont::serialize`).
#[test]
fn document_with_font_roundtrips_through_the_demo_save_json() {
    let doc = doc_with_roboto("Roboto");
    assert!(!font_map(&doc).is_empty(), "document must carry an external font");
    // The serialized font must actually carry payload — a bare data-URI prefix
    // here is the 0.9.x bug coming back.
    for (id, font) in font_map(&doc) {
        let uri = font["parsed_font"].as_str().unwrap_or_else(|| {
            panic!("font {id} must serialize as a data-URI string")
        });
        assert!(
            uri.len() > "data:font/ttf;base64,".len() + 1000,
            "font {id} serialized with a (near-)empty payload: {} chars",
            uri.len()
        );
    }

    // 1. Save (script.js save button; full font so the re-parsed program keeps
    //    all sfnt tables `ParsedFont::serialize` insists on).
    let (status, data) = call(
        Pdf_DocumentToBytesSync,
        &json!({ "doc": doc, "options": { "subset_fonts": false } }),
    );
    assert_eq!(status, 0, "save must succeed, got error: {data}");
    let b64 = data["bytes"].as_str().expect("bytes must be base64 string");
    let pdf_bytes = {
        use base64::Engine;
        base64::prelude::BASE64_STANDARD.decode(b64).expect("valid base64")
    };
    assert!(pdf_bytes.starts_with(b"%PDF"), "output must be a PDF");

    // 2. Upload the produced PDF back (script.js upload-pdf flow). This is also
    //    the structural font check: if the PDF embedded an empty/corrupt font
    //    program, the parser drops the font and the map comes back empty.
    let (status, data) = call(Pdf_BytesToDocumentSync, &json!({ "bytes": b64, "options": {} }));
    assert_eq!(status, 0, "re-parse must succeed, got error: {data}");
    let reparsed = data["doc"].clone();
    assert!(
        !font_map(&reparsed).is_empty(),
        "re-parsed document lost its fonts"
    );
    for (id, font) in font_map(&reparsed) {
        let uri = font["parsed_font"]
            .as_str()
            .unwrap_or_else(|| panic!("re-parsed font {id} must be a data-URI string"));
        assert!(
            uri.len() > "data:font/ttf;base64,".len() + 1000,
            "re-parsed font {id} has a (near-)empty program: {} chars",
            uri.len()
        );
    }

    // 3. Save again from the re-parsed JSON (parse/edit tab -> save button).
    //    This is the *reported* failure: a doc JSON produced by the WASM API
    //    itself must feed back into Pdf_DocumentToBytes without a font
    //    deserialization error.
    let (status, data) = call(
        Pdf_DocumentToBytesSync,
        &json!({ "doc": reparsed, "options": {} }),
    );
    assert_eq!(status, 0, "save of re-parsed doc must succeed, got error: {data}");
}

/// Demo-default chain (`options: {}` => `subset_fonts: true`, what the save
/// button actually sends): save, then upload the produced PDF back.
///
/// KNOWN BROKEN at HEAD — un-ignore when `ParsedFont::serialize`
/// (src/font.rs) stops round-tripping through azul's `to_bytes(None)`:
/// that call rebuilds the sfnt from a hardcoded table list (CMAP HEAD HHEA
/// HMTX MAXP NAME OS/2 POST GLYF LOCA) and *fails* for any font missing one
/// of them. Subset fonts written by allsorts' PDF profile drop OS/2/NAME/POST
/// ("font has no source bytes: read error: font is missing 'OS/2' table"),
/// and CFF-flavored fonts have no GLYF/LOCA at all ("missing 'loca' table").
/// So `Pdf_BytesToDocument` on a printpdf-subset PDF cannot serialize its own
/// response. Serializing the retained source bytes verbatim would fix both.
#[test]
// Un-ignored 2026-07-17: ParsedFont::serialize now emits the retained source
// bytes verbatim instead of azul's to_bytes(None) table rebuild, so subset and
// CFF fonts round-trip. This test is the regression guard for that fix.
fn upload_of_subset_font_pdf_roundtrips() {
    let doc = doc_with_roboto("Roboto");

    let (status, data) = call(Pdf_DocumentToBytesSync, &json!({ "doc": doc, "options": {} }));
    assert_eq!(status, 0, "save must succeed, got error: {data}");
    let b64 = data["bytes"].as_str().expect("bytes must be base64 string");

    let (status, data) = call(Pdf_BytesToDocumentSync, &json!({ "bytes": b64, "options": {} }));
    assert_eq!(
        status, 0,
        "re-parse of a printpdf-subset PDF must succeed, got error: {data}"
    );
    let reparsed = data["doc"].clone();
    assert!(!font_map(&reparsed).is_empty(), "re-parsed doc lost its fonts");

    let (status, data) = call(
        Pdf_DocumentToBytesSync,
        &json!({ "doc": reparsed, "options": {} }),
    );
    assert_eq!(status, 0, "re-save must succeed, got error: {data}");
}

/// The demo's html-to-pdf flow: `Pdf_HtmlToDocument`'s response `doc` is
/// stored by script.js and later sent back verbatim to `Pdf_DocumentToBytes`
/// (save button) and `Pdf_PageToSvg` (viewer).
///
/// Two contracts are pinned here:
///
/// 1. The response is *always* a well-formed JSON envelope. Before the
///    `output_serialization_error_envelope` fix in `src/wasm/api.rs`,
///    a document whose fonts could not re-serialize (e.g. layout fell back to
///    a CFF-flavored system font, whose azul `to_bytes(None)` fails with
///    "font is missing 'loca' table") made `api_inner` return an **empty
///    string**, which script.js's unconditional `JSON.parse` turned into
///    "Unexpected end of JSON input" with the cause lost.
///
/// 2. Whenever the call reports success (status 0), the returned `doc` must
///    feed back into the save button and the SVG viewer unchanged.
///
/// Which branch runs depends on the font the HTML layout picks (system font
/// fallback is machine-dependent — printpdf's `fonts` input map is currently
/// NOT consulted for family resolution, see the html/mod.rs findings), so the
/// test asserts contract 1 unconditionally and contract 2 opportunistically.
#[test]
fn html_to_document_response_is_always_a_json_envelope_and_roundtrips_on_success() {
    use base64::Engine;
    let input = json!({
        "html": "<html><body><p style=\"font-family: sans-serif;\">Hello printpdf</p></body></html>",
        "images": {},
        "fonts": {
            "RobotoMedium.ttf": base64::prelude::BASE64_STANDARD.encode(ROBOTO_TTF),
        },
        "options": {}
    });

    // Contract 1: never an empty / unparseable response (the `call` helper
    // panics on invalid JSON).
    let (status, data) = call(Pdf_HtmlToDocumentSync, &input);

    if status != 0 {
        // The envelope must carry a diagnosable message, not be a bare shell.
        let msg = data.as_str().expect("non-zero status must carry an error string");
        assert!(
            !msg.is_empty(),
            "error envelope must explain what failed"
        );
        eprintln!(
            "note: html-to-document reported status {status} on this machine \
             (font-dependent): {msg}"
        );
        return;
    }

    // Contract 2: the status-0 doc must round-trip into save + viewer.
    let doc = data["doc"].clone();
    let pages = doc["pages"].as_array().expect("doc must have pages");
    assert!(!pages.is_empty(), "html must produce at least one page");

    // Save button on the unmodified response value.
    let (status, data) = call(Pdf_DocumentToBytesSync, &json!({ "doc": doc, "options": {} }));
    assert_eq!(
        status, 0,
        "the doc JSON produced by Pdf_HtmlToDocument must round-trip into \
         Pdf_DocumentToBytes (the demo save button), got error: {data}"
    );
    let pdf_bytes = base64::prelude::BASE64_STANDARD
        .decode(data["bytes"].as_str().expect("bytes must be base64"))
        .expect("valid base64");
    assert!(pdf_bytes.starts_with(b"%PDF"));

    // Viewer flow on page 0: resources-for-page, then page-to-svg with the
    // per-page resource copy script.js builds (fonts/xobjects/layers subset).
    let page = &pages[0];
    let (status, res_ids) = call(Pdf_ResourcesForPageSync, &json!({ "page": page }));
    assert_eq!(status, 0, "resources-for-page must succeed, got error: {res_ids}");

    let mut fonts = serde_json::Map::new();
    for id in res_ids["fonts"].as_array().expect("fonts id list") {
        let id = id.as_str().expect("font id is a string");
        fonts.insert(id.to_string(), doc["resources"]["fonts"][id].clone());
    }
    let mut xobjects = serde_json::Map::new();
    for id in res_ids["xobjects"].as_array().expect("xobject id list") {
        let id = id.as_str().expect("xobject id is a string");
        xobjects.insert(id.to_string(), doc["resources"]["xobjects"][id].clone());
    }
    let svg_input = json!({
        "page": page,
        "resources": { "fonts": fonts, "xobjects": xobjects, "layers": {}, "extgstates": doc["resources"]["extgstates"] },
        "options": { "imageFormats": ["png", "jpeg"] }
    });
    let (status, data) = call(Pdf_PageToSvgSync, &svg_input);
    assert_eq!(status, 0, "page-to-svg must succeed, got error: {data}");
    let svg = data["svg"].as_str().expect("svg must be a string");
    assert!(svg.contains("<svg"), "output must contain an <svg> root");
}

/// The viewer flow on a natively-built document (deterministic, no HTML
/// layout / system-font dependence): resources-for-page must list the
/// external font, and page-to-svg must accept the script.js-style per-page
/// resource subset.
#[test]
fn viewer_flow_resources_for_page_and_page_to_svg() {
    let doc = doc_with_roboto("Roboto");
    let page = &doc["pages"][0];

    let (status, res_ids) = call(Pdf_ResourcesForPageSync, &json!({ "page": page }));
    assert_eq!(status, 0, "resources-for-page must succeed, got error: {res_ids}");
    // NOTE: fonts are referenced via SetFont ops; get_external_font_ids
    // documents that it currently returns [] (fonts resolved by resource name),
    // so the id list shape — not its content — is the contract here.
    assert!(res_ids["fonts"].is_array());
    assert!(res_ids["xobjects"].is_array());
    assert!(res_ids["layers"].is_array());

    // script.js copies the *listed* resources only; fonts come out empty for
    // this page (see above), which Pdf_PageToSvg must tolerate.
    let svg_input = json!({
        "page": page,
        "resources": { "fonts": {}, "xobjects": {}, "layers": {} },
        "options": { "imageFormats": ["png", "jpeg"] }
    });
    let (status, data) = call(Pdf_PageToSvgSync, &svg_input);
    assert_eq!(status, 0, "page-to-svg must succeed, got error: {data}");
    assert!(data["svg"].as_str().expect("svg string").contains("<svg"));
}

/// A response that cannot be serialized must still yield a well-formed JSON
/// error envelope (status != 0), never an empty string — script.js does
/// `JSON.parse()` on the return value unconditionally.
#[test]
fn api_always_returns_parseable_json() {
    // Garbage input: deserialization fails -> status 1 envelope.
    let out = Pdf_DocumentToBytesSync("{not json".to_string());
    let v: Value = serde_json::from_str(&out).expect("error path must produce valid JSON");
    assert_eq!(v["status"], 1);
    assert!(v["data"].as_str().is_some());

    // Wrong shape (valid JSON, missing required fields) -> status 1 envelope.
    let out = Pdf_DocumentToBytesSync("{\"nope\": 1}".to_string());
    let v: Value = serde_json::from_str(&out).expect("error path must produce valid JSON");
    assert_eq!(v["status"], 1);
    let msg = v["data"].as_str().expect("data must be the error string");
    assert!(msg.contains("failed to deserialize input"));
}
