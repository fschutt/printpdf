extern crate printpdf;
use printpdf::*;
use std::fs::File;

fn main() {
    let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 210.0, 297.0, "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let text = "Lorem ipsum";
    let text2 = "dolor sit amet";

    let font = doc.add_font(File::open("../assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
    let font2 = doc.add_font(File::open("../assets/fonts/leaguespartan-bold.ttf").unwrap()).unwrap();
    
    // `use_text` is a wrapper around making a simple string
    current_layer.use_text(text.clone(), 48, 0.0, 200.0, &font);
    
    // For more complex layout of text, you can use functions 
    // defined on the PdfLayerReference
    // Make sure to wrap your commands 
    // in a `begin_text_section()` and `end_text_section()` wrapper
    current_layer.begin_text_section();

        // setup the general fonts. 
        // see the docs for these functions for details
        current_layer.set_font(&font2, 33);
        current_layer.set_text_cursor(10.0, 10.0);
        current_layer.set_line_height(33);
        current_layer.set_word_spacing(3000);
        current_layer.set_character_spacing(10);
        current_layer.set_text_rendering_mode(TextRenderingMode::Stroke);

        // write two lines (one line break)
        current_layer.write_text(text.clone(), &font2);
        current_layer.add_line_break();
        current_layer.write_text(text2.clone(), &font2);
        current_layer.add_line_break();

        // write one line, but write text2 in superscript
        current_layer.write_text(text.clone(), &font2);
        current_layer.set_line_offset(10);
        current_layer.write_text(text2.clone(), &font2);

    current_layer.end_text_section();

    doc.save(&mut File::create("test_fonts.pdf").unwrap()).unwrap();
}