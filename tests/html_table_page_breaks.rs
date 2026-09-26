#![cfg(feature = "html")]
//! Regression test for fschutt/azul#478 — "Page-break placement regressed:
//! pages break ~1/3 early, capacity itself is unchanged".
//!
//! A single `<table>` of identical single-line rows on a default A4 page, no
//! `break-*` / `page-break-*` CSS at all (the reporter's fixture, verbatim).
//! With printpdf 0.12.6 a 46-row table paginated as 45 + 1; with 0.12.7/0.12.8
//! (break-aware pagination ON, `layout_document_paged_v2`) the same table
//! came out as 29 + 17 — every page except the last was cut about a third
//! short, although 45 rows still fit on one page when nothing has to break.
//!
//! The test calibrates the page capacity itself (the largest row count that
//! still fits on ONE page, found by bisection) so it does not depend on the
//! platform's `sans-serif` face, and then checks that once the table has to
//! break, every non-final page is still filled to capacity.
//!
//! Cause: azul-layout 0.0.16 widens an overflow-visible node's text clip to
//! its "scroll content size", which is floored at the BORDER box, from the
//! content-box origin (`solver3/display_list.rs`), so a `td { padding: 3px }`
//! clip runs 3px into the next row and every row's keep-together range
//! overlaps the next. `page_breaks::snap_break_up` then took each overlap as
//! nesting: after snapping to a row's top the break was "inside" the previous
//! row, snapped again, and climbed row by row until the `max_push_distance`
//! budget (0.33 × page) was spent. Both are fixed in azul: only a range that
//! ENCLOSES the range just left may pull the break further up, and a box's
//! own padding no longer counts as overflow. The tests are `#[ignore]`d while
//! the pinned azul-layout (0.0.16) still has the defects; run them with
//! `--ignored` against a patched azul-layout to verify, and un-ignore on the
//! bump.

use std::collections::BTreeMap;

use printpdf::*;

/// The reporter's `rows-046.html`, parametrised over the row count.
fn report_html(rows: usize) -> String {
    let mut body = String::new();
    for i in 0..rows {
        body.push_str(&format!(
            "<tr><td class=\"al\">Row {i:04}</td><td class=\"ar\">1,234.56</td><td class=\"ar\"></td></tr>\n"
        ));
    }
    format!(
        r#"<!DOCTYPE html><html><head><style>
body {{ font-family: sans-serif; color: #000; margin: 0; padding: 0; }}
h1 {{ font-size: 18pt; font-weight: bold; text-align: center; margin: 0 0 4px 0; }}
.subtitle {{ font-size: 12pt; color: #646464; text-align: center; margin: 0 0 2px 0; }}
.company {{ font-size: 11pt; text-align: center; margin: 0 0 12px 0; }}
table {{ width: 100%; border-collapse: collapse; margin-top: 8px; }}
th {{ font-size: 10pt; font-weight: bold; padding: 4px 6px; border-bottom: 1.5px solid #000; }}
td {{ font-size: 9pt; padding: 3px 6px; }}
.al {{ text-align: left; }}
.ar {{ text-align: right; }}
</style></head><body>
<h1>Sample Report</h1>
<p class="subtitle">Fixed fixture</p>
<p class="company">Example Co</p>
<table>
<thead><tr>
<th class="al" style="width:40.0%">Account</th>
<th class="ar" style="width:30.0%">Debit</th>
<th class="ar" style="width:30.0%">Credit</th>
</tr></thead>
<tbody>
{body}</tbody>
</table>
</body></html>"#
    )
}

fn render(rows: usize) -> PdfDocument {
    let mut warnings = Vec::new();
    PdfDocument::from_html(
        &report_html(rows),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .unwrap_or_else(|e| panic!("from_html({rows} rows) failed: {e:?}"))
}

/// Every character a page shows, in op order. The HTML bridge emits one
/// `ShowText` per glyph with the source codepoint in `Codepoint::cid`
/// (external fonts) or a plain `TextItem::Text` run (built-in fonts); both
/// are decoded here.
fn page_text(page: &PdfPage) -> String {
    let mut text = String::new();
    for op in &page.ops {
        let Op::ShowText { items } = op else { continue };
        for item in items {
            match item {
                TextItem::Text(t) => text.push_str(t),
                TextItem::GlyphIds(glyphs) => {
                    for g in glyphs {
                        if let Some(cid) = &g.cid {
                            text.push_str(cid);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    text
}

/// Data rows per page: each `<tr>` carries exactly one "Row NNNN" cell.
/// (Whitespace glyphs may be dropped, so match "Row", not "Row ".)
fn rows_per_page(doc: &PdfDocument) -> Vec<usize> {
    doc.pages
        .iter()
        .map(|p| page_text(p).matches("Row").count())
        .collect()
}

/// The largest row count that still renders as a single page — found by
/// bisection so the test holds for whatever `sans-serif` resolves to.
fn single_page_capacity() -> usize {
    let mut fits = 1;
    let mut overflows = 400;
    assert_eq!(render(fits).pages.len(), 1, "one row must fit on a page");
    assert!(render(overflows).pages.len() > 1, "400 rows must overflow A4");
    while overflows - fits > 1 {
        let mid = (fits + overflows) / 2;
        if render(mid).pages.len() == 1 {
            fits = mid;
        } else {
            overflows = mid;
        }
    }
    fits
}

#[test]
#[ignore = "azul#478: fixed in azul-layout's page_breaks::snap_break_up after 0.0.16; drop this once the azul-* pins move past it"]
fn azul_issue_478_overflowing_table_fills_the_first_page() {
    let capacity = single_page_capacity();
    assert!(
        capacity >= 20,
        "sanity: A4 should hold at least 20 nine-point rows, got {capacity}"
    );

    // One row more than fits: the break must land right where the page runs
    // out (the last row that no longer fits moves), NOT a third of a page up.
    let doc = render(capacity + 1);
    let rows = rows_per_page(&doc);
    assert_eq!(rows.iter().sum::<usize>(), capacity + 1, "every row is rendered once: {rows:?}");
    assert_eq!(rows.len(), 2, "one overflowing row means exactly two pages: {rows:?}");
    assert!(
        rows[0] >= capacity - 1,
        "azul#478: {} rows fit on one page, yet the break for {} rows was placed \
         after only {} rows (pages: {rows:?})",
        capacity,
        capacity + 1,
        rows[0]
    );
}

#[test]
#[ignore = "azul#478: fixed in azul-layout's page_breaks::snap_break_up after 0.0.16; drop this once the azul-* pins move past it"]
fn azul_issue_478_long_table_fills_every_non_final_page() {
    let capacity = single_page_capacity();
    let total = 120;

    let doc = render(total);
    let rows = rows_per_page(&doc);
    assert_eq!(rows.iter().sum::<usize>(), total, "every row is rendered once: {rows:?}");
    assert!(rows.len() >= 2, "120 rows must span several pages: {rows:?}");

    // Continuation pages carry a repeated <thead> but no title block, so each
    // holds at least as many rows as the first page did on its own.
    for (i, &n) in rows[..rows.len() - 1].iter().enumerate() {
        assert!(
            n >= capacity - 1,
            "azul#478: page {} holds only {n} rows although {capacity} fit on a page \
             (pages: {rows:?})",
            i + 1
        );
    }
}
