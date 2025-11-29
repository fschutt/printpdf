use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("=== Margin Collapsing Test (Inline Styles) ===");
    println!("This test uses inline styles to avoid CSS parsing issues\n");
    
    // Create HTML string with inline styles
    let html = r#"
<!DOCTYPE html>
<html>
<body style="width: 555px; background-color: #f0f0f0;">
    <h1 style="margin-top: 10px; margin-bottom: 30px; background-color: #ffcccc;">Test Heading 1</h1>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">First paragraph after heading.</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">Second paragraph.</p>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">Third paragraph.</p>
    <div style="margin-top: 40px; margin-bottom: 40px; background-color: #ccccff;">Box content</div>
    <p style="margin-top: 20px; margin-bottom: 20px; background-color: #ccffcc;">Paragraph after box.</p>
</body>
</html>
"#;
    
    println!("[OK] HTML created with inline styles");
    
    // Convert to PDF
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    
    let pdf_doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
        .expect("Failed to create PDF");
    
    println!("[OK] PDF generated successfully");
    
    // Save to file
    let filename = "margin_collapse_inline_test.pdf";
    let mut save_warnings = Vec::new();
    let pdf_bytes = pdf_doc.save(&PdfSaveOptions::default(), &mut save_warnings);
    std::fs::write(filename, &pdf_bytes).expect("Failed to write PDF file");
    
    println!("[OK] Saved to {}", filename);
    println!("\n=== Expected Behavior ===");
    println!("With margin collapsing:");
    println!("  1. H1 (margin-bottom: 30px) + P (margin-top: 20px) → 30px gap (not 50px)");
    println!("  2. P (margin-bottom: 20px) + P (margin-top: 20px) → 20px gap (not 40px)");
    println!("  3. P (margin-bottom: 20px) + Div (margin-top: 40px) → 40px gap (not 60px)");
    println!("  4. Div (margin-bottom: 40px) + P (margin-top: 20px) → 40px gap (not 60px)");
    println!("\nWithout margin collapsing:");
    println!("  All gaps would be the sum of both margins (incorrect behavior)");
    println!("\nOpen {} to verify the gaps visually.", filename);
}
