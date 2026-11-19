//! A footnote is printed at the foot of the page its reference is on.
//!
//! CSS Generated Content for Paged Media (W3C Working Draft), "Footnotes": "An element
//! becomes a footnote by applying `float: footnote` to that element. [...] The footnote
//! element is removed from the flow"; the footnote area is at the bottom of the page.
//! Implemented in azul-layout 0.0.20; pinned here end to end, through `from_html`.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

/// A few lines of text at the top of a page. With `note`, one word carries a footnote.
fn page(note: bool) -> String {
    let footnote = if note {
        r#"<span style="float: footnote; font-size: 9pt;">The note of this page.</span>"#
    } else {
        ""
    };
    format!(
        r#"<html><head><style>
            ::footnote-call {{ content: none; }}
            ::footnote-marker {{ content: none; }}
        </style></head><body style="font-size: 12pt;">
            <p>A short paragraph at the top of the page{footnote} and nothing else below it.</p>
        </body></html>"#
    )
}

/// The lowest baseline on the first page, in points from the bottom edge of the paper.
fn lowest_baseline(html: &str) -> (usize, f32) {
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("the page renders");
    let lowest = doc.pages[0]
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::SetTextMatrix { matrix } => Some(matrix.as_array()[5]),
            _ => None,
        })
        .fold(f32::MAX, f32::min);
    (doc.pages.len(), lowest)
}

#[test]
fn a_footnote_is_printed_at_the_foot_of_the_page_of_its_reference() {
    // Without a note the page's text is a paragraph at the top: nothing in its lower half.
    let (pages, plain) = lowest_baseline(&page(false));
    assert_eq!(pages, 1);
    assert!(plain > 420.0, "a short paragraph stands in the upper half of the page, lowest baseline at {plain}pt");

    // With the note: still one page, and the note's line stands near the bottom edge of the
    // content area, far below the paragraph it was written in.
    let (pages, with_note) = lowest_baseline(&page(true));
    assert_eq!(pages, 1, "the note is on the page of its reference, not on one of its own");
    assert!(
        with_note < 150.0,
        "the note is printed at the foot of the page, but the lowest text is at {with_note}pt from the bottom"
    );
}
