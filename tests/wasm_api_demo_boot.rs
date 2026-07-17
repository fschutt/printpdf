#![cfg(feature = "html")]

//! Native replication of the demo's exact boot + first-render sequence:
//! register all 8 default fonts, then render the invoice example. The browser
//! run of this sequence hit `RuntimeError: unreachable` (a wasm panic).

use printpdf::wasm::api::{Pdf_HtmlToDocumentSync, Pdf_RegisterFontsSync};
use serde_json::{json, Value};

#[test]
fn demo_boot_sequence_with_all_default_fonts() {
    use base64::Engine;
    let mut fonts = serde_json::Map::new();
    for f in [
        "Helvetica.ttf", "Helvetica-Bold.ttf", "Helvetica-Oblique.ttf",
        "Helvetica-BoldOblique.ttf", "Times.ttf", "Courier.ttf",
        "RobotoMedium.ttf", "NotoSansJP-Regular.otf",
    ] {
        let bytes = std::fs::read(format!("examples/assets/fonts/{f}")).unwrap();
        fonts.insert(f.to_string(), Value::String(
            base64::prelude::BASE64_STANDARD.encode(bytes)));
    }
    let r: Value = serde_json::from_str(&Pdf_RegisterFontsSync(
        json!({ "fonts": fonts, "replace": true }).to_string())).unwrap();
    assert_eq!(r["status"], 0, "register: {r}");

    let html = r#"<html><head><style>
        body { font-family: Helvetica, sans-serif; color: #222; }
        h1 { color: #b7410e; }
        table { width: 100%; border-collapse: collapse; }
        th { border-bottom: 2px solid #b7410e; }
    </style></head><body>
        <h1>INVOICE #2026-071</h1>
        <table><tr><th>Description</th><th>Amount</th></tr>
        <tr><td>PDF generation consulting</td><td>€1,440</td></tr></table>
    </body></html>"#;

    let r: Value = serde_json::from_str(&Pdf_HtmlToDocumentSync(
        json!({ "html": html }).to_string())).unwrap();
    assert_eq!(r["status"], 0, "render: {r}");
    assert!(r["data"]["doc"]["pages"].as_array().map(|p| !p.is_empty()).unwrap_or(false));

    // leave the registry clean
    let _ = Pdf_RegisterFontsSync(json!({ "replace": true }).to_string());
}
