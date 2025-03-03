use printpdf::*;

const HTML_STRINGS: &[&str; 1] = &["<img src='test.bmp' />"];

fn main() -> Result<(), String> {
    for (i, h) in HTML_STRINGS.iter().enumerate() {
        let config = XmlRenderOptions {
            components: Vec::new(),
            images: vec![(
                "test.bmp".to_string(),
                include_bytes!("./assets/img/BMP_test.bmp").to_vec(),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let mut doc = PdfDocument::new("HTML rendering demo");
        let pages = doc.html_to_pages(h, config, &mut Vec::new())?;
        let doc = doc.with_pages(pages).save(&PdfSaveOptions::default(), &mut Vec::new());
        std::fs::write(format!("html{i}.pdf"), doc).unwrap();
    }

    Ok(())
}
