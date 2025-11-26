/// Example demonstrating the new integrated fragmentation/pagination system
/// 
/// This test creates a multi-page document to verify that:
/// - Page breaks work correctly
/// - break-inside: avoid is respected
/// - Orphans/widows are handled
/// - Headers/footers with page numbers work

use std::collections::BTreeMap;
use printpdf::*;

fn main() {
    println!("=== Pagination Test ===\n");
    
    // Create a document with lots of content to force multiple pages
    let html = generate_long_html();
    
    println!("Generating PDF with {} characters of HTML...", html.len());
    
    // Configure page margins
    let options = GeneratePdfOptions {
        font_embedding: Some(true),
        page_width: Some(210.0),     // A4 width in mm
        page_height: Some(297.0),    // A4 height in mm
        margin_top: Some(20.0),
        margin_right: Some(15.0),
        margin_bottom: Some(25.0),   // Extra space for page numbers
        margin_left: Some(15.0),
        image_optimization: None,
    };
    
    let mut warnings = Vec::new();
    let images = BTreeMap::new();
    let fonts = BTreeMap::new();
    
    match PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings) {
        Ok(pdf) => {
            let output_path = "pagination_test.pdf";
            let mut save_warnings = Vec::new();
            let pdf_bytes = pdf.save(&PdfSaveOptions::default(), &mut save_warnings);
            std::fs::write(output_path, pdf_bytes).expect("Failed to write PDF");
            println!("✓ Successfully generated {} pages", pdf.pages.len());
            println!("✓ PDF saved to {}", output_path);
            
            if !warnings.is_empty() {
                println!("\nWarnings during generation:");
                for w in &warnings {
                    println!("  - {:?}", w);
                }
            }
            if !save_warnings.is_empty() {
                println!("\nWarnings during save:");
                for w in &save_warnings {
                    println!("  - {:?}", w);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to generate PDF: {:?}", e);
        }
    }
}

fn generate_long_html() -> String {
    let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: sans-serif;
            line-height: 1.6;
            color: #333;
        }
        h1 {
            color: #1a5276;
            border-bottom: 2px solid #1a5276;
            padding-bottom: 10px;
            page-break-after: avoid; /* Should stay with following content */
        }
        h2 {
            color: #2874a6;
            margin-top: 20px;
            page-break-after: avoid;
        }
        h3 {
            color: #3498db;
            page-break-after: avoid;
        }
        p {
            margin-bottom: 12px;
            orphans: 3;  /* Minimum 3 lines at page bottom */
            widows: 3;   /* Minimum 3 lines at page top */
        }
        .avoid-break {
            break-inside: avoid;
            border: 1px solid #ddd;
            padding: 15px;
            margin: 15px 0;
            background-color: #f8f9fa;
        }
        .force-break {
            page-break-before: always;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin: 15px 0;
            break-inside: avoid;
        }
        th {
            background-color: #1a5276;
            color: white;
            padding: 10px;
            text-align: left;
        }
        td {
            border: 1px solid #ddd;
            padding: 8px;
        }
        .page-info {
            position: fixed;
            bottom: 10mm;
            width: 100%;
            text-align: center;
            font-size: 10px;
            color: #777;
        }
    </style>
</head>
<body>
    <h1>CSS Fragmentation Test Document</h1>
    
    <p>This document tests the new integrated fragmentation system that respects CSS 
    break properties during layout, rather than splitting content post-layout.</p>
"#);

    // Add sections that should force page breaks
    for section in 1..=5 {
        html.push_str(&format!(r#"
    <h2>Section {}: Lorem Ipsum Content</h2>
    
    <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor 
    incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud 
    exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.</p>
    
    <p>Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu 
    fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa 
    qui officia deserunt mollit anim id est laborum.</p>
    
    <div class="avoid-break">
        <h3>Important Note (break-inside: avoid)</h3>
        <p>This entire box should stay together on one page. If there isn't enough room 
        on the current page, the whole box should move to the next page rather than being 
        split in the middle.</p>
        <ul>
            <li>Point one - should stay with header</li>
            <li>Point two - should stay with header</li>
            <li>Point three - should stay with header</li>
        </ul>
    </div>
    
    <p>Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium 
    doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis 
    et quasi architecto beatae vitae dicta sunt explicabo.</p>
    
    <p>Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed 
    quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro 
    quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit.</p>
"#, section));

        // Add a table every other section
        if section % 2 == 0 {
            html.push_str(&format!(r#"
    <table>
        <tr>
            <th>Section {} - Quarter</th>
            <th>Revenue</th>
            <th>Growth</th>
        </tr>
        <tr>
            <td>Q1</td>
            <td>$1.2M</td>
            <td>+15%</td>
        </tr>
        <tr>
            <td>Q2</td>
            <td>$1.4M</td>
            <td>+17%</td>
        </tr>
        <tr>
            <td>Q3</td>
            <td>$1.6M</td>
            <td>+14%</td>
        </tr>
        <tr>
            <td>Q4</td>
            <td>$1.9M</td>
            <td>+19%</td>
        </tr>
    </table>
"#, section));
        }

        // Add more paragraphs to ensure we have plenty of content
        for para in 1..=3 {
            html.push_str(&format!(r#"
    <p>Additional paragraph {} for section {}. At vero eos et accusamus et iusto odio 
    dignissimos ducimus qui blanditiis praesentium voluptatum deleniti atque corrupti quos 
    dolores et quas molestias excepturi sint occaecati cupiditate non provident, similique 
    sunt in culpa qui officia deserunt mollitia animi, id est laborum et dolorum fuga.</p>
"#, para, section));
        }
    }

    // Add a forced page break section
    html.push_str(r#"
    <div class="force-break">
        <h2>Appendix (forced page break before)</h2>
        <p>This section should always start on a new page due to page-break-before: always.</p>
    </div>
    
    <h3>Final Notes</h3>
    <p>This is the end of the test document. If pagination is working correctly, you should 
    see multiple pages with proper break handling:</p>
    <ol>
        <li>Headers stay with their following content (page-break-after: avoid)</li>
        <li>Boxes with break-inside: avoid stay together</li>
        <li>Tables don't get split in the middle</li>
        <li>The appendix starts on its own page (page-break-before: always)</li>
    </ol>
</body>
</html>"#);

    html
}
