use std::collections::BTreeMap;
use printpdf::{GeneratePdfOptions, PdfDocument};

#[test]
fn debug_html_bounds() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <p style="width: 800px;">Hello World</p>
    </body>
    </html>
    "#;

    let images = BTreeMap::default();
    let fonts = BTreeMap::default();
    let options = GeneratePdfOptions {
        page_height: Some(210.0),
        page_width: Some(297.0),
        font_embedding: Some(true),
        image_optimization: Some(printpdf::ImageOptimizationOptions::default()),
    };

    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings).unwrap();

    println!("\n=== GENERATED PDF OPERATIONS ===");
    for (page_idx, page) in doc.pages.iter().enumerate() {
        println!("Page {}: {} operations", page_idx, page.ops.len());
        for (i, op) in page.ops.iter().enumerate() {
            println!("  [{}] {:?}", i, op);
        }
    }
    println!("================================\n");

    // Check that we have text operations
    assert!(!doc.pages.is_empty());
    assert!(!doc.pages[0].ops.is_empty(), "No operations generated!");
}
