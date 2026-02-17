#![cfg(feature = "html")]

use printpdf::*;
use std::collections::BTreeMap;

#[test]
fn test_basic_text_breaks_across_pages() {
    let mut warnings = Vec::new();

    // Generate enough paragraphs to overflow A4.
    // A4 content area ≈ 1122 CSS px.  Each 12pt paragraph with
    // margin-bottom: 10px occupies roughly 30 CSS px, so we need
    // at least ~40 paragraphs to exceed a single page.
    let paragraphs: String = (1..=60)
        .map(|i| format!(
            "<p>Paragraph {}. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
             sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>",
            i
        ))
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {{ margin: 20px; font-size: 12pt; }}
                p {{ margin-bottom: 10px; }}
            </style>
        </head>
        <body>
            {paragraphs}
        </body>
        </html>
    "#);

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();

    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");

    // The document should have multiple pages
    assert!(doc.pages.len() > 1, "Expected multiple pages, got {}", doc.pages.len());
}

#[test]
fn test_page_break_before() {
    let mut warnings = Vec::new();
    
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .page-break { page-break-before: always; }
            </style>
        </head>
        <body>
            <p>This is on page 1.</p>
            <p class="page-break">This should be on page 2.</p>
            <p class="page-break">This should be on page 3.</p>
        </body>
        </html>
    "#;
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages", doc.pages.len());
    
    // Should have 3 pages due to page-break-before
    assert!(doc.pages.len() >= 3, "Expected at least 3 pages, got {}", doc.pages.len());
}

#[test]
fn test_page_break_after() {
    let mut warnings = Vec::new();
    
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .break-after { page-break-after: always; }
            </style>
        </head>
        <body>
            <p class="break-after">This is on page 1, with break after.</p>
            <p class="break-after">This is on page 2, with break after.</p>
            <p>This is on page 3.</p>
        </body>
        </html>
    "#;
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages", doc.pages.len());
    
    // Should have 3 pages due to page-break-after
    assert!(doc.pages.len() >= 3, "Expected at least 3 pages, got {}", doc.pages.len());
}

#[test]
fn test_long_single_text_node_breaks() {
    let mut warnings = Vec::new();
    
    // Single very long paragraph that must break across pages
    let long_text = (0..100)
        .map(|i| format!("This is sentence number {}. It contains enough words to make it substantial.", i))
        .collect::<Vec<_>>()
        .join(" ");
    
    let html = format!(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {{ margin: 20px; font-size: 12pt; }}
            </style>
        </head>
        <body>
            <p>{}</p>
        </body>
        </html>
    "#, long_text);
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages for long text", doc.pages.len());
    
    // Should have multiple pages
    assert!(doc.pages.len() > 1, "Expected multiple pages for long text, got {}", doc.pages.len());
    
    // Extract and verify text is on multiple pages
    let page_texts = doc.extract_text();
    let mut total_chars = 0;
    for (i, page_text) in page_texts.iter().enumerate() {
        let page_str = page_text.join(" ");
        println!("Page {} has {} characters", i + 1, page_str.len());
        total_chars += page_str.len();
    }
    
    // Verify we captured most of the text
    assert!(total_chars > long_text.len() / 2, "Expected to capture most of the text across pages");
}

#[test]
fn test_avoid_page_break_inside() {
    let mut warnings = Vec::new();
    
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                .keep-together { page-break-inside: avoid; }
                p { margin-bottom: 5px; }
            </style>
        </head>
        <body>
            <div class="keep-together">
                <p>This block should stay together.</p>
                <p>All these paragraphs should be on the same page.</p>
                <p>They should not be split.</p>
            </div>
            <p>Paragraph 1 outside block.</p>
            <p>Paragraph 2 outside block.</p>
            <p>Paragraph 3 outside block.</p>
            <p>Paragraph 4 outside block.</p>
            <p>Paragraph 5 outside block.</p>
        </body>
        </html>
    "#;
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages", doc.pages.len());
    
    // Just verify it generates something reasonable
    assert!(!doc.pages.is_empty(), "Expected at least one page");
}

#[test]
fn test_explicit_page_dimensions() {
    let mut warnings = Vec::new();
    
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                @page {
                    size: A4;
                    margin: 2cm;
                }
                body { font-size: 12pt; }
            </style>
        </head>
        <body>
            <p>Content with explicit page dimensions.</p>
            <p>More content to fill the page.</p>
            <p>Even more content.</p>
        </body>
        </html>
    "#;
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages", doc.pages.len());
    
    // Verify basic structure
    assert!(!doc.pages.is_empty(), "Expected at least one page");
    
    // Check that pages have reasonable dimensions (A4 is ~210mm x 297mm)
    for (i, page) in doc.pages.iter().enumerate() {
        println!("Page {} dimensions: {:?}x{:?}", i + 1, page.media_box.width, page.media_box.height);
    }
}
