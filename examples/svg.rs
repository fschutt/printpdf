//! Embeds the SVG image on the page
extern crate printpdf;

use printpdf::*;

const SVG: &str = include_str!("../assets/svg/tiger.svg");

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);
    let svg = Svg::parse(SVG).unwrap();

    let rotation_center_x = Px((svg.width.0 as f32 / 2.0) as usize);
    let rotation_center_y = Px((svg.height.0 as f32 / 2.0) as usize);

    let reference = svg.into_xobject(&current_layer);

    for i in 0..10 {
        reference.clone().add_to_layer(
            &current_layer,
            SvgTransform {
                rotate: Some(SvgRotation {
                    angle_ccw_degrees: i as f32 * 36.0,
                    rotation_center_x: rotation_center_x.into_pt(300.0),
                    rotation_center_y: rotation_center_y.into_pt(300.0),
                }),
                translate_x: Some(Mm(i as f32 * 20.0 % 50.0).into()),
                translate_y: Some(Mm(i as f32 * 30.0).into()),
                ..Default::default()
            },
        );
    }

    let pdf_bytes = doc.save_to_bytes().unwrap();
    std::fs::write("test_svg.pdf", &pdf_bytes).unwrap();
}
