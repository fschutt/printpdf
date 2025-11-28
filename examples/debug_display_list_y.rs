extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("=== Display List Y-Coordinate Debug ===\n");

    // Simple HTML to debug Y coordinates
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: 'Helvetica'; margin: 0; padding: 20px; }
        h1 { font-size: 24px; margin: 0 0 10px 0; }
        p { font-size: 16px; margin: 0 0 10px 0; line-height: 1.2; }
    </style>
</head>
<body>
    <h1>ACME CORPORATION</h1>
    <p>First paragraph of text that should appear right below the heading.</p>
    <p>Second paragraph that should appear below the first.</p>
    <p>Third paragraph for testing vertical spacing.</p>
</body>
</html>
"#;

    println!("HTML content:");
    println!("{}", html);
    println!("\n--- Generating PDF ---\n");

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings).unwrap();
    
    // Print warnings
    for w in &warnings {
        println!("Warning: {}", w);
    }
    
    let output_path = "debug_y_coords.pdf";
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    let mut file = File::create(output_path).unwrap();
    file.write_all(&bytes).unwrap();
    
    println!("\n✓ PDF saved to {}", output_path);
    println!("Open the PDF to check Y spacing between elements.");
}
