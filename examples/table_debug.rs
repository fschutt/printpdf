extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;
use std::fs::File;

fn main() {
    println!("=== Table Debug Test ===\n");

    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: 'Helvetica'; margin: 20px; }
        table {
            width: 100%;
            border-collapse: collapse;
            margin: 15px 0;
        }
        th {
            background-color: #1a5276;
            color: white;
            font-weight: bold;
            text-align: left;
            padding: 8px;
            border: 1px solid #ddd;
        }
        td {
            padding: 8px;
            border: 1px solid #ddd;
        }
    </style>
</head>
<body>
    <h2>Simple Table Test</h2>
    <table>
        <tr>
            <th>Business Unit</th>
            <th>Revenue</th>
            <th>YoY Growth</th>
            <th>Operating Margin</th>
            <th>YoY Change</th>
        </tr>
        <tr>
            <td>Technology</td>
            <td>143.2</td>
            <td>+18.7%</td>
            <td>24.3%</td>
            <td>+3.2pts</td>
        </tr>
        <tr>
            <td>Manufacturing</td>
            <td>82.5</td>
            <td>+8.4%</td>
            <td>15.8%</td>
            <td>+1.5pts</td>
        </tr>
        <tr>
            <td>Consumer Products</td>
            <td>45.3</td>
            <td>+5.2%</td>
            <td>12.1%</td>
            <td>+0.8pts</td>
        </tr>
        <tr>
            <td>Services</td>
            <td>16.5</td>
            <td>+2.1%</td>
            <td>14.5%</td>
            <td>-0.5pts</td>
        </tr>
    </table>
</body>
</html>
"#;

    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();

    println!("Generating PDF from HTML...");
    let doc = match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {:?}", e);
            return;
        }
    };
    
    for w in &warnings {
        println!("Warning: {:?}", w);
    }

    // Debug: Print page operations
    println!("\n=== PDF Page Operations ===\n");
    for (page_idx, page) in doc.pages.iter().enumerate() {
        println!("Page {} ({} operations):", page_idx, page.ops.len());
        for (op_idx, op) in page.ops.iter().enumerate() {
            let op_str = format!("{:?}", op);
            let display = if op_str.len() > 200 {
                format!("{}...", &op_str[..200])
            } else {
                op_str
            };
            println!("  [{:04}] {}", op_idx, display);
        }
    }
    println!();
    
    let output_path = "table_debug.pdf";
    let save_options = PdfSaveOptions::default();
    let mut save_warnings = Vec::new();
    let bytes = doc.save(&save_options, &mut save_warnings);
    
    let mut file = File::create(output_path).unwrap();
    use std::io::Write;
    file.write_all(&bytes).unwrap();
    
    println!("\n[OK] PDF saved to {}", output_path);
    println!("Open {} to check the table rendering", output_path);
}
