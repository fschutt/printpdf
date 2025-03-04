use printpdf::{ExternalStream, Svg};

#[test]
fn test_op_svg_embed() {
    let svg = include_str!("./tiger.svg");
    let parsed = Svg::parse(svg, &mut Vec::new()).unwrap();
    let parsed = parsed.stream.get_ops().unwrap();
    let parsed = parsed.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>();

    let tigerstream = include_str!("./tiger-svgstream.txt");
    let ops = ExternalStream::decode_ops(&tigerstream).unwrap();
    let ops = ops.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>();

    text_diff::print_diff(&ops.join("\r\n"), &parsed.join("\r\n"), "\r\n");
    panic!();
}