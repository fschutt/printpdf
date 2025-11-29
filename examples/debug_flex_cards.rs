extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== Flex Cards Debug Test ===\n");

    // Minimal test case for flex layout with cards
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: 'Helvetica';
            margin: 20px;
        }
        .kpi-cards {
            display: flex;
            justify-content: space-between;
            margin: 20px 0;
            background-color: #eeeeee;
        }
        .kpi-card {
            width: 30%;
            padding: 15px;
            border-radius: 5px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
            text-align: center;
        }
        h3 {
            margin: 0 0 10px 0;
            color: #333;
        }
        p {
            margin: 5px 0;
        }
        .positive {
            color: #27ae60;
            font-weight: bold;
        }
    </style>
</head>
<body>
    <h1>Flex Cards Test</h1>
    
    <div class="kpi-cards">
        <div class="kpi-card" style="background-color: #e8f8f5;">
            <h3>Revenue</h3>
            <p style="font-size: 24px;">$287.5M</p>
            <p class="positive">+12.3% YoY</p>
        </div>
        
        <div class="kpi-card" style="background-color: #fef9e7;">
            <h3>Operating Margin (Two Lines)</h3>
            <p style="font-size: 24px;">18.5%</p>
            <p class="positive">+2.1pts YoY</p>
        </div>
        
        <div class="kpi-card" style="background-color: #ebf5fb;">
            <h3>Net Income</h3>
            <p style="font-size: 24px;">$42.3M</p>
            <p class="positive">+15.7% YoY</p>
        </div>
    </div>
    
    <p>The three cards above should have the same height (align-items: stretch is default).</p>
    <p>The middle card has a longer h3 title that wraps to two lines.</p>
</body>
</html>
"#;

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    let options = GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        ..Default::default()
    };
    
    let mut warnings = Vec::new();

    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings).unwrap();
    
    for warning in &warnings {
        println!("Warning: {}", warning);
    }
    
    let output_path = "debug_flex_cards.pdf";
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    let mut file = File::create(output_path).unwrap();
    use std::io::Write;
    file.write_all(&bytes).unwrap();
    
    println!("\n[OK] PDF saved to {}", output_path);
    println!("\nOpen {} to verify the flex card layout!", output_path);
}
