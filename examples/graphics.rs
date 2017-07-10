extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", 210.0, 297.0, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Quadratic shape. The "false" determines if the next (following) 
    // point is a bezier handle (for curves)
    // If you want holes, simply reorder the winding of the points to be 
    // counterclockwise instead of clockwise.
    let points1 = vec![(Point::new(100.0, 100.0), false),
                       (Point::new(100.0, 200.0), false),
                       (Point::new(300.0, 200.0), false),
                       (Point::new(300.0, 100.0), false)];
    
    // Is the shape stroked? Is the shape closed? Is the shape filled?
    let line1 = Line::new(points1, true, true, true);
    
    // Triangle shape
    let points2 = vec![(Point::new(150.0, 150.0), false),
                       (Point::new(150.0, 250.0), false),
                       (Point::new(350.0, 250.0), false)];
    let line2 = Line::new(points2, true, false, false);

    let fill_color = Color::Cmyk(Cmyk::new(0.0, 0.23, 0.0, 0.0, None));
    let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
    let mut dash_pattern = LineDashPattern::default();
    dash_pattern.dash_1 = Some(20);
    
    current_layer.set_fill_color(fill_color);
    current_layer.set_outline_color(outline_color);
    current_layer.set_outline_thickness(10);

    // Draw first line
    current_layer.add_shape(line1);
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
    current_layer.set_outline_thickness(15);

    // draw second line
    current_layer.add_shape(line2);

    // If this is successful, you should see a PDF two shapes, one rectangle
    // and a dotted line 
    doc.save(&mut File::create("test_graphics.pdf").unwrap()).unwrap();
}