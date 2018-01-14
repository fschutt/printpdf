//! `printpdf` is a library designed for creating printable PDF documents.
//!
//! [Crates.io](https://crates.io/crates/printpdf) | [Documentation](https://docs.rs/printpdf)
//!
//! ```toml,ignore
//! [dependencies]
//! printpdf = "0.2.2"
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
//! ## Installation requirements
//! 
//! ### Linux
//! 
//! You need to install freetype, if you haven't already installed it:
//! ```
//! sudo apt install libfreetype6-dev
//! ```
//! 
//! ### Windows
//! 
//! #### pc-windows-gnu
//! 
//! In order to easily setup freetype just get MSYS2 and install either the `mingw-w64-x86_64-freetype` or `mingw-w64-i686-freetype` package and then use Rust from within the correct mingw shell of MSYS2.
//! 
//! More information on setting up MSYS2 for Rust can be found in [the Rust readme](https://github.com/rust-lang/rust#building-on-windows)
//! 
//! #### pc-windows-msvc
//! 
//! Prebuilt libraries for freetype are available [here](https://github.com/PistonDevelopers/binaries).
//! 
//! Then in the root of your project, in one of the parent directories, or in your home directory, create a .cargo directory. This directory should contain a `config` file that contains the following snippet:
//! 
//! ```toml
//! [target.i686-pc-windows-msvc.freetype]
//! rustc-link-search = ["C:\\Path\\To\\binaries\\i686"]
//! rustc-link-lib = ["freetype"]
//! 
//! [target.x86_64-pc-windows-msvc.freetype]
//! rustc-link-search = ["C:\\Path\\To\\binaries\\x86_64"]
//! rustc-link-lib = ["freetype"]
//! ```
//! 
//! For more informations, check [the official Cargo documentation](http://doc.crates.io/build-script.html#overriding-build-scripts).
//! 
//! (taken from [this README](https://github.com/PistonDevelopers/freetype-sys/blob/master/README.md))
//! 
//! ## Writing PDF
//! 
//! ### Simple page
//!
//! ```rust
//! use printpdf::*;
//! use std::fs::File;
//! use std::io::BufWriter;
//!
//! let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
//! let (page2, layer1) = doc.add_page(Mm(10.0), Mm(250.0),"Page 2, Layer 1");
//!
//! doc.save(&mut BufWriter::new(File::create("test_working.pdf").unwrap())).unwrap();
//! ```
//!
//! ### Adding graphical shapes
//!
//! ```rust
//! use printpdf::*;
//! use std::fs::File;
//! use std::io::BufWriter;
//! use std::iter::FromIterator;
//! 
//! let (doc, page1, layer1) = PdfDocument::new("printpdf graphics test", Mm(297.0), Mm(210.0), "Layer 1");
//! let current_layer = doc.get_page(page1).get_layer(layer1);
//! 
//! // Quadratic shape. The "false" determines if the next (following)
//! // point is a bezier handle (for curves)
//! // If you want holes, simply reorder the winding of the points to be
//! // counterclockwise instead of clockwise.
//! let points1 = vec![(Point::new(Mm(100.0), Mm(100.0)), false),
//!                    (Point::new(Mm(100.0), Mm(200.0)), false),
//!                    (Point::new(Mm(300.0), Mm(200.0)), false),
//!                    (Point::new(Mm(300.0), Mm(100.0)), false)];
//! 
//! // Is the shape stroked? Is the shape closed? Is the shape filled?
//! let line1 = Line { 
//!     points: points1, 
//!     is_closed: true, 
//!     has_fill: true,
//!     has_stroke: true,
//!     is_clipping_path: false,
//! };
//! 
//! // Triangle shape
//! // Note: Line is invisible by default, the previous method of 
//! // constructing a line is recommended!
//! let mut line2 = Line::from_iter(vec![
//!     (Point::new(Mm(150.0), Mm(150.0)), false),
//!     (Point::new(Mm(150.0), Mm(250.0)), false),
//!     (Point::new(Mm(350.0), Mm(250.0)), false)]);
//! 
//! line2.set_stroke(true);
//! line2.set_closed(false);
//! line2.set_fill(false);
//! line2.set_as_clipping_path(false);
//!
//! let fill_color = Color::Cmyk(Cmyk::new(0.0, 0.23, 0.0, 0.0, None));
//! let outline_color = Color::Rgb(Rgb::new(0.75, 1.0, 0.64, None));
//! let mut dash_pattern = LineDashPattern::default();
//! dash_pattern.dash_1 = Some(20);
//!
//! current_layer.set_fill_color(fill_color);
//! current_layer.set_outline_color(outline_color);
//! current_layer.set_outline_thickness(10.0);
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
//! current_layer.set_outline_thickness(15.0);
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
//! ```rust
//! extern crate printpdf;
//! 
//! // imports the `image` library with the exact version that we are using
//! use printpdf::*;
//! 
//! use std::convert::From;
//! use std::fs::File;
//!
//! fn main() {
//!     let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
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
//!         width: Px(200),
//!         height: Px(200),
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
//! let (doc, page1, layer1) = PdfDocument::new("PDF_Document_title", Mm(247.0), Mm(210.0), "Layer 1");
//! let current_layer = doc.get_page(page1).get_layer(layer1);
//!
//! let text = "Lorem ipsum";
//! let text2 = "unicode: стуфхfцчшщъыьэюя";
//!
//! let font = doc.add_external_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
//! let font2 = doc.add_external_font(File::open("assets/fonts/RobotoMedium.ttf").unwrap()).unwrap();
//!
//! // text, font size, x from left edge, y from top edge, font
//! current_layer.use_text(text, 48, Mm(200.0), Mm(200.0), &font);
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
//!     current_layer.set_text_cursor(Mm(10.0), Mm(10.0));
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
//! ## Changelog
//! 
//! See the CHANGELOG.md file.
//!
//! # Further reading
//!
//! The `PDFDocument` is hidden behind a `PDFDocumentReference`, which locks 
//! the things you can do behind a facade. Pretty much all functions operate 
//! on a `PDFLayerReference`, so that would be where to look for existing 
//! functions or where to implement new functions. The `PDFDocumentReference`
//! is a reference-counted document. It uses the pages and layers for inner 
//! mutablility, because
//! I ran into borrowing issues with the document. __IMPORTANT:__ All functions 
//! that mutate the state of the document, "borrow" the document mutably for 
//! the duration of the function. It is important that you don't borrow the 
//! document twice (your program will crash if you do so). I have prevented
//! this wherever possible, by making the document only public to the crate 
//! so you cannot lock it from outside of this library.
//!
//! Images have to be added to the pages resources before using them. Meaning, 
//! you can only use an image on the page that you added it to. Otherwise, 
//! you may end up with a corrupt PDF.
//!
//! Fonts are embedded using `freetype`. There is a `rusttype` branch in this 
//! repository, but `rusttype` does fails to get the height of an unscaled
//! font correctly, so that's why you currently have to use `freetype`
//! 
//! Please report issues if you have any, especially if you see `BorrowMut` 
//! errors (they should not happen). Kerning is currently not done, because 
//! neither `freetype` nor `rusttype` can reliably read kerning data.
//! However, "correct" kerning / placement requires a full font shaping 
//! engine, etc. This would be a completely different project.
//!
//! For learning how a PDF is actually made, please read the 
//! [wiki](https://github.com/fschutt/printpdf/wiki) (currently not 
//! completely finished). When I began making this library, these resources 
//! were not available anywhere, so I hope to help other people
//! with these topics. Reading the wiki is essential if you want to 
//! contribute to this library.
//!
//! # Goals and Roadmap
//!
//! The goal of printpdf is to be a general-use PDF library, such as 
//! libharu or similar. PDFs generated by printpdf should always adhere 
//! to a PDF standard, except if you turn it off. Currently, only the 
//! standard `PDF/X-3:2002` is covered (i.e. valid PDF according to Adobe 
//! Acrobat). Over time, there will be more standards supported. Checking a 
//! PDF for errors is currently only a stub.
//!
//! ## Planned features / Not done yet
//!
//! The following features aren't implemented yet, most
//! - Clipping
//! - Aligning / layouting text
//! - Open Prepress Interface
//! - Halftoning images, Gradients, Patterns
//! - SVG / instantiated content
//! - Forms, annotations
//! - Bookmarks / Table of contents
//! - Conformance / error checking for various PDF standards
//! - Embedded Javascript
//! - Reading PDF
//! - Completion of printpdf wiki
//!
//! # Testing
//!
//! Currently the testing is pretty much non-existent, because PDF is very hard to test. 
//! This should change over time: Testing should be done in two stages. First, test 
//! the individual PDF objects, if the conversion into a PDF object is done correctly. 
//! The second stage is manual inspection of PDF objects via Adobe Preflight.
//!
//! Put the tests of the first stage in /tests/mod.rs. The second stage tests are 
//! better to be handled inside the plugins' mod.rs file. `printpdf` depends highly 
//! on [lopdf](https://github.com/J-F-Liu/lopdf), so you can either construct your 
//! test object against a real type or a debug string of your serialized type. 
//! Either way is fine - you just have to check that the test object is conform to 
//! what PDF expects.
//!
//! # Useful links
//!
//! Here are some resources I found while working on this library:
//!
//! [`PDFXPlorer`, shows the DOM tree of a PDF, needs .NET 2.0](http://www.o2sol.com/pdfxplorer/download.htm)
//!
//! [Official PDF 1.7 reference](http://www.adobe.com/content/dam/Adobe/en/devnet/acrobat/pdfs/pdf_reference_1-7.pdf)
//!
//! [[GERMAN] How to embed unicode fonts in PDF](http://www.p2501.ch/pdf-howto/typographie/vollzugriff/direkt)
//!
//! [PDF X/1-a Validator](https://www.pdf-online.com/osa/validate.aspx)
//!
//! [PDF X/3 technical notes](http://www.pdfxreport.com/lib/exe/fetch.php?media=en:technote_pdfx_checks.pdf)

#![allow(unused_doc_comment)]
#![allow(unused_variables)]
#![allow(dead_code)]

// Enable clippy if our Cargo.toml file asked us to do so.
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![warn(missing_copy_implementations,
        trivial_numeric_casts,
        trivial_casts,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications)]

#![cfg_attr(feature="clippy", warn(cast_possible_truncation))]
#![cfg_attr(feature="clippy", warn(cast_possible_truncation))]
#![cfg_attr(feature="clippy", warn(cast_precision_loss))]
#![cfg_attr(feature="clippy", warn(cast_sign_loss))]
#![cfg_attr(feature="clippy", warn(missing_docs_in_private_items))]
#![cfg_attr(feature="clippy", warn(mut_mut))]

// Disallow `println!`. Use `debug!` for debug output
// (which is provided by the `log` crate).
#![cfg_attr(feature="clippy", warn(print_stdout))]

// This allows us to use `unwrap` on `Option` values (because doing makes
// working with Regex matches much nicer) and when compiling in test mode
// (because using it in tests is idiomatic).
#![cfg_attr(all(not(test), feature="clippy"), warn(result_unwrap_used))]
#![cfg_attr(feature="clippy", warn(unseparated_literal_suffix))]
#![cfg_attr(feature="clippy", warn(wrong_pub_self_convention))]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;
#[macro_use] pub mod glob_macros;

extern crate lopdf;
extern crate rusttype;
extern crate chrono;
extern crate rand;
pub extern crate image;

pub mod types;
pub mod scale;
pub mod errors;
mod glob_defines;
mod indices;
#[cfg(test)] mod tests;

pub use self::errors::errors::Error as PrintpdfError;
pub use self::errors::errors::ErrorKind as PrintpdfErrorKind;
pub use self::errors::pdf_error::Error as PdfError;
pub use self::errors::pdf_error::ErrorKind as PdfErrorKind;
pub use self::errors::index_error::Error as IndexError;
pub use self::errors::index_error::ErrorKind as IndexErrorKind;

pub use self::scale::{Mm, Pt, Px};
pub use self::types::pdf_conformance::{CustomPdfConformance, PdfConformance};
pub use self::types::pdf_document::{PdfDocumentReference, PdfDocument};
pub use self::types::pdf_metadata::PdfMetadata;
pub use self::types::pdf_page::{PdfPage, PdfPageReference};
pub use self::types::pdf_layer::{PdfLayer, PdfLayerReference};

pub use self::types::plugins::xmp::xmp_metadata::XmpMetadata;
pub use self::types::plugins::misc::document_info::DocumentInfo;

/// Stub module for 3D content in a PDF
pub use self::types::plugins::graphics::three_dimensional;
pub use self::types::plugins::graphics::two_dimensional::font::{
    Font, BuiltinFont, ExternalFont, TextRenderingMode, IndirectFontRef, DirectFontRef, FontList
};
pub use self::types::plugins::graphics::two_dimensional::image::Image;
pub use self::types::plugins::graphics::two_dimensional::line::Line;
pub use self::types::plugins::graphics::two_dimensional::point::Point;

pub use self::types::plugins::graphics::color::{
    Color, Rgb, Cmyk, Greyscale, SpotColor, PdfColor, ColorSpace, ColorBits
};
pub use self::types::plugins::graphics::ctm::{CurTransMat, TextMatrix};
pub use self::types::plugins::graphics::extgstate::{
    ExtendedGraphicsState, ExtendedGraphicsStateList, ExtendedGraphicsStateRef, ExtendedGraphicsStateBuilder,
    OverprintMode, BlackGenerationFunction, BlackGenerationExtraFunction, UnderColorRemovalFunction,
    UnderColorRemovalExtraFunction, TransferFunction, TransferExtraFunction, HalftoneType,
    SpotFunction, BlendMode, SeperableBlendMode, NonSeperableBlendMode, RenderingIntent, SoftMask,
    SoftMaskFunction, LineJoinStyle, LineCapStyle, LineDashPattern, 
};
pub use self::types::plugins::graphics::icc_profile::{
    IccProfileType, IccProfile, IccProfileRef, IccProfileList
};
pub use self::types::plugins::graphics::ocg::{OCGList, OCGRef};
pub use self::types::plugins::graphics::pattern::{Pattern, PatternRef, PatternList};
pub use self::types::plugins::graphics::pdf_resources::PdfResources;
pub use self::types::plugins::graphics::xobject::{
    XObject, XObjectList, XObjectRef, ImageXObject, ImageXObjectRef, 
    ImageFilter, FormXObject, FormXObjectRef, FormType, SMask, GroupXObject, 
    GroupXObjectType, ReferenceXObject, OptionalContentGroup, OCGIntent, PostScriptXObject, 
};

/// Stub module for future audio embedding implementation
pub use self::types::plugins::media::audio;
/// Stub module for future video embedding implementation
pub use self::types::plugins::media::video;
/// Stub module for interactive (JavaScript) content, embedded in PDF files
pub use self::types::plugins::interactive;
/// Stub module for encryption (passwords). Not implemented yet.
pub use self::types::plugins::security;
