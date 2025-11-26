extern crate printpdf;

// Visual test for CSS vertical-align baseline alignment
//
// This example demonstrates and tests the baseline alignment behavior
// for inline elements including:
// - Text with different font sizes
// - Inline images with vertical-align: baseline, top, middle, bottom
// - Mixed inline content
//
// Run with: cargo run --example baseline_alignment
// Output: baseline_alignment.pdf

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("Testing CSS vertical-align baseline alignment...");

    let html = r##"
<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: sans-serif;
            font-size: 16px;
            padding: 20px;
        }
        
        h1 {
            font-size: 20px;
            margin-bottom: 5px;
            color: #333;
        }
        
        h2 {
            font-size: 14px;
            margin-top: 30px;
            margin-bottom: 10px;
            color: #666;
            border-bottom: 1px solid #ccc;
        }
        
        .test-container {
            margin: 10px 0;
            padding: 10px;
            border: 1px solid #ddd;
            background-color: #f9f9f9;
        }
        
        .baseline-demo {
            background-color: #e8f4ff;
            padding: 5px;
        }
        
        /* Inline boxes to simulate images with different alignments */
        .inline-box {
            display: inline-block;
            width: 30px;
            height: 20px;
            background-color: #4a90d9;
            border: 1px solid #2a70b9;
        }
        
        .inline-box-tall {
            display: inline-block;
            width: 20px;
            height: 40px;
            background-color: #d94a4a;
            border: 1px solid #b92a2a;
        }
        
        .va-baseline { vertical-align: baseline; }
        .va-top { vertical-align: top; }
        .va-middle { vertical-align: middle; }
        .va-bottom { vertical-align: bottom; }
        .va-text-top { vertical-align: text-top; }
        .va-text-bottom { vertical-align: text-bottom; }
        .va-sub { vertical-align: sub; }
        .va-super { vertical-align: super; }
        
        .small-text { font-size: 10px; }
        .large-text { font-size: 24px; }
        .superscript { font-size: 10px; vertical-align: super; }
        .subscript { font-size: 10px; vertical-align: sub; }
        
        .label {
            display: inline-block;
            width: 80px;
            font-size: 11px;
            color: #666;
        }
        
        .note {
            font-size: 11px;
            color: #888;
            margin-top: 5px;
        }
    </style>
</head>
<body>
    <h1>CSS vertical-align Baseline Alignment Test</h1>
    
    <h2>1. Text Baseline Alignment</h2>
    <div class="test-container">
        <p class="baseline-demo">
            Normal text <span class="small-text">small text</span> 
            normal <span class="large-text">LARGE</span> normal again
        </p>
        <p class="note">All text should align on the same baseline regardless of size.</p>
    </div>
    
    <h2>2. Subscript and Superscript</h2>
    <div class="test-container">
        <p class="baseline-demo">
            H<span class="subscript">2</span>O is water. 
            E = mc<span class="superscript">2</span> is famous.
            X<span class="subscript">i</span><span class="superscript">j</span> matrix notation.
        </p>
        <p class="note">Sub/super scripts should be positioned below/above the baseline.</p>
    </div>
    
    <h2>3. Inline Boxes with vertical-align</h2>
    <div class="test-container">
        <p class="baseline-demo">
            <span class="label">baseline:</span>
            Text <span class="inline-box va-baseline"></span> text
        </p>
        <p class="baseline-demo">
            <span class="label">top:</span>
            Text <span class="inline-box va-top"></span> text
        </p>
        <p class="baseline-demo">
            <span class="label">middle:</span>
            Text <span class="inline-box va-middle"></span> text
        </p>
        <p class="baseline-demo">
            <span class="label">bottom:</span>
            Text <span class="inline-box va-bottom"></span> text
        </p>
        <p class="note">
            Each box should be aligned differently within its line box.
        </p>
    </div>
    
    <h2>4. Tall Inline Boxes</h2>
    <div class="test-container">
        <p class="baseline-demo">
            <span class="label">baseline:</span>
            Hello <span class="inline-box-tall va-baseline"></span> world
        </p>
        <p class="baseline-demo">
            <span class="label">top:</span>
            Hello <span class="inline-box-tall va-top"></span> world
        </p>
        <p class="baseline-demo">
            <span class="label">middle:</span>
            Hello <span class="inline-box-tall va-middle"></span> world
        </p>
        <p class="baseline-demo">
            <span class="label">bottom:</span>
            Hello <span class="inline-box-tall va-bottom"></span> world
        </p>
        <p class="note">
            Tall boxes should expand the line box height when needed.
        </p>
    </div>
    
    <h2>5. Mixed Content</h2>
    <div class="test-container">
        <p class="baseline-demo">
            Start <span class="large-text">BIG</span> 
            <span class="inline-box va-middle"></span>
            <span class="small-text">tiny</span>
            <span class="inline-box-tall va-baseline"></span>
            end
        </p>
        <p class="note">
            Mixed sizes and inline boxes should all work together.
        </p>
    </div>
    
    <h2>6. Implementation Status</h2>
    <div class="test-container">
        <p><strong>Currently implemented:</strong></p>
        <ul>
            <li>✅ Global vertical-align on UnifiedConstraints</li>
            <li>✅ baseline, top, middle, bottom alignment modes</li>
            <li>✅ sub, super for script positioning</li>
        </ul>
        <p><strong>Known limitations:</strong></p>
        <ul>
            <li>⚠️ Per-image vertical-align stored but not used in positioning</li>
            <li>⚠️ text-top, text-bottom fall back to baseline</li>
            <li>⚠️ Length/percentage values not supported</li>
        </ul>
    </div>
</body>
</html>
"##;

    // Create PDF from HTML
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let mut options = GeneratePdfOptions::default();
    options.page_width = Some(210.0); // A4 in mm
    options.page_height = Some(297.0);
    let mut warnings = Vec::new();

    println!("Parsing HTML and generating PDF...");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("✓ Successfully generated PDF");
            if !warnings.is_empty() {
                println!("Warnings ({}):", warnings.len());
                for warn in warnings.iter().take(10) {
                    println!("  - {:?}", warn);
                }
                if warnings.len() > 10 {
                    println!("  ... and {} more warnings", warnings.len() - 10);
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
    let output_path = "baseline_alignment.pdf";
    println!("Saving PDF to {}...", output_path);
    
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    if !save_warnings.is_empty() {
        println!("Save warnings:");
        for warn in &save_warnings {
            println!("  - {:?}", warn);
        }
    }
    
    match File::create(output_path) {
        Ok(mut file) => {
            match file.write_all(&bytes) {
                Ok(_) => {
                    println!("✓ PDF saved successfully!");
                    println!("");
                    println!("Open {} to verify baseline alignment visually.", output_path);
                    println!("");
                    println!("What to check:");
                    println!("1. Text of different sizes should share a common baseline");
                    println!("2. Sub/superscripts should be positioned correctly");
                    println!("3. Inline boxes should align according to their vertical-align value");
                    println!("4. The line box should expand to fit tall inline elements");
                }
                Err(e) => eprintln!("✗ Failed to write PDF: {}", e),
            }
        }
        Err(e) => eprintln!("✗ Failed to create file: {}", e),
    }
}
