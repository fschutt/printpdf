use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("=== Margin Collapsing with Border/Padding Test ===\n");
    
    // Test 1: Normal collapsing (no border/padding)
    let html1 = r#"
<!DOCTYPE html>
<html>
<body style="width: 555px;">
    <h2>Test 1: Normal Collapsing (no border)</h2>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">First paragraph (20px margins)</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ffcccc;">Second paragraph (20px margins)</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccccff;">Third paragraph (20px margins)</p>
</body>
</html>
"#;

    // Test 2: Border prevents collapsing
    let html2 = r#"
<!DOCTYPE html>
<html>
<body style="width: 555px;">
    <h2>Test 2: Border Prevents Collapsing</h2>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">First paragraph (20px margins)</p>
    <p style="margin-top: 20px; margin-bottom: 20px; border-top: 2px solid black; background-color: #ffcccc;">Second paragraph with border-top</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccccff;">Third paragraph (20px margins)</p>
</body>
</html>
"#;

    // Test 3: Padding prevents collapsing
    let html3 = r#"
<!DOCTYPE html>
<html>
<body style="width: 555px;">
    <h2>Test 3: Padding Prevents Collapsing</h2>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">First paragraph (20px margins)</p>
    <p style="margin-top: 20px; margin-bottom: 20px; padding-top: 10px; background-color: #ffcccc;">Second paragraph with padding-top</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccccff;">Third paragraph (20px margins)</p>
</body>
</html>
"#;

    // Generate all three test PDFs
    for (i, html) in [html1, html2, html3].iter().enumerate() {
        let images = BTreeMap::new();
        let fonts = BTreeMap::new();
        let options = GeneratePdfOptions::default();
        let mut warnings = Vec::new();
        
        let pdf_doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
            .expect("Failed to create PDF");
        
        let filename = format!("margin_collapse_border_test_{}.pdf", i + 1);
        let mut save_warnings = Vec::new();
        let pdf_bytes = pdf_doc.save(&PdfSaveOptions::default(), &mut save_warnings);
        std::fs::write(&filename, &pdf_bytes).expect("Failed to write PDF file");
        
        println!("✓ Generated: {}", filename);
    }
    
    println!("\n=== Expected Behavior ===");
    println!("Test 1: Gaps should be ~20px (margins collapse)");
    println!("Test 2: Gap before bordered element should be ~40px (20+20, NO collapse due to border)");
    println!("Test 3: Gap before padded element should be ~40px (20+20, NO collapse due to padding)");
    println!("\nCompare the three PDFs to verify border/padding blocking behavior.");
}
