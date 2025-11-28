//! Test for pagination boundary issues - verifies text doesn't duplicate across pages
extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("=== Pagination Boundary Test ===\n");

    // Generate HTML with numbered paragraphs to easily spot duplication
    let mut paragraphs = String::new();
    for i in 1..=50 {
        paragraphs.push_str(&format!(
            "<p style=\"margin: 8px 0; padding: 5px; border-bottom: 1px solid #ddd;\">Paragraph #{:02} - This is a test paragraph with enough text to be visible. Line number {} should appear on exactly ONE page.</p>\n",
            i, i
        ));
    }

    let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
<style>
body {{ font-family: Helvetica, sans-serif; font-size: 12px; line-height: 1.4; }}
h1 {{ color: #2c5aa0; font-size: 20px; margin-bottom: 15px; text-align: center; }}
p {{ margin: 8px 0; }}
.page-marker {{ background: #ffeb3b; padding: 2px 5px; font-weight: bold; }}
</style>
</head>
<body>
<h1>Pagination Boundary Test Document</h1>
<p><strong>Instructions:</strong> Check that each paragraph number appears on EXACTLY ONE page. If you see "Paragraph #XX" on two pages, the bug is NOT fixed.</p>
<hr style="margin: 20px 0;">
{}
<hr style="margin: 20px 0;">
<p style="text-align: center;"><strong>END OF DOCUMENT</strong></p>
</body>
</html>
"#, paragraphs);

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    let options = GeneratePdfOptions {
        page_width: Some(210.0),  // A4
        page_height: Some(297.0),
        margin_top: Some(15.0),
        margin_right: Some(15.0),
        margin_bottom: Some(15.0),
        margin_left: Some(15.0),
        show_page_numbers: Some(true),
        header_text: None,
        footer_text: None,
        skip_first_page: None,
        ..Default::default()
    };
    
    let mut warnings = Vec::new();
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            let page_count = doc.pages.len();
            println!("Generated {} pages", page_count);
            println!("Expected: Each paragraph appears on exactly ONE page");
            println!("\nIf text is duplicated at page boundaries, the bug is NOT fixed.");
            
            let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
            File::create("pagination_boundary_test.pdf").unwrap().write_all(&bytes).unwrap();
            println!("\nSaved to pagination_boundary_test.pdf");
            println!("Please open the PDF and visually verify:");
            println!("  - Paragraph numbers are sequential (no gaps, no duplicates)");
            println!("  - Text near page boundaries appears on ONE page only");
        }
        Err(e) => println!("ERROR: {}", e),
    }
}
