//! A justified line that a `<br>` ends is not spread to the width of its box.
//!
//! CSS Text 3, section 6.3 (`text-align-last`): "the last line of a block or a line
//! right before a forced line break" is aligned by `text-align-last`, and under its
//! initial value `auto` a justified block sets such a line from the start edge. Verses
//! set as one paragraph with a `<br>` after each line (a hymn, a poem) were printed
//! with every short line pulled out to both edges. Fixed in azul-layout 0.0.19 (`text3`: a line
//! is justified by its own resolved alignment); pinned here end to end.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

/// The box is 400px wide (300pt). A spread line puts its last word at the right edge;
/// an unspread short line stays in the left third.
const VERSES: &str = r#"
    <html><body style="margin: 0; font-size: 12pt;">
        <p style="text-align: justify; width: 400px;">One two<br/>three four<br/>five six<br/>seven.</p>
    </body></html>
"#;

/// The same box with text that wraps by itself: its lines, but for the last, are spread.
const PROSE: &str = r#"
    <html><body style="margin: 0; font-size: 12pt;">
        <p style="text-align: justify; width: 400px;">This paragraph has no forced break and is long
        enough to wrap over several lines, so that all of its lines but the last one are spread to the
        full width of the box, word by word, from the left edge to the right edge.</p>
    </body></html>
"#;

/// The x (in points) at which the rightmost piece of text on the page starts.
fn rightmost_text_start(html: &str) -> f32 {
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("the page renders");
    doc.pages
        .iter()
        .flat_map(|page| page.ops.iter())
        .filter_map(|op| match op {
            Op::SetTextMatrix { matrix } => Some(matrix.as_array()[4]),
            _ => None,
        })
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_justified_line_before_a_forced_break_is_not_spread() {
    // The measure sees spreading: a wrapped line has a word that starts in the right half.
    let prose = rightmost_text_start(PROSE);
    assert!(prose > 150.0, "a wrapped justified line reaches the right half of its box, got {prose}pt");

    // Two short words and a forced break: nothing starts beyond the left third.
    let verses = rightmost_text_start(VERSES);
    assert!(
        verses < 100.0,
        "a line that `<br>` ends is set from the start edge, but a word starts at {verses}pt of 300pt"
    );
}
