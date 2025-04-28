use printpdf::*;

fn main() {
    let mut doc = PdfDocument::new("TEST");

    let font_slice = include_bytes!("./assets/fonts/NotoSansJP-Regular.otf");

    let parsed_font = ParsedFont::from_bytes(font_slice, 0, &mut vec![]).unwrap();
    let font_id = doc.add_font(&parsed_font);

    let texts = [
        "日本語 中国語 韓国語",
        "012 abc ABC XYZ   @#$",
        "中文［sighs］",
        "中文Aah!",
        "中文HOME Sweet Home.",
        "中文Well, time for breakfast.",
        "中文［sniffing］",
        "中文［sighing］ Ah!",
    ];

    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point {
                x: Mm(10.0).into(),
                y: Mm(240.0).into(),
            },
        },
        Op::SetLineHeight { lh: Pt(24.0) },
        Op::SetCharacterSpacing { multiplier: 0.0 },
    ];

    let text_ops = texts
        .map(|text| {
            {
                vec![
                    Op::SetFontSize {
                        size: Pt(16.0),
                        font: font_id.clone(),
                    },
                    Op::WriteText {
                        items: vec![TextItem::Text(text.to_string())],
                        font: font_id.clone(),
                    },
                    Op::AddLineBreak,
                ]
            }
        })
        .concat();

    ops.extend_from_slice(&text_ops);
    ops.extend_from_slice(&[Op::EndTextSection]);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops.to_vec());
    let pages = vec![page];

    let mut options = PdfSaveOptions::default();
    options.subset_fonts = false;
    options.optimize = false;

    let mut warnings = vec![];
    let bytes = doc.with_pages(pages).save(&options, &mut warnings);

    std::fs::write("./font.pdf", bytes).unwrap();

    for warning in warnings {
        if warning.severity != PdfParseErrorSeverity::Info {
            println!("{:#?}", warning);
        }
    }
}
