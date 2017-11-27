extern crate printpdf;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 500.0, 300.0, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let text = "Lorem ipsum";
    let text2 = "dolor sit amet";

    let mut font_reader = std::io::Cursor::new(include_bytes!("../assets/fonts/RobotoMedium.ttf").as_ref());
    let mut font_reader2 = std::io::Cursor::new(include_bytes!("../assets/fonts/RobotoMedium.ttf").as_ref());

    let font = doc.add_external_font(&mut font_reader).unwrap();
    let font2 = doc.add_external_font(&mut font_reader2).unwrap();

    // `use_text` is a wrapper around making a simple string
    current_layer.use_text(text, 48, 10.0, 200.0, &font);

    // text fill color = blue
    let blue = Rgb::new(13.0 / 256.0, 71.0 / 256.0, 161.0 / 256.0, None);
    let orange = Rgb::new(244.0 / 256.0, 67.0 / 256.0, 54.0 / 256.0, None);
    current_layer.set_fill_color(Color::Rgb(blue));
    current_layer.set_outline_color(Color::Rgb(orange));

    // For more complex layout of text, you can use functions
    // defined on the PdfLayerReference
    // Make sure to wrap your commands
    // in a `begin_text_section()` and `end_text_section()` wrapper
    current_layer.begin_text_section();

        // setup the general fonts.
        // see the docs for these functions for details
        current_layer.set_font(&font2, 33);
        current_layer.set_text_cursor(10.0, 100.0);
        current_layer.set_line_height(33);
        current_layer.set_word_spacing(3000);
        current_layer.set_character_spacing(10);

        // write two lines (one line break)
        current_layer.write_text(text, &font2);
        current_layer.add_line_break();
        current_layer.write_text(text2, &font2);
        current_layer.add_line_break();

        current_layer.set_text_rendering_mode(TextRenderingMode::FillStroke);
        current_layer.set_character_spacing(0);
        current_layer.set_text_matrix(TextMatrix::Rotate(10.0));

        // write one line, but write text2 in superscript
        current_layer.write_text(text, &font2);
        current_layer.set_line_offset(10);
        current_layer.set_text_rendering_mode(TextRenderingMode::Stroke);
        current_layer.set_font(&font2, 18);
        current_layer.write_text(text2, &font2);

    current_layer.end_text_section();

    doc.save(&mut BufWriter::new(File::create("test_fonts.pdf").unwrap())).unwrap();
}
