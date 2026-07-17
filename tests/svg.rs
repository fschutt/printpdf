#![cfg(feature = "svg")]

use printpdf::{ExternalStream, Svg};

#[test]
fn test_op_svg_embed() {
    let svg = include_str!("./tiger.svg");
    let parsed = Svg::parse(svg, &mut Vec::new()).unwrap();
    let parsed = parsed.stream.get_ops().unwrap();

    let tigerstream = include_str!("./tiger-svgstream.txt");
    let ops = ExternalStream::decode_ops(&tigerstream).unwrap();

    for i in 0..parsed.len() {
        pretty_assertions::assert_eq!(format!("{:#?}", parsed[i]), format!("{:#?}", ops[i]),);
    }
}

#[test]
fn test_svg_text_with_fonts_passed_from_outside() {
    // `<text>` needs a font database; parse_with_fonts feeds caller-supplied
    // fonts into it (the only font source on wasm, an addition to system fonts
    // elsewhere). With the font supplied, the text must survive conversion as
    // drawing ops instead of being dropped.
    // NOTE: the font's *typographic* family is "Roboto" (its legacy family
    // name "Roboto Medium" is not indexed by fontdb), so that is the name the
    // SVG must use.
    let font = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60">
        <text x="10" y="40" font-family="Roboto" font-size="30">Hji</text>
    </svg>"#;

    let mut fonts = std::collections::BTreeMap::new();
    fonts.insert("roboto-medium".to_string(), font.to_vec());

    let parsed = Svg::parse_with_fonts(svg, &fonts, &mut Vec::new()).unwrap();
    let ops = parsed.stream.get_ops().unwrap();
    // Text is converted to glyph outlines, so real path-drawing ops must be
    // present — an empty SVG still produces q/cm/Q wrapper ops, which is why
    // `!ops.is_empty()` alone proves nothing.
    let path_ops = ops
        .iter()
        .filter(|op| {
            matches!(
                op,
                printpdf::Op::DrawPolygon { .. } | printpdf::Op::DrawLine { .. }
            )
        })
        .count();
    assert!(
        path_ops > 0,
        "SVG text with an externally-supplied font must produce glyph outline ops, got: {:#?}",
        ops
    );
}
