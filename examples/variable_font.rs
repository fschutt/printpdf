//! One variable font, two weights.
//!
//! A PDF cannot hold a variable font, so printpdf creates a static font at the axis values you
//! choose and embeds that. This example reads the axes of a font, creates a light and a bold
//! instance and writes `variable_font.pdf`.
//!
//! Run it with the bundled font, or pass the path of another variable font:
//!
//! ```text
//! cargo run --example variable_font
//! cargo run --example variable_font -- path/to/variable-font.ttf
//! ```

use printpdf::{
    font_variation_axes, FontVariationSettings, FontVariationTag, Mm, Op, PdfDocument,
    PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, TextItem,
};
use std::{env, error::Error, fs};

const BUNDLED: &[u8] =
    include_bytes!("../tests/assets/variable-fonts/RedHatMono-VariableFont_wght.ttf");

fn main() -> Result<(), Box<dyn Error>> {
    let font_bytes = match env::args().nth(1) {
        Some(path) => fs::read(path)?,
        None => BUNDLED.to_vec(),
    };

    let axes = font_variation_axes(&font_bytes, 0)?;
    let weight = axes
        .iter()
        .find(|axis| axis.tag == FontVariationTag::WGHT)
        .ok_or("the font has no wght axis")?;
    println!("wght axis: {} to {}, default {}", weight.min, weight.max, weight.default);

    let mut doc = PdfDocument::new("Variable font instances");
    let light = doc.add_variable_font(
        &font_bytes,
        0,
        &FontVariationSettings::new().with(FontVariationTag::WGHT, weight.min),
    )?;
    let bold = doc.add_variable_font(
        &font_bytes,
        0,
        &FontVariationSettings::new().with(FontVariationTag::WGHT, weight.max),
    )?;

    let line = |font, y: f32, text: &str| {
        vec![
            Op::StartTextSection,
            Op::SetTextCursor { pos: Point::new(Mm(20.0), Mm(y)) },
            Op::SetFont { font: PdfFontHandle::External(font), size: Pt(24.0) },
            Op::ShowText { items: vec![TextItem::Text(text.to_string())] },
            Op::EndTextSection,
        ]
    };
    let mut ops = line(light, 260.0, &format!("Weight {}", weight.min));
    ops.extend(line(bold, 240.0, &format!("Weight {}", weight.max)));

    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), ops)])
        .save(&PdfSaveOptions::default(), &mut warnings);
    fs::write("variable_font.pdf", pdf)?;

    for warning in warnings {
        eprintln!("{}", warning.msg);
    }
    Ok(())
}
