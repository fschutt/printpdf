//! `printpdf` is a library designed for creating printable PDF documents.
//!
//! [Crates.io](https://crates.io/crates/printpdf) | [Documentation](https://docs.rs/printpdf)
//!
//! ```ignore
//! [dependencies]
//! printpdf = "0.1.2"
//! ```
//!
//! # Features
//!
//! Currently, printpdf can only write documents, not read them.
//!
//! - Page generation
//! - Layers (Illustrator like layers)
//! - Graphics (lines, shapes, bezier curves)
//! - Images (currently BMP only or generate your own images)
//! - Embedded fonts (TTF and OTF) with Unicode support
//! - Advanced graphics - overprint control, blending modes, etc.
//! - Advanced typography - character scaling, character spacing, superscript, subscript, outlining, etc.
//! - PDF layers (you should be able to open the PDF in Illustrator and have the layers appear)
//!
//! # Getting started
//!
//! ## Writing PDF
//!
//! There are two types of functions: `add_*` and `use_*`. `add_*`-functions operate on the
//! document and return a reference to the content that has been added. This is used for
//! instantiating objects via references in the document (for example, for reusing a block of
//! data - like a font) without copying it (and bloating the file size).
//!
//! Instancing happens via the `use_*`-functions, which operate on the layer. Meaning, you can only
//! instantiate blobs / content when you have a reference to the layer. Here are some examples:
//!
//! ### Simple page
//!
//! ```rust
//! use printpdf::*;
//! use std::fs::File;
//!
//! let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
//! let (page2, layer1) = doc.add_page(10.0, 250.0,"Page 2, Layer 1");
//!
//! doc.save(&mut File::create("test_working.pdf").unwrap()).unwrap();
//! ```
//!
//! ### Adding graphical shapes
//!
//! ```
//! use printpdf::*;
//! use std::fs::File;
//!
//! let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
//!
//! let mut current_layer = doc.get_page(page1).get_layer(layer1);
//!
//! // Quadratic shape. The "false" determines if the next (following)
//! // point is a bezier handle (for curves)
//! // If you want holes, simply reorder the winding of the points to be
//! // counterclockwise instead of clockwise.
//! let points1 = vec![(Point::new(100.0, 100.0), false),
//!                    (Point::new(100.0, 200.0), false),
//!                    (Point::new(300.0, 200.0), false),
//!                    (Point::new(300.0, 100.0), false)];
//!
//! // Is the shape stroked? Is the shape closed? Is the shape filled?
//! let line1 = Line::new(points1, true, true, true);
//!
//! // Triangle shape
//! let points2 = vec![(Point::new(150.0, 150.0), false),
//!                    (Point::new(150.0, 250.0), false),
//!                    (Point::new(350.0, 250.0), false)];
//!
//! let line2 = Line::new(points2, true, false, false);
//!
//! let fill_color = Color::Cmyk(Cmyk::new(0.0, 0.23, 0.0, 0.0, None));
//! let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
//! let mut dash_pattern = LineDashPattern::default();
//! dash_pattern.dash_1 = Some(20);
//!
//! current_layer.set_fill_color(fill_color);
//! current_layer.set_outline_color(outline_color);
//! current_layer.set_outline_thickness(10);
//!
//! // Draw first line
//! current_layer.add_shape(line1);
//!
//! let fill_color_2 = Color::Cmyk(Cmyk::new(0.0, 0.0, 0.0, 0.0, None));
//! let outline_color_2 = Color::Greyscale(Greyscale::new(0.45, None));
//!
//! // More advanced graphical options
//! current_layer.set_overprint_stroke(true);
//! current_layer.set_blend_mode(BlendMode::Seperable(SeperableBlendMode::Multiply));
//! current_layer.set_line_dash_pattern(dash_pattern);
//! current_layer.set_line_cap_style(LineCapStyle::Round);
//!
//! current_layer.set_fill_color(fill_color_2);
//! current_layer.set_outline_color(outline_color_2);
//! current_layer.set_outline_thickness(15);
//!
//! // draw second line
//! current_layer.add_shape(line2);
//! ```
//!
//! ### Adding images
//!
//! Note: Images only get compressed in release mode. You might get huge PDFs (6 or more MB) in
//! debug mode. In release mode, the compression makes these files much smaller (~ 100 - 200 KB).
//!
//! To make this process faster, use `BufReader` instead of directly reading from the file.
//! Images are currently not a top priority.
//!
//! Scaling of images is implicitly done to fit one pixel = one dot at 300 dpi.
//!
//! ```
//! #![feature(try_from)]
//! extern crate printpdf;
//! extern crate image; /* currently: version 0.14.0 */
//!
//! use printpdf::*;
//! use std::fs::File;
//! use std::convert::TryFrom;
//! use std::convert::From;
//!
//! fn main() {
//!     let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
//!     let current_layer = doc.get_page(page1).get_layer(layer1);
//!
//!     // currently, the only reliable file format is bmp (jpeg works, but not in release mode)
//!     // this is an issue of the image library, not a fault of printpdf
//!     let mut image_file = File::open("assets/img/BMP_test.bmp").unwrap();
//!     let image = Image::try_from(image::bmp::BMPDecoder::new(&mut image_file)).unwrap();
//!
//!     // translate x, translate y, rotate, scale x, scale y
//!     // by default, an image is optimized to 300 DPI (if scale is None)
//!     // rotations and translations are always in relation to the lower left corner
//!     image.add_to_layer(current_layer.clone(), None, None, None, None, None, None);
//!
//!     // you can also construct images manually from your data:
//!     let mut image_file_2 = ImageXObject {
//!         width: 200,
//!         height: 200,
//!         color_space: ColorSpace::Greyscale,
//!         bits_per_component: ColorBits::Bit8,
//!         interpolate: true,
//!         /* put your bytes here. Make sure the total number of bytes =
//!            width * height * (bytes per component * number of components)
//!            (e.g. 2 (bytes) x 3 (colors) for RGB 16bit) */
//!         image_data: Vec::new(),
//!         image_filter: None, /* does not work yet */
//!         clipping_bbox: None, /* doesn't work either, untested */
//!     };
//!
//!     let image2 = Image::from(image_file_2);
//! }
//! ```
//!
//! ### Adding fonts
//!
//! Note: Fonts are shared between pages. This means that they are added to the document first
//! and then a reference to this one object can be passed to multiple pages. This is different to
//! images, for example, which can only be used once on the page they are created on (since that's
//! the most common use-case).
//!
//! ```rust
//! use printpdf::*;
//! use std::fs::File;
//!
//! let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", 247.0, 210.0, "Layer 1");
//! let current_layer = doc.get_page(page1).get_layer(layer1);
//!
//! let text = "Lorem ipsum";
//! let text2 = "unicode: стуфхfцчшщъыьэюя";
//!
//! let font = doc.add_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
//! let font2 = doc.add_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
//!
//! // text, font size, x from left edge, y from top edge, font
//! current_layer.use_text(text, 48, 200.0, 200.0, &font);
//!
//! // For more complex layout of text, you can use functions
//! // defined on the PdfLayerReference
//! // Make sure to wrap your commands
//! // in a `begin_text_section()` and `end_text_section()` wrapper
//! current_layer.begin_text_section();
//!
//!     // setup the general fonts.
//!     // see the docs for these functions for details
//!     current_layer.set_font(&font2, 33);
//!     current_layer.set_text_cursor(10.0, 10.0);
//!     current_layer.set_line_height(33);
//!     current_layer.set_word_spacing(3000);
//!     current_layer.set_character_spacing(10);
//!     current_layer.set_text_rendering_mode(TextRenderingMode::Stroke);
//!
//!     // write two lines (one line break)
//!     current_layer.write_text(text.clone(), &font2);
//!     current_layer.add_line_break();
//!     current_layer.write_text(text2.clone(), &font2);
//!     current_layer.add_line_break();
//!
//!     // write one line, but write text2 in superscript
//!     current_layer.write_text(text.clone(), &font2);
//!     current_layer.set_line_offset(10);
//!     current_layer.write_text(text2.clone(), &font2);
//!
//! current_layer.end_text_section();
//! ```
//!
//! # Further reading
//!
//! The `PDFDocument` is hidden behind a `PDFDocumentReference`, which locks the things you can
//! do behind a facade. Pretty much all functions operate on a `PDFLayerReference`, so that would
//! be where to look for existing functions or where to implement new functions. The `PDFDocumentReference`
//! is a reference-counted document. It uses the pages and layers for inner mutablility, because
//! I ran into borrowing issues with the document. __IMPORTANT:__ All functions that mutate the state
//! of the document, "borrow" the document mutably for the duration of the function. It is important
//! that you don't borrow the document twice (your program will crash if you do so). I have prevented
//! this wherever possible, by making the document only public to the crate so you cannot lock it from
//! outside of this library.
//!
//! Images have to be added to the pages resources before using them. Meaning, you can only use an image
//! on the page that you added it to. Otherwise, you may end up with a corrupt PDF.
//!
//! Fonts are embedded using `rusttype`. In the future, there should be an option to use `freetype`,
//! because `freetype` can use OpenType fonts. Please report issues if you have any, especially if you
//! see `BorrowMut` errors (they should not happen). Kerning is currently not done, should be added later.
//! However, "correct" kerning / placement requires a full font shaping engine, etc. This would be a completely
//! different project.
//!
//! For learning how a PDF is actually made, please read the [wiki](https://github.com/sharazam/printpdf/wiki).
//! When I began making this library, these resources were not available anywhere, so I hope to help other people
//! with these topics. Reading the wiki is essential if you want to contribute to this library.
//!
//! # Goals and Roadmap
//!
//! The goal of printpdf is to be a general-use PDF library, such as libharu or similar.
//! PDFs generated by printpdf must always adhere to a PDF standard. However, not all standards
//! are supported. See this list:
//!
//! - [ ] PDF/A-1b:2005
//! - [ ] PDF/A-1a:2005
//! - [ ] PDF/A-2:2011
//! - [ ] PDF/A-2a:2011
//! - [ ] PDF/A-2b:2011
//! - [ ] PDF/A-2u:2011
//! - [ ] PDF/A-3:2012
//! - [ ] PDF/UA-1
//! - [ ] PDF/X-1a:2001
//! - [x] PDF/X-3:2002
//! - [ ] PDF/X-1a:2003
//! - [ ] PDF/X-3:2003
//! - [ ] PDF/X-4:2010
//! - [ ] PDF/X-4P:2010
//! - [ ] PDF/X-5G:2010
//! - [ ] PDF/X-5PG:2010
//! - [ ] PDF/X-5N:2010
//! - [ ] PDF/E-1
//! - [ ] PDF/VT:2010
//!
//! Over time, there will be more standards supported. Checking a PDF for errors is currently only a stub.
//!
//! ## Planned features
//!
//! - Clipping
//! - Aligning / layouting text
//! - Open Prepress Interface
//! - Halftoning images, Gradients, Patterns
//! - SVG / instantiated content
//! - More font support
//! - Forms, annotations
//! - Bookmarks / Table of contents
//! - Conformance / error checking for various PDF standards
//! - Embedded Javascript
//! - Reading PDF
//! - Completion of printpdf wiki
//!
//! # Contributing
//!
//! [READ THE WIKI FIRST !!!](https://github.com/sharazam/printpdf/wiki)
//!
//! - Fork the project, make you own branch
//! - If you want to add support for some data type, let's say images or embedded video, create your type
//! in `/src/types/plugins/[family of your type]/[type].rs`
//! - The type should implement `IntoPdfObject`, so that it can be added to the document
//! - Change the `page` and `layer content types to have a convenience function for adding your type
//! - Document your changes. Add a doc test (how you expect the type to be used) and a unit test
//! (if the type is conform to the expected PDF type)
//! - If you want to change this README, change the lib.rs instead and run `cargo readme > README.md`.
//! - Create pull request
//!
//! # Testing
//!
//! Currently the testing is pretty much non-existent, because PDF is very hard to test. This should change
//! over time: Testing should be done in two stages. First, test the individual PDF objects, if the conversion
//! into a PDF object is done correctly. The second stage is manual inspection of PDF objects via Adobe Preflight.
//!
//! Put the tests of the first stage in /tests/mod.rs. The second stage tests are better to be handled
//! inside the plugins' mod.rs file. `printpdf` depends highly on [lopdf](https://github.com/J-F-Liu/lopdf),
//! so you can either construct your test object against a real type or a debug string of your serialized
//! type. Either way is fine - you just have to check that the test object is conform to what PDF expects.
//!
//! # Useful links
//!
//! Here are some resources I found while working on this library
//!
//! [PDFXPlorer, shows the DOM tree of a PDF, needs .NET 2.0](http://www.o2sol.com/pdfxplorer/download.htm)
//!
//! [Official PDF 1.7 reference](http://www.adobe.com/content/dam/Adobe/en/devnet/acrobat/pdfs/pdf_reference_1-7.pdf)
//!
//! [[GERMAN] How to embed unicode fonts in PDF](http://www.p2501.ch/pdf-howto/typographie/vollzugriff/direkt)
//!
//! [PDF X/1-a Validator](https://www.pdf-online.com/osa/validate.aspx)
//!
//! [PDF X/3 technical notes](http://www.pdfxreport.com/lib/exe/fetch.php?media=en:technote_pdfx_checks.pdf)
//!

#![feature(try_from)]
#![feature(collection_placement)]
#![feature(placement_in_syntax)]

#![allow(unused_doc_comment)]
#![allow(unused_variables)]
#![allow(dead_code)]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;
#[macro_use] pub mod glob_macros;

extern crate lopdf;
extern crate freetype;
extern crate chrono;
extern crate rand;
extern crate svg;
extern crate image;

pub mod traits;
pub mod types;
pub mod errors;
mod glob_defines;
mod indices;
#[cfg(test)] mod tests;

pub use self::traits::*;
pub use self::types::*;
pub use self::errors::*;
use glob_defines::*;
