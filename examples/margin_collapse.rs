extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== Margin Collapsing Test ===\n");

    // Load HTML from file
    println!("Loading HTML from examples/assets/html/margin_collapse_test.html...");
    
    let html = include_str!("assets/html/margin_collapse_test.html");

    // Create PDF from HTML
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("\nParsing HTML and generating PDF...");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("[OK] Successfully generated PDF");
            if !warnings.is_empty() {
                println!("\nGeneration warnings ({}):", warnings.len());
                for (i, warn) in warnings.iter().enumerate().take(10) {
                    println!("  {}. {:?}", i + 1, warn);
                }
                if warnings.len() > 10 {
                    println!("  ... and {} more", warnings.len() - 10);
                }
            }
            doc
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to generate PDF: {}", e);
            return;
        }
    };

    // Save to file
    let output_path = "margin_collapse_test.pdf";
    println!("\nSaving PDF to {}...", output_path);
    
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    if !save_warnings.is_empty() {
        println!("\nSave warnings ({}):", save_warnings.len());
        for (i, warn) in save_warnings.iter().enumerate().take(5) {
            println!("  {}. {:?}", i + 1, warn);
        }
    }

    match std::fs::write(output_path, bytes) {
        Ok(_) => println!("[OK] Saved successfully"),
        Err(e) => eprintln!("[ERROR] Failed to save: {}", e),
    }
    
    println!("\n=== Test Complete ===");
    println!("\nOpen {} to verify margin collapsing:", output_path);
    println!("1. Check body-h1 top margin (should be 20px, not 30px)");
    println!("2. Check h1-p margin (should be 30px, not 50px)");
    println!("3. Check p-p sibling margins (should be 20px, not 40px)");
    println!("4. Check p-div margin with larger value (should be 40px)");
}
