use printpdf::*;
use std::collections::BTreeMap;

fn main() {
    println!("=== Text Shaping Example: ParsedFont → UnifiedLayout → Vec<Op> ===\n");

    // Create a simple HTML example that demonstrates text shaping
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .example { margin: 20px 0; padding: 10px; border-left: 3px solid #007acc; }
        .english { color: #000; }
        .arabic { color: #c41e3a; font-size: 16px; direction: rtl; }
        .mixed { color: #2e8b57; font-size: 14px; }
    </style>
</head>
<body>
    <h1>Text Shaping Demonstration</h1>
    
    <div class="example">
        <h2>1. Basic English Text</h2>
        <p class="english">This demonstrates basic Latin script shaping with proper kerning and spacing.</p>
    </div>
    
    <div class="example">
        <h2>2. Arabic Text (Right-to-Left)</h2>
        <p class="arabic">مرحبا بالعالم! هذا مثال على تشكيل النص العربي</p>
        <p class="english">(Arabic: "Hello World! This is an example of Arabic text shaping")</p>
    </div>
    
    <div class="example">
        <h2>3. Mixed Scripts</h2>
        <p class="mixed">English mixed with العربية (Arabic) demonstrates bidirectional text handling.</p>
    </div>
    
    <div class="example">
        <h2>4. Complex Layout</h2>
        <p>This showcases the <strong>ParsedFont → UnifiedLayout → Vec&lt;Op&gt;</strong> workflow:</p>
        <ul>
            <li>Font loading with ParsedFont::from_bytes()</li>
            <li>Text layout using azul's UnifiedLayout system</li>
            <li>PDF operations generation via render_unified_layout()</li>
            <li>Precise positioning with SetTextMatrix operations</li>
        </ul>
    </div>
</body>
</html>
    "#;

    // Create PDF from HTML using the complete azul text3 workflow
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("Generating PDF using azul text3 integration...");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("[OK] Successfully generated PDF using text shaping");
            
            // Print details about the text shaping process
            if !warnings.is_empty() {
                println!("\nText shaping warnings ({}):", warnings.len());
                for (i, warn) in warnings.iter().enumerate().take(5) {
                    println!("  {}. {:?}", i + 1, warn);
                }
            }
            
            doc
        }
        Err(e) => {
            println!("[ERROR] Failed to generate PDF: {}", e);
            return;
        }
    };

    // Save the PDF
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let pdf_bytes = doc.save(&save_options, &mut save_warnings);
    
    std::fs::write("text_shaping_example.pdf", &pdf_bytes).expect("Failed to write PDF");
    
    println!("\n[OK] Saved text_shaping_example.pdf ({} bytes)", pdf_bytes.len());
    
    // Print technical details
    println!("\n=== Technical Workflow ===");
    println!("1. HTML → XML parsing");
    println!("2. CSS → azul StylePropertyMap"); 
    println!("3. ParsedFont loading from system/embedded fonts");
    println!("4. Text → UnifiedLayout via azul text3 engine");
    println!("5. UnifiedLayout → PDF Operations via render_unified_layout()");
    println!("6. SetTextMatrix for absolute glyph positioning");
    
    println!("\nThis demonstrates complete text shaping for complex scripts!");
}