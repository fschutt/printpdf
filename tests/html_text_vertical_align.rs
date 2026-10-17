//! A superscript and a subscript are drawn off their line in the PDF.
//!
//! pdfocr's engine-issue report (issue 1): `<sup>`, `<sub>` and `vertical-align: super`
//! made the line taller, but every run was drawn at the SAME text-matrix y - the shift
//! azul's line layout computes was lost when its compact ("dense") paragraph form was
//! expanded into the positioned glyphs this crate draws. Fixed in azul
//! (`text3::dense`: each run keeps its own solved y); pinned here end to end.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

const PAGE: &str = r#"
    <html>
        <body>
            <p style="font-size: 16px; line-height: 40px;">Cock<sup>a</sup> and x<sub>2</sub></p>
        </body>
    </html>
"#;

/// The distinct baselines (text-matrix y, rounded to 0.1 pt) the page draws text at.
fn text_baselines(doc: &PdfDocument) -> Vec<i64> {
    let mut ys: Vec<i64> = doc
        .pages
        .iter()
        .flat_map(|page| page.ops.iter())
        .filter_map(|op| match op {
            Op::SetTextMatrix { matrix } => Some((matrix.as_array()[5] * 10.0).round() as i64),
            _ => None,
        })
        .collect();
    ys.sort_unstable();
    ys.dedup();
    ys
}

#[test]
fn a_superscript_and_a_subscript_are_drawn_off_the_line_in_the_pdf() {
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        PAGE,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("the page renders");
    let baselines = text_baselines(&doc);
    assert!(
        baselines.len() >= 3,
        "the line's text, the raised \"a\" and the lowered \"2\" sit on three different \
         baselines, got {baselines:?} (in tenths of a point)"
    );
}
