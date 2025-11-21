extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== HTML Inline Debug Test ===\n");

    let html = include_str!("assets/html/test_inline_debug.html");

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("Parsing HTML and generating PDF...\n");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("✓ Successfully generated PDF");
            if !warnings.is_empty() {
                println!("\nGeneration warnings ({}):", warnings.len());
                for (i, warn) in warnings.iter().enumerate() {
                    println!("  {}. {:?}", i + 1, warn);
                }
            }
            doc
        }
        Err(e) => {
            eprintln!("✗ Failed to generate PDF: {}", e);
            return;
        }
    };

    // Debug: Output pages[0].ops
    println!("\n=== Debug: pages[0].ops ===");
    if let Some(page) = doc.pages.get(0) {
        println!("Page 0 has {} operations:", page.ops.len());
        for (i, op) in page.ops.iter().enumerate() {
            println!("  Op[{}]: {:?}", i, op);
        }
    } else {
        println!("No pages found!");
    }
    
    let output_path = "html_inline_debug_test.pdf";
    println!("\nSaving PDF to {}...", output_path);
    
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    match File::create(output_path) {
        Ok(mut file) => {
            use std::io::Write;
            match file.write_all(&bytes) {
                Ok(_) => println!("✓ PDF saved successfully!"),
                Err(e) => eprintln!("✗ Failed to write PDF: {}", e),
            }
        }
        Err(e) => eprintln!("✗ Failed to create file: {}", e),
    }
}
