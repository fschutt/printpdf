#![cfg(feature = "text_layout")]
//! The crashes of issue #292 that run in this crate without a huge allocation: arithmetic on
//! sizes and units, glyph-id widths at `u16::MAX`, and images that claim an absurd size. Each
//! must return a value, not panic (an overflow check panics in debug and in tests built with
//! `overflow-checks`).

use printpdf::*;

const FONT: &[u8] = include_bytes!("assets/variable-fonts/RedHatMono-VariableFont_wght.ttf");

fn font() -> ParsedFont {
    ParsedFont::from_bytes(FONT, 0, &mut Vec::new()).expect("fixture font parses")
}

#[test]
fn px_arithmetic_saturates() {
    assert_eq!(Px(0) - Px(1), Px(0));
    assert_eq!(Px(usize::MAX) + Px(1), Px(usize::MAX));
}

#[test]
fn nan_units_are_equal_to_themselves() {
    let mm = Mm(f32::NAN);
    assert!(mm == mm, "Eq must be reflexive");
    let pt = Px(0).into_pt(0.0);
    assert!(pt == pt, "Eq must be reflexive");
    let px = Px(0) - Pt::default().into_px(0.0);
    assert!(px.into_pt(0.0) == px.into_pt(0.0));
}

#[test]
fn widths_of_glyph_ids_at_the_end_of_the_range() {
    let font = font();
    let _ = get_normalized_widths_ttf(&font, &[(u16::MAX, String::new()), (0, String::new())]);
    let _ = get_normalized_widths_codes(&font, &[(u16::MAX, 0), (0, 0)]);
}

#[cfg(feature = "images")]
#[test]
fn images_with_an_impossible_size_do_not_overflow() {
    let mut huge = RawImage::empty(1_000_000_000_000_000_000, 10_000, RawImageFormat::R8);
    let _ = huge.optimize(&ImageOptimizationOptions::default());

    let mut tall = RawImage::empty(0, 1_000_000_000_000_000_000, RawImageFormat::R16);
    let options = ImageOptimizationOptions {
        dither_greyscale: Some(true),
        ..Default::default()
    };
    let _ = tall.optimize(&options);
}
