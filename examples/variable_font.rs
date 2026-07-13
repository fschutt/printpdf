use std::{env, error::Error, fs};

use printpdf::{
    FontVariationSettings, FontVariationTag, Mm, Op, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, TextItem,
};

fn main() -> Result<(), Box<dyn Error>> {
    let font_path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example variable_font -- path/to/variable-font.ttf")?;
    let font_bytes = fs::read(font_path)?;
    let mut warnings = Vec::new();
    let mut document = PdfDocument::new("Variable font instances");

    let light = document.add_variable_font(
        &font_bytes,
        0,
        &FontVariationSettings::new().with(FontVariationTag::WGHT, 300.0),
        &mut warnings,
    )?;
    let bold = document.add_variable_font(
        &font_bytes,
        0,
        &FontVariationSettings::new().with(FontVariationTag::WGHT, 800.0),
        &mut warnings,
    )?;

    let page = PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(20.0), Mm(260.0)),
            },
            Op::SetFont {
                font: PdfFontHandle::External(light),
                size: Pt(24.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Weight 300".into())],
            },
            Op::SetTextCursor {
                pos: Point::new(Mm(20.0), Mm(240.0)),
            },
            Op::SetFont {
                font: PdfFontHandle::External(bold),
                size: Pt(24.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Weight 800".into())],
            },
            Op::EndTextSection,
        ],
    );

    let pdf = document
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);
    fs::write("variable_font.pdf", pdf)?;

    for warning in warnings {
        eprintln!("{}", warning.msg);
    }
    Ok(())
}
