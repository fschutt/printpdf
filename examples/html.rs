use printpdf::*;

const HTML_STRINGS: &[&str;2] = &[
    "<p>Hello!</p>",
    "<p style='color:red;font-family:sans-serif'>Hello!</p>",
];

fn main() -> Result<(), String>{
    for (i, h) in HTML_STRINGS.iter().enumerate() {
        let doc = PdfDocument::new("HTML rendering demo")
        .with_html(h, &XmlRenderOptions::default())?
        .save(&PdfSaveOptions::default());
        std::fs::write(format!("html{i}.pdf"), doc).unwrap();
    }
    Ok(())
}
