use printpdf::Svg;

#[test]
fn test_op_svg_embed() {
    let svg = include_str!("./tiger.svg");
    let parsed = Svg::parse(svg, &mut Vec::new()).unwrap();
    let parsed = parsed.stream.get_ops().unwrap();
    pretty_assertions::assert_eq!(parsed, vec![

    ])
}