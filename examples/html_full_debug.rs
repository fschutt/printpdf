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
        .yellow { background-color: #ffff00; }
        .blue { background-color: #0000ff; color: white; }
    </style>
</head>
<body>
    <p>Text with <span class="yellow">yellow background</span> inline.</p>
    <p>Another line with <span class="blue">blue background and white text</span> to test.</p>
    <ol>
        <li>First item with list marker</li>
        <li>Second item</li>
    </ol>
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
    
    println!("\n[OK] PDF saved to {}", output_path);
}
