#![cfg(feature = "html")]
//! A CID-keyed CFF font read back from a PDF keeps its glyph widths.
//!
//! A bare CFF program (`/FontFile3 /CIDFontType0C`) has no `hmtx`, so the
//! parser synthesizes one from each charstring's optional leading width
//! operand (`src/font.rs`, `cff_charstring_width`). Whether that operand is
//! there is decided at the first stack-clearing operator, which may sit inside
//! a subroutine. NotoSansJP's 語 starts with 21 stem operands and a
//! `callsubr`; the subroutine pushes one more and runs `hstemhm`: 22 operands,
//! no width, so the advance is `defaultWidthX` (1000). The scanner's
//! "no width here" answer from inside the subroutine read as "subroutine
//! returned", so it kept scanning the caller past the hints, found an odd
//! operand count at the next `cntrmask` and took the first stem coordinate
//! (-30) as the width: 語 got advance 0, so everything that pens through a
//! parsed TJ run with the parsed widths (the SVG preview) drew the rest of the
//! run one em too far left, on top of it.

use std::collections::BTreeMap;

use printpdf::*;

const NOTO_SANS_JP: &[u8] = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");

#[test]
fn parsed_cid_cff_font_keeps_the_advance_of_a_glyph_whose_hints_start_in_a_subroutine() {
    let html = r#"<html><body><p style="font-family:'Noto Sans JP';font-size:20px;">日本語のテキスト</p></body></html>"#;
    let mut fonts = BTreeMap::new();
    fonts.insert(
        "Noto Sans JP".to_string(),
        Base64OrRaw::Raw(NOTO_SANS_JP.to_vec()),
    );
    let mut w = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut w,
    )
    .expect("from_html");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut w);
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut w).expect("parse");

    let mut font = None;
    let mut checked = Vec::new();
    for op in &parsed.pages[0].ops {
        match op {
            Op::SetFont {
                font: PdfFontHandle::External(id),
                ..
            } => {
                font = parsed.resources.fonts.map.get(id).map(|f| &f.parsed_font);
            }
            Op::ShowText { items } => {
                let font = font.expect("an external font is selected");
                for item in items {
                    if let TextItem::GlyphIds(glyphs) = item {
                        for g in glyphs {
                            let text = g.cid.clone().unwrap_or_default();
                            // Every glyph of this text is a full-width CJK glyph.
                            assert_eq!(
                                font.get_horizontal_advance(g.gid),
                                1000,
                                "{text:?} (gid {}) lost its advance width in the parsed font",
                                g.gid
                            );
                            checked.push(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(checked.concat(), "日本語のテキスト");
}
