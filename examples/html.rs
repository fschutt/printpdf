use std::collections::BTreeMap;

use printpdf::*;

fn main() -> Result<(), String> {
    // Create a new PDF document
    let mut doc = PdfDocument::new("HTML to PDF Example");

    // Basic HTML content with styles
    let html = vec![
        (
            "default",
            include_str!("./assets/html/default.html").to_string(),
        ),
        (
            "recipe",
            include_str!("./assets/html/recipe.html").to_string(),
        ),
        (
            "report",
            include_str!("./assets/html/report.html").to_string(),
        ),
        (
            "synthwave",
            include_str!("./assets/html/synthwave.html").to_string(),
        ),
    ];

    // Convert the HTML to PDF pages
    let mut warnings = Vec::new();
    let mut pages = Vec::new();
    for (_id, html) in html.iter() {
        let newpages = PdfDocument::from_html(
            &html,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &GeneratePdfOptions::default(),
            &mut Vec::new(),
        );

        pages.append(&mut newpages.unwrap_or_default().pages);
    }

    // Add the pages to the document
    doc.with_pages(pages);

    // Save the PDF to a file
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    std::fs::write("./html_example.pdf", bytes).unwrap();
    println!("Created html_example.pdf");

    Ok(())
}
