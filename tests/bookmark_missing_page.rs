//! A bookmark that points at a page that does not exist is left out when saving.
//!
//! This happens when the pages a bookmark pointed at are removed from a parsed document.
//! Saving used to panic when no bookmark was left (`Option::unwrap` on an empty list), and
//! wrote an outline `/Count` that included the bookmarks it had left out.

use printpdf::*;

fn document(pages: usize, bookmark_pages: &[usize]) -> PdfDocument {
    let mut doc = PdfDocument::new("bookmarks");
    doc.pages = (0..pages)
        .map(|_| PdfPage::new(Mm(210.0), Mm(297.0), Vec::new()))
        .collect();
    for (i, page) in bookmark_pages.iter().enumerate() {
        doc.add_bookmark(&format!("Chapter {i}"), *page);
    }
    doc
}

fn save(doc: &PdfDocument) -> Vec<u8> {
    doc.save(&PdfSaveOptions::default(), &mut Vec::new())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn saving_with_only_bookmarks_on_missing_pages_does_not_panic() {
    let bytes = save(&document(1, &[5]));

    assert!(!contains(&bytes, b"/Outlines"), "no outline is written");
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
    assert_eq!(parsed.pages.len(), 1);
    assert!(parsed.bookmarks.map.is_empty());
}

#[test]
fn the_outline_counts_only_the_bookmarks_that_were_written() {
    // pages 1 and 2 exist, page 9 does not
    let bytes = save(&document(3, &[1, 2, 9]));

    assert!(contains(&bytes, b"/Outlines"));
    assert!(contains(&bytes, b"/Count 2"), "two bookmarks are in the outline");
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
    let mut pages: Vec<usize> = parsed.bookmarks.map.values().map(|b| b.page).collect();
    pages.sort_unstable();
    assert_eq!(pages, [1, 2]);
}

#[test]
fn bookmarks_on_existing_pages_are_unchanged() {
    let bytes = save(&document(2, &[1, 2]));
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
    assert_eq!(parsed.bookmarks.map.len(), 2);
}
