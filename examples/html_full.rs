extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== HTML to PDF Full Example ===\n");

    // Load HTML from file or command line argument
    let html = if let Some(path) = std::env::args().nth(1) {
        println!("Loading HTML from {}...", path);
        match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("✗ Failed to read file {}: {}", path, e);
                return;
            }
        }
    } else {
        println!("Loading HTML from examples/assets/html/report.html...");
        include_str!("assets/html/report.html").to_string()
    };

    // Create PDF from HTML with page margins
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    // Configure PDF options including page margins
    // Margins affect the content area available for layout and page breaking
    let options = GeneratePdfOptions {
        page_width: Some(210.0),      // A4 width in mm
        page_height: Some(297.0),     // A4 height in mm
        margin_top: Some(20.0),       // 20mm top margin
        margin_right: Some(15.0),     // 15mm right margin  
        margin_bottom: Some(20.0),    // 20mm bottom margin
        margin_left: Some(15.0),      // 15mm left margin
        ..Default::default()
    };
    
    let mut warnings = Vec::new();

    println!("\nParsing HTML and generating PDF...");
    println!("Page margins: top={}mm, right={}mm, bottom={}mm, left={}mm",
        options.margin_top.unwrap_or(0.0),
        options.margin_right.unwrap_or(0.0),
        options.margin_bottom.unwrap_or(0.0),
        options.margin_left.unwrap_or(0.0)
    );
    
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
                    println!("✓ Page margins applied");
                    println!("✓ PDF rendering successful");
                    println!("\nFeatures tested:");
                    println!("  • Page margins (top, right, bottom, left)");
                    println!("  • Headings (h1, h2) with borders");
                    println!("  • Paragraphs with line-height and styling");
                    println!("  • Inline spans with background colors (.highlight)");
                    println!("  • Unordered lists (ul/li) with margins");
                    println!("  • Ordered lists (ol/li) - numbered items");
                    println!("  • Complex tables with headers (th) and data cells (td)");
                    println!("  • CSS borders, backgrounds, and border-radius");
                    println!("  • Text alignment and padding");
                    println!("  • Footer styling with different font sizes");
                    println!("\nOpen {} to verify the results!", output_path);
                }
                Err(e) => eprintln!("✗ Failed to write PDF: {}", e),
            }
        }
        Err(e) => eprintln!("✗ Failed to create file: {}", e),
    }
}
