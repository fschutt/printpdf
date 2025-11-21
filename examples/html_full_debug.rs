extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== HTML Debug Test ===\n");

    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: 'Helvetica'; margin: 20px; }
        h1 { font-size: 2em; border-bottom: 2px solid #cc0000; }
        h2 { font-size: 1.5em; color: #0000cc; }
        .yellow { background-color: #ffff00; padding: 2px; }
        .footer { text-align: center; font-size: 12px; color: #666; }
    </style>
</head>
<body>
    <h1>Test Header with Border</h1>
    <p>Text with <span class="yellow">yellow background</span> inline.</p>
    <h2>Subheading Test</h2>
    <ol>
        <li>First item</li>
        <li>Second item</li>
        <li>Third item</li>
    </ol>
    <div class="footer">Footer text centered</div>
</body>
</html>
"#;

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings).unwrap();
    
    let output_path = "html_full_debug.pdf";
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    let mut file = File::create(output_path).unwrap();
    use std::io::Write;
    file.write_all(&bytes).unwrap();
    
    println!("\n✓ PDF saved to {}", output_path);
}
