extern crate printpdf;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf circle test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let radius_1 = Pt(40.0);
    let radius_2 = Pt(30.0);
    let offset_x = Pt(10.0);
    let offset_y = Pt(50.0);

    let line = Polygon {
        rings: vec![
            calculate_points_for_circle(radius_1, offset_x, offset_y),
            calculate_points_for_circle(radius_2, offset_x, offset_y), // hole
        ],
        mode: PaintMode::FillStroke,
        winding_order: WindingOrder::EvenOdd,
    };

    current_layer.add_polygon(line);

    let scale_x_rect = Pt(40.0);
    let scale_y_rect = Pt(10.0);
    let offset_x_rect = Pt(20.0);
    let offset_y_rect = Pt(5.0);

    let line = Polygon {
        rings: vec![calculate_points_for_rect(
            scale_x_rect,
            scale_y_rect,
            offset_x_rect,
            offset_y_rect,
        )],
        mode: PaintMode::FillStroke,
        winding_order: WindingOrder::NonZero,
    };

    current_layer.add_polygon(line);

    doc.save(&mut BufWriter::new(
        File::create("test_circle.pdf").unwrap(),
    ))
    .unwrap();
}
