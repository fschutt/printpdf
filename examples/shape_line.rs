//! Shapes lines of text in code, without HTML, and draws them.
//!
//! `shape_line` turns a string into positioned glyphs with the font's kerning, ligatures and
//! mark positions, and returns the width of the line. `show_placed_glyphs` draws them. The
//! example writes `shape_line.pdf` with:
//!
//! - a Latin line, and the same line again with its right end at the right margin, which the
//!   measured width makes possible;
//! - an Arabic line, joined and in right-to-left order;
//! - a mixed line: an Arabic sentence with a Latin word inside, each part shaped in its own
//!   font and placed in right-to-left order.
//!
//! It needs the `text_layout` feature and not `html`:
//!
//! ```text
//! cargo run --example shape_line --no-default-features --features text_layout
//! ```

use printpdf::*;
use std::{error::Error, fs};

const LATIN_FONT: &[u8] = include_bytes!("assets/fonts/RobotoMedium.ttf");
const ARABIC_FONT: &[u8] = include_bytes!("../tests/assets/fonts/arabic/Vazirmatn-ArabicTest.ttf");

const LEFT: f32 = 20.0; // mm
const RIGHT: f32 = 190.0; // mm

/// Operations that draw `glyphs` with their baseline start at (`x`, `y`) in mm.
fn draw(
    fonts: &[(FontId, &ParsedFont)],
    size: Pt,
    glyphs: &[PlacedGlyph],
    x: Pt,
    y: Mm,
    text: &str,
) -> Vec<Op> {
    let azul: Vec<_> = fonts.iter().map(|(id, f)| (id.clone(), f.as_azul())).collect();
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetTextMatrix { matrix: TextMatrix::Translate(x, Pt::from(y)) },
    ];
    ops.extend(show_placed_glyphs(&azul, size, glyphs, Some(text)));
    ops.push(Op::EndTextSection);
    ops
}

/// A small label in a built-in font.
fn label(text: &str, y: Mm) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor { pos: Point::new(Mm(LEFT), y) },
        Op::SetFont { font: PdfFontHandle::Builtin(BuiltinFont::Helvetica), size: Pt(9.0) },
        Op::ShowText { items: vec![TextItem::Text(text.to_string())] },
        Op::EndTextSection,
    ]
}

fn main() -> Result<(), Box<dyn Error>> {
    let latin = ParsedFont::from_bytes(LATIN_FONT, 0, &mut Vec::new()).ok_or("Latin font")?;
    let arabic = ParsedFont::from_bytes(ARABIC_FONT, 0, &mut Vec::new()).ok_or("Arabic font")?;

    let mut doc = PdfDocument::new("shape_line");
    let latin_id = doc.add_font(&latin);
    let arabic_id = doc.add_font(&arabic);
    let size = Pt(24.0);
    let mut ops = Vec::new();

    // 1. Latin: kerning (AV, To), the fi and ffl ligatures
    let text = "AV To office fi ffl";
    let line = shape_line(&latin, text, size)?;
    println!("Latin: {} glyphs, {:.1} pt wide", line.glyphs.len(), line.width.0);
    ops.extend(label("Latin, left aligned and right aligned by its measured width", Mm(270.0)));
    let latin_fonts = [(latin_id.clone(), &latin)];
    ops.extend(draw(&latin_fonts, size, &line.glyphs, Mm(LEFT).into(), Mm(255.0), text));
    let right_edge: Pt = Mm(RIGHT).into();
    ops.extend(draw(&latin_fonts, size, &line.glyphs, right_edge - line.width, Mm(240.0), text));

    // 2. Arabic: joined forms, the lam-alef ligature, right-to-left order
    let text = "مرحبا بالعالم لا";
    let line = shape_line(&arabic, text, size)?;
    println!("Arabic: {} glyphs, {:.1} pt wide", line.glyphs.len(), line.width.0);
    ops.extend(label("Arabic, shaped right to left", Mm(215.0)));
    let arabic_fonts = [(arabic_id.clone(), &arabic)];
    ops.extend(draw(&arabic_fonts, size, &line.glyphs, right_edge - line.width, Mm(200.0), text));

    // 3. Mixed direction: the sentence reads right to left, "Roboto" left to right inside it.
    // Each part is shaped in its own font; the parts are put side by side in visual order, the
    // last word of the sentence at the left.
    let (first, middle, last) = ("مرحبا ", "Roboto", " بالعالم");
    let text = format!("{first}{middle}{last}");
    let parts = [
        (shape_line(&arabic, last, size)?, 1),
        (shape_line(&latin, middle, size)?, 0),
        (shape_line(&arabic, first, size)?, 1),
    ];
    let mut glyphs = Vec::new();
    let mut pen = 0.0f32;
    for (part, font) in &parts {
        for g in &part.glyphs {
            glyphs.push(PlacedGlyph { font: *font, x: g.x + pen, ..g.clone() });
        }
        pen += part.width.0;
    }
    println!("Mixed: {} glyphs, {pen:.1} pt wide", glyphs.len());
    ops.extend(label("Mixed: an Arabic sentence with a Latin word, two fonts", Mm(175.0)));
    let both = [(latin_id, &latin), (arabic_id, &arabic)];
    ops.extend(draw(&both, size, &glyphs, right_edge - Pt(pen), Mm(160.0), &text));

    let mut warnings = Vec::new();
    let pdf = doc
        .with_pages(vec![PdfPage::new(Mm(210.0), Mm(297.0), ops)])
        .save(&PdfSaveOptions::default(), &mut warnings);
    fs::write("shape_line.pdf", pdf)?;
    for warning in warnings {
        eprintln!("{}", warning.msg);
    }
    println!("Wrote shape_line.pdf");
    Ok(())
}
