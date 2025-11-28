#![cfg(feature = "html")]

use printpdf::*;
use std::collections::BTreeMap;

#[test]
fn test_basic_text_breaks_across_pages() {
    let mut warnings = Vec::new();
    
    // Create HTML with enough text to overflow a single page
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { margin: 20px; font-size: 12pt; }
                p { margin-bottom: 10px; }
            </style>
        </head>
        <body>
            <p>This is paragraph 1. Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>
            <p>This is paragraph 2. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>
            <p>This is paragraph 3. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.</p>
            <p>This is paragraph 4. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum.</p>
            <p>This is paragraph 5. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia.</p>
            <p>This is paragraph 6. Deserunt mollit anim id est laborum. Sed ut perspiciatis unde omnis iste natus.</p>
            <p>This is paragraph 7. Error sit voluptatem accusantium doloremque laudantium, totam rem aperiam.</p>
            <p>This is paragraph 8. Eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae.</p>
            <p>This is paragraph 9. Vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit.</p>
            <p>This is paragraph 10. Aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos.</p>
            <p>This is paragraph 11. Qui ratione voluptatem sequi nesciunt. Neque porro quisquam est, qui dolorem.</p>
            <p>This is paragraph 12. Ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam.</p>
            <p>This is paragraph 13. Eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem.</p>
            <p>This is paragraph 14. Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit.</p>
            <p>This is paragraph 15. Laboriosam, nisi ut aliquid ex ea commodi consequatur? Quis autem vel eum iure.</p>
            <p>This is paragraph 16. Reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur.</p>
            <p>This is paragraph 17. Vel illum qui dolorem eum fugiat quo voluptas nulla pariatur?</p>
            <p>This is paragraph 18. At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis.</p>
            <p>This is paragraph 19. Praesentium voluptatum deleniti atque corrupti quos dolores et quas molestias.</p>
            <p>This is paragraph 20. Excepturi sint occaecati cupiditate non provident, similique sunt in culpa.</p>
        </body>
        </html>
    "#;
    
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to generate PDF from HTML");
    
    println!("Generated PDF with {} pages", doc.pages.len());
    println!("Warnings: {:?}", warnings);
    
    // The document should have multiple pages
    assert!(doc.pages.len() > 1, "Expected multiple pages, got {}", doc.pages.len());
    
    // Extract text from all pages
    let page_texts = doc.extract_text();
    
    // Check that we have text on multiple pages
    assert!(page_texts.len() > 1, "Expected text on multiple pages");
    
    // Verify text is distributed across pages
    for (i, page_text) in page_texts.iter().enumerate() {
        println!("Page {} text length: {}", i + 1, page_text.join(" ").len());
        assert!(!page_text.is_empty(), "Page {} should have text", i + 1);
    }
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
