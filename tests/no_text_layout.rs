//! Font embedding with `default-features = false` (i.e. without `text_layout`).
//!
//! Issue #258: "external fonts + `default-features = false` show no Text in PDF Reader".
//!
//! Without `text_layout` there is no azul-layout, so printpdf falls back to its own
//! `ParsedFont`. That fallback used to be a shell — it retained the font bytes but left
//! `codepoint_to_glyph` empty, so `lookup_glyph_index` returned `None` for every
//! character, every glyph in the content stream came out as `.notdef`, and the page was
//! blank. The font *was* embedded, which is why this looked so puzzling: `pdffonts` was
//! perfectly happy, there just wasn't any text.
//!
//! These tests only run in the no-`text_layout` configuration, which is the only one where
//! the fallback is compiled at all:
//!
//! ```sh
//! cargo test --no-default-features --test no_text_layout
//! ```

#![cfg(not(feature = "text_layout"))]

use printpdf::{
    ops::PdfFontHandle,
    units::{Mm, Pt},
    Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, TextItem,
};

const ROBOTO_TTF: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// The fallback parser must actually read the font, not hand back a blank shell.
#[test]
fn fallback_parser_reads_the_cmap_and_metrics() {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("font must parse");

    // The whole of #258: no cmap => every glyph is .notdef => blank page.
    for c in "Roboto".chars() {
        let gid = font
            .lookup_glyph_index(c as u32)
            .unwrap_or_else(|| panic!("no glyph for {c:?} — the cmap was not parsed (#258)"));
        assert_ne!(gid, 0, "{c:?} maps to .notdef");
    }

    // Roboto is a 2048-upm font. A hardcoded 1000 would silently mis-scale every width.
    assert_eq!(
        font.units_per_em, 2048,
        "units_per_em was not read from the head table"
    );
    assert!(
        font.font_metrics.ascent > 0 && font.font_metrics.descent < 0,
        "hhea metrics were not read: {:?}",
        font.font_metrics
    );

    // Advance widths must come from hmtx, not be absent.
    let gid = font.lookup_glyph_index('o' as u32).unwrap();
    let width = font.glyph_widths.get(&gid).copied().unwrap_or(0);
    assert!(width > 0, "glyph {gid} has no advance width (hmtx not read)");
}

/// End-to-end: the text must reach the content stream as real glyph ids.
#[test]
fn external_font_text_is_not_all_notdef() {
    let font = ParsedFont::from_bytes(ROBOTO_TTF, 0, &mut Vec::new()).expect("parse");
    let mut doc = PdfDocument::new("no-text-layout");
    let font_id = doc.add_font(&font);

    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::External(font_id),
                    size: Pt(24.0),
                },
                Op::ShowText {
                    items: vec![TextItem::Text("Roboto".to_string())],
                },
                Op::EndTextSection,
            ],
        )])
        .save(
            &PdfSaveOptions {
                subset_fonts: false,
                optimize: false,
                ..Default::default()
            },
            &mut warnings,
        );

    // The font program still has to be embedded (that half always worked).
    assert!(
        pdf.len() > 100_000,
        "PDF is only {} bytes — the font program is missing",
        pdf.len()
    );

    // And the glyph ids must not all be zero. Six characters, six big-endian u16 gids,
    // written as a hex string: `<0036005300460053005800 53>` and so on. If the cmap were
    // missing, every one of them would be `0000`.
    let body = String::from_utf8_lossy(&pdf);
    assert!(
        !body.contains("<000000000000000000000000>"),
        "every glyph in the content stream is .notdef — the cmap was not parsed (#258)"
    );
}
