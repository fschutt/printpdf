//! Debug example to analyze pagination issues
extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("=== Pagination Debug Analysis ===\n");

    // Simple HTML to debug
    let html = r#"
<!DOCTYPE html>
<html>
<head>
<style>
body { font-family: Helvetica, sans-serif; font-size: 14px; }
h1 { color: #2c5aa0; font-size: 24px; margin-bottom: 10px; }
h2 { color: #2c5aa0; font-size: 18px; margin-top: 20px; }
p { margin: 10px 0; line-height: 1.5; }
.section { margin: 20px 0; padding: 15px; background: #f0f7ff; border-left: 4px solid #2c5aa0; }
</style>
</head>
<body>
<h1>DEBUG TEST DOCUMENT</h1>
<p>This is page 1 content - paragraph 1.</p>
<p>This is page 1 content - paragraph 2.</p>

<h2>Section A - First Section</h2>
<div class="section">
<p>Section A content line 1.</p>
<p>Section A content line 2.</p>
<p>Section A content line 3.</p>
</div>

<h2>Section B - Second Section</h2>
<p>Section B intro paragraph.</p>
<p>Section B detail paragraph with more text to fill space.</p>
<p>Section B conclusion paragraph.</p>

<h2>Section C - Third Section</h2>
<p>Section C content that should appear somewhere.</p>
<p>More Section C content.</p>
<p>Even more Section C content to push things to new pages.</p>
<p>Final Section C paragraph.</p>

<h2>Section D - Fourth Section</h2>
<p>Section D starts here.</p>
<p>Section D continues with more content.</p>
<p>Section D has lots of paragraphs to ensure pagination.</p>
<p>Section D paragraph 4.</p>
<p>Section D paragraph 5.</p>
<p>Section D paragraph 6.</p>

<h2>Section E - Fifth Section</h2>
<p>Section E should be on page 2 or later.</p>
<p>Section E more content.</p>
<p>Section E final content.</p>
</body>
</html>
"#;

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    // Test 1: WITHOUT headers/footers
    println!("=== TEST 1: No headers/footers ===");
    let options_no_hf = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(20.0),
        margin_left: Some(15.0),
        show_page_numbers: None,
        header_text: None,
        footer_text: None,
        skip_first_page: None,
        ..Default::default()
    };
    
    let mut warnings = Vec::new();
    match PdfDocument::from_html(&html, &images, &fonts, &options_no_hf, &mut warnings) {
        Ok(doc) => {
            println!("  Generated {} pages (no headers)", doc.pages.len());
            let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
            File::create("debug_no_headers.pdf").unwrap().write_all(&bytes).unwrap();
            println!("  Saved to debug_no_headers.pdf");
        }
        Err(e) => println!("  ERROR: {}", e),
    }
    
    // Test 2: WITH headers/footers, skip_first_page=false
    println!("\n=== TEST 2: Headers on ALL pages ===");
    let options_all_hf = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(20.0),
        margin_left: Some(15.0),
        show_page_numbers: Some(true),
        header_text: Some("DEBUG HEADER TEXT".to_string()),
        footer_text: None,
        skip_first_page: Some(false),
        ..Default::default()
    };
    
    let mut warnings = Vec::new();
    match PdfDocument::from_html(&html, &images, &fonts, &options_all_hf, &mut warnings) {
        Ok(doc) => {
            println!("  Generated {} pages (headers on all)", doc.pages.len());
            let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
            File::create("debug_headers_all.pdf").unwrap().write_all(&bytes).unwrap();
            println!("  Saved to debug_headers_all.pdf");
        }
        Err(e) => println!("  ERROR: {}", e),
    }
    
    // Test 3: WITH headers/footers, skip_first_page=true
    println!("\n=== TEST 3: Headers skip first page ===");
    let options_skip_first = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(20.0),
        margin_left: Some(15.0),
        show_page_numbers: Some(true),
        header_text: Some("SKIP FIRST HEADER".to_string()),
        footer_text: None,
        skip_first_page: Some(true),
        ..Default::default()
    };
    
    let mut warnings = Vec::new();
    match PdfDocument::from_html(&html, &images, &fonts, &options_skip_first, &mut warnings) {
        Ok(doc) => {
            println!("  Generated {} pages (skip first)", doc.pages.len());
            let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
            File::create("debug_skip_first.pdf").unwrap().write_all(&bytes).unwrap();
            println!("  Saved to debug_skip_first.pdf");
        }
        Err(e) => println!("  ERROR: {}", e),
    }
    
    println!("\n=== Debug PDFs generated ===");
    println!("Compare:");
    println!("  1. debug_no_headers.pdf - baseline without headers");
    println!("  2. debug_headers_all.pdf - headers on all pages");
    println!("  3. debug_skip_first.pdf - headers skip first page");
}
