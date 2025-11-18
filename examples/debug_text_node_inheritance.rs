extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("\n=== Debug: Text Node Property Inheritance ===\n");

    // Minimal HTML to test text node property inheritance
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: 'Helvetica', sans-serif;
        }
    </style>
</head>
<body>
    <h1>Bold Heading</h1>
    <p>Normal paragraph</p>
</body>
</html>
"#;

    println!("HTML:");
    println!("{}", html);
    println!("\n--- Parsing and analyzing DOM ---\n");

    // Parse HTML to DOM
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(_doc) => {
            println!("✓ PDF generated successfully");
            println!("\nCheck the styled_dom debug output above");
            println!("\nExpected behavior:");
            println!("  1. H1 element should have font-weight: bold from UA CSS");
            println!("  2. Text node inside H1 should INHERIT font-weight: bold");
            println!("  3. P element has no explicit font-weight");
            println!("  4. Text node inside P should inherit normal weight from body");
            println!("\nIf H1 text is not bold in PDF:");
            println!("  → Text nodes are not inheriting properties from parent elements");
            println!("  → Check compute_inherited_values() in prop_cache.rs");
        }
        Err(e) => {
            eprintln!("✗ Failed: {}", e);
        }
    }
}
