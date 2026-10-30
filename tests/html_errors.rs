//! `from_html` never answers `Ok` with an empty document.
//!
//! A page the HTML loader could not read used to come back as `Ok(PdfDocument)`
//! with no pages and the reason only in `warnings` (pdfocr #11: a noncharacter in
//! the text dropped the whole page). A conversion either gives pages or an `Err`
//! that says why; the warnings keep their details either way.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::{GeneratePdfOptions, PdfDocument};

/// Characters XML 1.0 excludes from `Char` (a strict loader refuses them; a
/// normalizing one replaces them). Whatever the loader does, the answer is pages
/// or an error, never an empty `Ok`.
#[test]
fn text_with_characters_xml_excludes_gives_pages_or_an_error() {
    for bad in ["\u{0}", "\u{8}", "\u{B}", "\u{C}", "\u{1F}", "\u{FFFE}", "\u{FFFF}"] {
        let html = format!("<html><body><p>before{bad}after</p></body></html>");
        let mut warnings = Vec::new();
        match PdfDocument::from_html(
            &html,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &GeneratePdfOptions::default(),
            &mut warnings,
        ) {
            Ok(doc) => assert!(
                !doc.pages.is_empty(),
                "{bad:?}: Ok with no pages (warnings: {warnings:?})"
            ),
            Err(msg) => assert!(!msg.is_empty(), "{bad:?}: an Err with no message"),
        }
    }
}

#[test]
fn plain_html_still_gives_a_page() {
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        "<html><body><p>hello</p></body></html>",
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("from_html");
    assert!(!doc.pages.is_empty());
}
