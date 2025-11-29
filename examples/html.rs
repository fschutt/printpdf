extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("Testing HTML to PDF implementation...");

    // Get HTML file from command line args
    let args: Vec<String> = std::env::args().collect();
    let html = if args.len() >= 2 {
        println!("Reading HTML from file: {}", args[1]);
        std::fs::read_to_string(&args[1]).expect("Failed to read HTML file")
    } else {
        // Default HTML content
        r#"
        <html>
            <head>
                <style>
                    .title {
                        font-size: 24px;
                        color: #333333;
                        margin-bottom: 10px;
                    }
                    .content {
                        font-size: 14px;
                        color: #666666;
                        padding: 20px;
                    }
                    .box {
                        width: 200px;
                        height: 100px;
                        background-color: #e0e0e0;
                        border: 1px solid #999999;
                    }
                </style>
            </head>
            <body>
                <div class="title">Hello from Azul!</div>
                <div class="content">
                    This is a test of the HTML to PDF converter using azul's 
                    solver3 layout engine and text3 text shaping.
                </div>
                <div class="box"></div>
            </body>
        </html>
        "#.to_string()
    };

    // Create PDF from HTML
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("Parsing HTML and generating PDF...");
    
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(doc) => {
            println!("[OK] Successfully generated PDF");
            if !warnings.is_empty() {
                println!("Warnings:");
                for warn in &warnings {
                    println!("  - {:?}", warn);
                }
            }
            doc
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to generate PDF: {}", e);
            return;
        }
    };

    // Save to file
    let output_path = if args.len() >= 3 {
        args[2].clone()
    } else {
        "html_example.pdf".to_string()
    };
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
    
    match File::create(&output_path) {
        Ok(mut file) => {
            use std::io::Write;
            match file.write_all(&bytes) {
                Ok(_) => println!("[OK] PDF saved successfully!"),
                Err(e) => eprintln!("[ERROR] Failed to write PDF: {}", e),
            }
        }
        Err(e) => eprintln!("[ERROR] Failed to create file: {}", e),
    }
}
