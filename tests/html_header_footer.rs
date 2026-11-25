//! Header text, footer text and page numbers of `GeneratePdfOptions` are drawn on every page.
//!
//! azul-layout draws them through its font manager (with `FakePageConfig::with_font_family` it
//! can name the family), also when the caller registers no font, which is what `from_html` does.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

/// Three pages of text, the paragraphs separated by forced page breaks.
const THREE_PAGES: &str = r#"<html><body style="font-size: 12pt;">
    <p>First page body.</p>
    <p style="break-before: page;">Second page body.</p>
    <p style="break-before: page;">Third page body.</p>
</body></html>"#;

fn pages_text(opts: &GeneratePdfOptions) -> Vec<String> {
    let mut w = Vec::new();
    let doc = PdfDocument::from_html(THREE_PAGES, &BTreeMap::new(), &BTreeMap::new(), opts, &mut w)
        .expect("the document renders");
    // Through a save and a parse: the text has to be in the PDF, not only in the ops.
    let bytes = doc.save(&PdfSaveOptions::default(), &mut w);
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut w).expect("parse");
    parsed
        .extract_text()
        .iter()
        .map(|page| page.join(" "))
        .collect()
}

fn options() -> GeneratePdfOptions {
    GeneratePdfOptions {
        page_width: Some(210.0),
        page_height: Some(297.0),
        margin_top: Some(20.0),
        margin_bottom: Some(20.0),
        margin_left: Some(20.0),
        margin_right: Some(20.0),
        ..GeneratePdfOptions::default()
    }
}

#[test]
fn header_and_footer_text_are_on_every_page() {
    let text = pages_text(&GeneratePdfOptions {
        header_text: Some("HEADERWORD".to_string()),
        footer_text: Some("FOOTERWORD".to_string()),
        ..options()
    });
    assert_eq!(text.len(), 3, "{text:?}");
    for (i, page) in text.iter().enumerate() {
        assert!(page.contains("HEADERWORD"), "page {} has no header: {page:?}", i + 1);
        assert!(page.contains("FOOTERWORD"), "page {} has no footer: {page:?}", i + 1);
    }
}

#[test]
fn skip_first_page_leaves_the_first_page_without() {
    let text = pages_text(&GeneratePdfOptions {
        header_text: Some("HEADERWORD".to_string()),
        skip_first_page: Some(true),
        ..options()
    });
    assert!(!text[0].contains("HEADERWORD"), "{:?}", text[0]);
    assert!(text[1].contains("HEADERWORD") && text[2].contains("HEADERWORD"), "{text:?}");
}

#[test]
fn page_numbers_count_the_pages() {
    let text = pages_text(&GeneratePdfOptions {
        show_page_numbers: Some(true),
        ..options()
    });
    for (i, page) in text.iter().enumerate() {
        let number = (i + 1).to_string();
        assert!(page.contains(&number), "page {} shows no page number: {page:?}", i + 1);
    }
}

#[test]
fn without_the_options_the_pages_have_only_their_body() {
    let text = pages_text(&options());
    assert_eq!(text.len(), 3);
    assert!(!text[0].contains("HEADERWORD") && text[0].contains("First page body"), "{:?}", text[0]);
}
