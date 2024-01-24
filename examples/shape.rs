extern crate printpdf;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(400.0), Mm(400.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Quadratic shape. The "false" determines if the next (following)
    // point is a bezier handle (for curves)
    // If you want holes, simply reorder the winding of the points to be
    // counterclockwise instead of clockwise.
    let points1 = vec![
        (Point::new(Mm(100.0), Mm(100.0)), false),
        (Point::new(Mm(100.0), Mm(200.0)), false),
        (Point::new(Mm(300.0), Mm(200.0)), false),
        (Point::new(Mm(300.0), Mm(100.0)), false),
    ];

    // Is the shape stroked? Is the shape closed? Is the shape filled?
    let line1 = Line {
        points: points1.clone(),
        is_closed: true,
    };

    let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
    let dash_pattern = LineDashPattern {
        dash_1: Some(20),
        ..Default::default()
    };

    // Draw first line
    current_layer.set_outline_color(outline_color);
    current_layer.set_outline_thickness(10.0);
    current_layer.add_line(line1);

    // Triangle shape
    // Note: Line is invisible by default, the previous method of
    // constructing a line is recommended!
    let line2 = Polygon {
        rings: vec![vec![
            (Point::new(Mm(150.0), Mm(150.0)), false),
            (Point::new(Mm(150.0), Mm(250.0)), false),
            (Point::new(Mm(350.0), Mm(250.0)), false),
        ]],
        mode: PaintMode::FillStroke,
        winding_order: WindingOrder::NonZero,
    };

    let fill_color_2 = Color::Cmyk(Cmyk::new(0.0, 0.0, 0.0, 0.0, None));
    let outline_color_2 = Color::Greyscale(Greyscale::new(0.45, None));

    // More advanced graphical options
    current_layer.set_overprint_stroke(true);
    current_layer.set_blend_mode(BlendMode::Seperable(SeperableBlendMode::Multiply));
    current_layer.set_line_dash_pattern(dash_pattern);
    current_layer.set_line_cap_style(LineCapStyle::Round);
    current_layer.set_line_join_style(LineJoinStyle::Round);
    current_layer.set_fill_color(fill_color_2);
    current_layer.set_outline_color(outline_color_2);
    current_layer.set_outline_thickness(15.0);

    // draw second line
    current_layer.add_polygon(line2);

    // quad clip - note: FIRST SET THE CLIP, then paint the path
    current_layer.save_graphics_state();
    let line4 = Polygon {
        rings: vec![points1.clone()],
        mode: PaintMode::Clip,
        winding_order: WindingOrder::NonZero,
    };

    current_layer.add_polygon(line4);

    let points5 = vec![
        (Point::new(Mm(150.0), Mm(150.0)), false),
        (Point::new(Mm(150.0), Mm(250.0)), false),
        (Point::new(Mm(350.0), Mm(250.0)), false),
    ];

    let line3 = Line {
        points: points5.clone(),
        is_closed: true,
    };

    let outline_color2 = Color::Rgb(Rgb::new(1.0, 0.75, 0.0, None));
    current_layer.set_line_dash_pattern(LineDashPattern::default());
    current_layer.set_outline_color(outline_color2);
    current_layer.set_outline_thickness(5.0);
    current_layer.add_line(line3);

    current_layer.restore_graphics_state(); // unset clip again for further operations

    // If this is successful, you should see a PDF two shapes, one rectangle
    // and a dotted line
    doc.save(&mut BufWriter::new(File::create("test_graphics.pdf").unwrap()))
        .unwrap();
}
