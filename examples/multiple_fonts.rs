//! Same document as `otf-font.rs`, saved with `subset_fonts: true`.
//!
//! Writes `font_subset.pdf`. CI validates both PDFs with
//! `scripts/verify_pdf_font.py` (fontTools) — the subset path exercises the
//! allsorts CFF subsetter, whose subset charset keeps the ORIGINAL font's CIDs,
//! so the Identity-H codes must be those CIDs, not the renumbered gids (#280).

use printpdf::*;

fn main() {
    let mut doc = PdfDocument::new("Demo of multiple fonts, subsetted");

    let font_slice = include_bytes!("./assets/fonts/SourceHanSerif.ttc");

    // One group per language: each face in the collection cycles JA/KR/TC/SC,
    // so offset `i` (0..4) selects the language and stepping by 4 walks its faces.
    let texts = [
        "新しい時代のこころを映すタイプフェイスデザイン",
        "동해 물과 백두산이 마르고 닳도록 하느님이 보우하사 우리나라 만세.",
        "這句話後來演變成「飲水思源」這個成語，意為喝水的時候想一想流水的源頭，比喻不忘本。",
        "这句话后来演变成“饮水思源”这个成语，意为喝水的时候想一想流水的源头，比喻不忘本。",
    ];

    let font_groups: Vec<Vec<(FontId, String)>> = (0..4)
        .map(|offset| {
            (offset..28)
                .step_by(4)
                .map(|i| {
                    let parsed_font = ParsedFont::from_bytes(font_slice, i, &mut vec![]).unwrap();
                    let name = parsed_font
                        .font_name
                        .clone()
                        .unwrap_or_else(|| "Undefined".to_string());
                    (doc.add_font(&parsed_font), name)
                })
                .collect()
        })
        .collect();

    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Mm(10.0).into(),
                y: Mm(285.0).into(),
            },
        },
        Op::SetLineHeight { lh: Pt(14.0) },
        Op::SetCharacterSpacing { multiplier: 0.0 },
    ];

    for (i, (group, text)) in font_groups.iter().zip(texts).enumerate() {
        for (font_id, font_name) in group {
            ops.push(Op::SetFont {
                font: PdfFontHandle::External(font_id.clone()),
                size: Pt(12.0),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(font_name.clone())],
            });
            ops.push(Op::AddLineBreak);
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            });
            ops.push(Op::AddLineBreak);
        }
        if i + 1 < font_groups.len() {
            ops.push(Op::SetLineHeight { lh: Pt(8.0) });
            ops.push(Op::AddLineBreak);
            ops.push(Op::SetLineHeight { lh: Pt(14.0) });
        }
    }

    ops.push(Op::SetTextCursor {
        pos: Point {
            x: Mm(120.0).into(),
            y: Mm(255.0).into(),
        },
    });
    for group in &font_groups {
        ops.push(Op::SetFont {
            font: PdfFontHandle::External(group[2].0.clone()),
            size: Pt(24.0),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text("\u{66DC}".to_string())],
        });
    }
    ops.push(Op::EndTextSection);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops.to_vec());
    let pages = vec![page];

    let mut options = PdfSaveOptions::default();
    options.subset_fonts = true;
    options.optimize = false;

    let mut warnings = vec![];
    let bytes = doc.with_pages(pages).save(&options, &mut warnings);

    std::fs::write("./multiple_fonts.pdf", bytes).unwrap();

    for warning in warnings {
        if warning.severity != PdfParseErrorSeverity::Info {
            println!("{:#?}", warning);
        }
    }
}
