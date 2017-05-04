//! # printpdf
//! 
//! printpdf is a library designed for creating printable (PDF-X/1-A conform) 
//! PDF documents.
//! 
//! # Getting started
//!
//! ## Writing PDF
//! 
//! There are three types of functions: `add_*`, `get_*` and `set_*`.
//! To use anything in your PDF document, you have to first `add_` it and then `set_` it.
//! `get_*` functions are for getting PDF structure without modifying the document.
//! PDF is a state-based file-format, once you set something (for example a color), 
//! it will be used for all following elements, until you set it to something else.
//! 
//! ```rust
//! #[macro_use]
//! extern crate printpdf;
//! 
//! use printpdf::*;
//! 
//! // To prevent empty documents, you must specify at least one page
//! // You can later on add more pages with the add_page() function
//! // You also have to specify the title of the PDF and the document creator
//! let mut doc = PDFDocument::new(
//!                   vec![Page::new(247, 210)], 
//!                  "Hello World PDF!",
//!                  "superprogram_v1.1");
//! 
//! let text = "Hello World! Unicode test: стуфхfцчшщъыьэюя";
//! 
//! // printpdf is made for PDF-X/1A conform documents. 
//! // As such, using the default fonts is not permitted. You have to use your own fonts here
//! let roboto_font_path = "assets/fonts/Roboto.ttf";
//! let roboto_font = doc.add_ttf(roboto_font_path).unwrap();
//! 
//! // It isn't allowed to create anything without specifying the layer and page
//! // that the content should be on. If the page isn't valid, an error will be returned
//! let layer1 = doc.add_layer(doc.get_page_by_num(0).unwrap());
//! 
//! // Set the horizonal + vertical offset from the top left corner in pt
//! // get_* functions do not change the state of the document
//! let marker = doc.get_marker(100.0, 100.0, &layer1);
//! 
//! // Write the text with font + font size
//! doc.add_text(text, roboto_font, 48, &marker);
//! 
//! // printpdf support 2d graphics only - Lines, Points, Polygons and SVG Symbols
//! doc.add_page(Page::new(250, 250));
//! let layer2 = doc.add_layer(doc.get_page_by_num(1).unwrap());
//! 
//! let marker2 = doc.get_marker(200.0, 200.0, &layer2);
//! let marker3 = doc.get_marker(300.0, 300.0, &layer2);
//! let marker4 = doc.get_marker(400.0, 500.0, &layer2);
//! let marker5 = doc.get_marker(700.0, 700.0, &layer2);
//! 
//! // mismatching layers or pages will be an error, first reset the marker to position 2
//! doc.set_marker(400.0, 400.0, &layer2);
//! doc.add_line_to(&marker2).unwrap();                           /* simple line */
//! doc.add_bezier4_to(&marker3, &marker4, &marker5).unwrap();    /* quadratic bezier */
//! doc.add_bezier4_to(&marker3, &marker3, &marker2).unwrap();    /* points that are duplicated i*/
//! 
//! let outline = Outline::new(Cmyk::new(1.0, 0.75, 0.0, 0.0), 5.0);
//! doc.add_stroke(&outline);
//! 
//! // A special thing is transcoding SVG files directly into PDF (for mapping symbols)
//! let svg_sample = Svg::parse(File::open("assets/svg/sample.svg").unwrap());
//! // Specify the lower left corner of the SVG
//! let marker6 = doc.get_marker(700.0, 700.0, &layer2);
//! doc.add_svg_at(&svg_sample, &marker6, svg_sample.width, svg_sample.height);
//! 
//! // There is no support for comments, images, annotations, 3D objects, signatures, gradients, etc. yet.
//! 
//! // Save the PDF file
//! doc.save("output.pdf");
//! 
//! ```
//! 
//! ## Reading PDF 
//! 
//! TODO
//! 

#![allow(dead_code)]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;

             extern crate lopdf;
             extern crate freetype;

#[macro_use] pub mod glob_macros;
             pub mod api;
             pub mod errors;
             mod glob_defines;
             
pub use api::*;
pub use errors::*;
pub(crate) use glob_defines::*;