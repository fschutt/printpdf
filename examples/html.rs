use printpdf::*;

const HTML_STRINGS: &[&str; 1] = &[
    // "<div style='background:red;padding:10px;'><div style='background:yellow;padding:20px;'></div></div>",
    "<p style='color:red;font-family:sans-serif'>Hello!</p>",
];

fn main() -> Result<(), String> {
    for (i, h) in HTML_STRINGS.iter().enumerate() {
        let doc = PdfDocument::new("HTML rendering demo")
            .with_html(h, &XmlRenderOptions::default())?
            .save(&PdfSaveOptions::default());
        std::fs::write(format!("html{i}.pdf"), doc).unwrap();
    }
    Ok(())
}
