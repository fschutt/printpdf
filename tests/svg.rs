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
