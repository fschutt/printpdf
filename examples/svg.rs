//! Embeds the SVG image on the page
extern crate printpdf;

use printpdf::*;

const SVG: &str = include_str!("../assets/svg/tiger.svg");

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);
    let svg = Svg::parse(SVG).unwrap();
    svg.add_to_layer(current_layer.clone(), SvgTransform::default());

    let pdf_bytes = doc.save_to_bytes().unwrap();
    std::fs::write("test_svg.pdf", &pdf_bytes).unwrap();
}