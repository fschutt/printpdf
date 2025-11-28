//! Debug example to dump display list items and understand pagination
extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("=== Display List Debug ===\n");

    // Very simple HTML to trace
    let html = r#"
<!DOCTYPE html>
<html>
<head>
<style>
body { font-family: Helvetica, sans-serif; font-size: 14px; }
h1 { color: blue; font-size: 24px; }
h2 { color: green; font-size: 18px; }
p { margin: 10px 0; }
</style>
</head>
<body>
<h1>TITLE ON PAGE 1</h1>
<p>Paragraph 1 on page 1.</p>
<p>Paragraph 2 on page 1.</p>
<p>Paragraph 3 on page 1.</p>

<h2>Section A</h2>
<p>Section A content.</p>

<h2>Section B</h2>
<p>Section B content - this might flow to page 2.</p>
<p>More Section B content.</p>
<p>Even more content.</p>
<p>Still more content.</p>
<p>Content continues.</p>
<p>And continues.</p>
<p>More text here.</p>
<p>Additional paragraphs.</p>
<p>To force pagination.</p>
<p>We need lots of text.</p>
<p>So the document spans.</p>
<p>Multiple pages.</p>

<h2>KEY SECTION HEADER</h2>
<p>This is the KEY SECTION content.</p>
<p>It should appear cleanly.</p>
<p>On whatever page it lands.</p>
</body>
</html>
"#;

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    // Test WITHOUT headers first to see baseline behavior
    println!("=== Generating WITHOUT headers ===");
    let options_no_header = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(20.0),
        margin_left: Some(15.0),
        show_page_numbers: Some(false),
        header_text: None,
        skip_first_page: Some(false),
        ..Default::default()
    };
    
    let mut warnings = Vec::new();
    match PdfDocument::from_html(&html, &images, &fonts, &options_no_header, &mut warnings) {
        Ok(doc) => {
            println!("Generated {} pages (no headers)", doc.pages.len());
            for (page_idx, page) in doc.pages.iter().enumerate() {
                let mut glyph_renders = 0;
                let mut y_positions: Vec<f32> = Vec::new();
                let mut current_y: Option<f32> = None;
                
                for op in &page.ops {
                    match op {
                        Op::SetTextMatrix { matrix } => {
                            if let crate::matrix::TextMatrix::Raw(m) = matrix {
                                current_y = Some(m[5]);
                            }
                        }
                        Op::ShowText { items } => {
                            for item in items {
                                if let crate::text::TextItem::GlyphIds(_) = item {
                                    glyph_renders += 1;
                                    if let Some(y) = current_y {
                                        y_positions.push(y);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                // Find max Y (top of page in PDF coords)
                let max_y = y_positions.iter().cloned().fold(0.0f32, f32::max);
                let y_near_top: Vec<_> = y_positions.iter().filter(|&&y| y > 750.0).collect();
                println!("  Page {}: {} renders, max_y={:.1}, {} renders near top (>750)", 
                    page_idx + 1, glyph_renders, max_y, y_near_top.len());
            }
        }
        Err(e) => println!("ERROR: {}", e),
    }
    
    println!("\n=== Generating WITH headers (skip_first_page=true) ===");
    let options_with_header = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(20.0),
        margin_left: Some(15.0),
        show_page_numbers: Some(true),
        header_text: Some("TEST HEADER".to_string()),
        skip_first_page: Some(true),
        ..Default::default()
    };
    
    warnings.clear();
    match PdfDocument::from_html(&html, &images, &fonts, &options_with_header, &mut warnings) {
        Ok(doc) => {
            println!("Generated {} pages (with headers)", doc.pages.len());
            for (page_idx, page) in doc.pages.iter().enumerate() {
                println!("\n--- Page {} ---", page_idx + 1);
                let mut current_y: Option<f32> = None;
                let mut glyph_at_y: Vec<(f32, Vec<u16>)> = Vec::new();
                
                for op in &page.ops {
                    match op {
                        Op::SetTextMatrix { matrix } => {
                            if let crate::matrix::TextMatrix::Raw(m) = matrix {
                                current_y = Some(m[5]);
                            }
                        }
                        Op::ShowText { items } => {
                            for item in items {
                                if let crate::text::TextItem::GlyphIds(glyphs) = item {
                                    if let Some(y) = current_y {
                                        let gids: Vec<u16> = glyphs.iter().map(|g| g.gid).collect();
                                        glyph_at_y.push((y, gids));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                // Show first few and last few positions
                println!("First 5 glyph positions:");
                for (y, gids) in glyph_at_y.iter().take(5) {
                    println!("  Y={:.1}: {} glyphs (first gid={})", y, gids.len(), gids.first().unwrap_or(&0));
                }
                
                // Find Y=771 area
                println!("\nPositions near Y=771:");
                let near_771: Vec<_> = glyph_at_y.iter().filter(|(y, _)| *y > 770.0 && *y < 773.0).collect();
                for (y, gids) in near_771.iter().take(10) {
                    println!("  Y={:.1}: {} glyphs", y, gids.len());
                }
                
                println!("\nLast 5 glyph positions:");
                for (y, gids) in glyph_at_y.iter().rev().take(5) {
                    println!("  Y={:.1}: {} glyphs", y, gids.len());
                }
            }
            
            let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
            File::create("debug_displaylist.pdf").unwrap().write_all(&bytes).unwrap();
            println!("\nSaved to debug_displaylist.pdf");
        }
        Err(e) => println!("ERROR: {}", e),
    }
}
