//! Shared types regarding the structure of a PDF.

pub mod pdf_document;
pub mod pdf_layer;
pub mod pdf_page;

pub use self::pdf_document::PdfDocument;
pub use self::pdf_page::PdfPage;
pub use self::pdf_layer::PdfLayer;

/// Index of the page (0-based)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfPageIndex(usize);
/// Index of the layer on the nth page
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfLayerIndex(usize);
/// Indes of the arbitrary content data
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfContentIndex(usize);

/// Index of a font
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FontIndex(PdfContentIndex);
/// Index of a svg file
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SvgIndex(PdfContentIndex);

/// Expandable plugins that implement PdfContent
pub mod plugins;

pub use self::plugins::*;