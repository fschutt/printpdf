//! HTML CSS gradients -> PDF shadings example.
//!
//! Renders divs with `background: linear-gradient(...)` / `radial-gradient(...)`
//! to a PDF, proving the gradients become PDF axial (ShadingType 2) / radial
//! (ShadingType 3) shadings painted with the `sh` operator.
//!
//! Run with:
//!     cargo run --example html_gradient --features html
//!
//! Output: `html_gradient.pdf` in the current directory.

extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("Rendering HTML with CSS gradients to PDF...");

    let html = r#"
    <html>
        <head><style>
            .title { font-size: 20px; color: #222222; margin-bottom: 10px; }
            .box { width: 400px; height: 120px; margin-bottom: 16px; }
            .lin { background: linear-gradient(to right, #ff0000, #0000ff); }
            .multi { background: linear-gradient(90deg, #ff0000 0%, #ffff00 50%, #0000ff 100%); }
            .rad { background: radial-gradient(circle, #00ff00, #003300); }
        </style></head>
        <body>
            <div class="title">CSS gradients embedded as PDF shadings</div>
            <div class="box lin"></div>
            <div class="box multi"></div>
            <div class="box rad"></div>
        </body>
    </html>
    "#;

    let images: BTreeMap<String, Base64OrRaw> = BTreeMap::new();
    let fonts: BTreeMap<String, Base64OrRaw> = BTreeMap::new();
    let options = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(20.0),
        margin_bottom: Some(20.0),
        margin_left: Some(20.0),
        ..Default::default()
    };

    let mut warnings = Vec::new();
    let doc = match PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("[ERROR] from_html failed: {}", e);
            std::process::exit(1);
        }
    };

    // ---- Evidence #1: the document registered shadings (one per gradient). ----
    let shading_count = doc.resources.shadings.map.len();
    println!("[INFO] {} shading(s) registered", shading_count);
    for (id, sh) in doc.resources.shadings.map.iter() {
        let kind = match sh.geometry {
            ShadingGeometry::Axial { .. } => "axial (linear)",
            ShadingGeometry::Radial { .. } => "radial",
        };
        println!("[OK] shading {:?}: {} with {} stops", id.0, kind, sh.stops.len());
    }
    assert!(
        shading_count >= 1,
        "expected at least one gradient shading registered in resources"
    );

    // ---- Evidence #2: pages paint the shadings via Op::PaintShading. ----
    let paint_count: usize = doc
        .pages
        .iter()
        .flat_map(|p| p.ops.iter())
        .filter(|op| matches!(op, Op::PaintShading { .. }))
        .count();
    println!("[INFO] {} PaintShading op(s) across pages", paint_count);
    assert!(paint_count >= 1, "expected at least one Op::PaintShading in a page");

    // ---- Evidence #3: the serialized PDF contains shading dictionaries. ----
    // (`/ShadingType` and `/FunctionType` live in resource-object dictionaries,
    // which are not compressed, so a raw byte scan finds them — the same
    // technique the html_image example uses for `/Subtype/Image`.)
    let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
    std::fs::write("html_gradient.pdf", &pdf_bytes).expect("write html_gradient.pdf");
    println!("Wrote html_gradient.pdf ({} bytes)", pdf_bytes.len());

    let contains = |needle: &[u8]| pdf_bytes.windows(needle.len()).any(|w| w == needle);
    assert!(contains(b"/ShadingType"), "serialized PDF must contain a /ShadingType");
    assert!(contains(b"/FunctionType"), "serialized PDF must contain a /FunctionType");
    println!("[OK] Serialized PDF contains /ShadingType + /FunctionType");

    println!("\nSUCCESS: CSS gradients were embedded as PDF shadings.");
}
