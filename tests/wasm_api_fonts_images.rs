#![cfg(all(feature = "html", feature = "images"))]

//! Native tests for the `Pdf_RegisterFonts` / `Pdf_DecodeImage` wasm API
//! endpoints (the sync twins run natively; the async wasm exports are thin
//! wrappers over the same inner functions).

use printpdf::wasm::api::{Pdf_DecodeImageSync, Pdf_HtmlToDocumentSync, Pdf_RegisterFontsSync};
use serde_json::{json, Value};

fn call(f: fn(String) -> String, input: Value) -> Value {
    serde_json::from_str(&f(input.to_string())).expect("api output must be JSON")
}

#[test]
fn registered_fonts_participate_in_every_render() {
    let font_b64 = {
        use base64::Engine;
        base64::prelude::BASE64_STANDARD
            .encode(std::fs::read("examples/assets/fonts/RobotoMedium.ttf").unwrap())
    };

    let r = call(
        Pdf_RegisterFontsSync,
        json!({ "fonts": { "Roboto Medium": font_b64 }, "replace": true }),
    );
    assert_eq!(r["status"], 0, "register: {r}");
    assert_eq!(r["data"]["registered"], 1);

    // No fonts in the call itself — the registered one must still resolve.
    let r = call(
        Pdf_HtmlToDocumentSync,
        json!({
            "html": "<html><body><p style='font-family: \"Roboto Medium\";'>Registered</p></body></html>"
        }),
    );
    assert_eq!(r["status"], 0, "render: {r}");
    let fonts = &r["data"]["doc"]["resources"]["fonts"];
    assert!(
        fonts.as_object().map(|m| !m.is_empty()).unwrap_or(false),
        "the registered font must be embedded in the rendered doc, got: {fonts}"
    );

    // Cleanup so other tests see an empty registry.
    let r = call(Pdf_RegisterFontsSync, json!({ "replace": true }));
    assert_eq!(r["data"]["total"], 0);
}

#[test]
fn decode_image_returns_rawimage_json() {
    let png_b64 = {
        use base64::Engine;
        base64::prelude::BASE64_STANDARD
            .encode(std::fs::read("examples/assets/img/dog_alpha.png").unwrap())
    };
    let r = call(Pdf_DecodeImageSync, json!({ "bytes": png_b64 }));
    assert_eq!(r["status"], 0, "{r}");
    let img = &r["data"]["image"];
    assert!(img["width"].as_u64().unwrap_or(0) > 0, "width: {img}");
    assert!(img["height"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn decode_image_rejects_garbage_with_envelope() {
    let r = call(Pdf_DecodeImageSync, json!({ "bytes": "AAAA" }));
    assert_eq!(r["status"], 2, "{r}");
}
