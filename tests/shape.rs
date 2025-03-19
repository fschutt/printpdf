use printpdf::*;

#[test]
fn test_shape_text_simple() {
    let font_bytes = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).unwrap();

    let options = TextShapingOptions {
        font_size: Pt(12.0),
        ..Default::default()
    };

    let shaped_text = shape_text("Hello World", &font, &options);

    // Should have at least one line
    assert!(!shaped_text.lines.is_empty());

    // The line should have words
    assert!(!shaped_text.lines[0].words.is_empty());
}

#[test]
fn test_text_with_hole() {
    let font_bytes = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut Vec::new()).unwrap();

    let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nullam auctor, nisl \
                     eget ultricies tincidunt, nisl nisl aliquam nisl, eget aliquam nisl nisl \
                     eget nisl. Nullam auctor, nisl eget ultricies tincidunt, nisl nisl aliquam \
                     nisl, eget aliquam nisl nisl eget nisl.";

    let hole = TextHole {
        rect: Rect {
            x: Pt(10.0),
            y: Pt(10.0),
            width: Pt(20.0),
            height: Pt(20.0),
        },
    };

    let options = TextShapingOptions {
        font_size: Pt(12.0),
        max_width: Some(Pt(200.0)),
        holes: vec![hole],
        ..Default::default()
    };

    let shaped_text = shape_text(long_text, &font, &options);

    // Text should have multiple lines
    assert!(shaped_text.lines.len() > 1);
}
