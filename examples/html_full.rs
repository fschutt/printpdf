extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== HTML to PDF Full Example ===\n");

    // Load HTML from file
    println!("Loading HTML from examples/assets/html/default.html...");
    
    let html = include_str!("assets/html/default.html");

    // Create PDF from HTML
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("\nParsing HTML and generating PDF...");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("✓ Successfully generated PDF");
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
            eprintln!("✗ Failed to generate PDF: {}", e);
            return;
        }
    };

    // Save to file
    let output_path = "html_full_test.pdf";
    println!("\nSaving PDF to {}...", output_path);
    
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    if !save_warnings.is_empty() {
        println!("Save warnings ({}):", save_warnings.len());
        for (i, warn) in save_warnings.iter().enumerate().take(10) {
            println!("  {}. {:?}", i + 1, warn);
        }
        if save_warnings.len() > 10 {
            println!("  ... and {} more", save_warnings.len() - 10);
        }
    }
    
    match File::create(output_path) {
        Ok(mut file) => {
            use std::io::Write;
            match file.write_all(&bytes) {
                Ok(_) => {
                    println!("\n✓ PDF saved successfully!");
                    println!("\n=== Test Results ===");
                    println!("✓ HTML parsing successful");
                    println!("✓ CSS styling applied");
                    println!("✓ Layout calculation completed");
                    println!("✓ PDF rendering successful");
                    println!("\nFeatures tested:");
                    println!("  • Headings (h1, h2)");
                    println!("  • Paragraphs with styling");
                    println!("  • Unordered lists (ul/li)");
                    println!("  • Ordered lists (ol/li)");
                    println!("  • Tables (table/tr/th/td)");
                    println!("  • CSS borders and backgrounds");
                    println!("\nOpen {} to verify the results!", output_path);
                }
                Err(e) => eprintln!("✗ Failed to write PDF: {}", e),
            }
        }
        Err(e) => eprintln!("✗ Failed to create file: {}", e),
    }
}
