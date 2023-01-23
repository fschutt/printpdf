extern crate printpdf;

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) =
        PdfDocument::new("printpdf circle test", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let radius = Pt(40.0);
    let offset_x = Pt(10.0);
    let offset_y = Pt(50.0);

    let line = Line {
        points: calculate_points_for_circle(radius, offset_x, offset_y),
        is_closed: true,
        has_fill: true,
        has_stroke: true,
        is_clipping_path: false,
    };

    current_layer.add_shape(line);

    let scale_x_rect = Pt(40.0);
    let scale_y_rect = Pt(10.0);
    let offset_x_rect = Pt(20.0);
    let offset_y_rect = Pt(5.0);

    let line = Line {
        points: calculate_points_for_rect(scale_x_rect, scale_y_rect, offset_x_rect, offset_y_rect),
        is_closed: true,
        has_fill: true,
        has_stroke: true,
        is_clipping_path: false,
    };

    current_layer.add_shape(line);

    doc.save(&mut BufWriter::new(
        File::create("test_circle.pdf").unwrap(),
    ))
    .unwrap();
}
