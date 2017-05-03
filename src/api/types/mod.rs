//! Shared types regarding the structure of a PDF.

pub mod pdf_document;
pub mod pdf_layer;
pub mod pdf_marker;
pub mod pdf_page;

pub use self::pdf_document::PdfDocument;
pub use self::pdf_page::PdfPage;
pub use self::pdf_marker::PdfMarker;
pub use self::pdf_layer::PdfLayer;

/// Index of the page (0-based)
pub type PdfPageIndex = usize;
/// Index of the layer on the nth page
pub type PdfLayerIndex = (PdfPageIndex, usize);
/// Index of the marker on the nth-layer on the mth-page
pub type PdfMarkerIndex = (PdfPageIndex, usize, usize);
/// Indes of the arbitrary content data
pub type PdfContentIndex = usize;

/// ### Strongly typed data structures
pub struct FontIndex(PdfContentIndex);
pub struct SvgIndex(PdfContentIndex);

/// Expandable plugins that implement PdfContent
pub mod plugins;

pub use self::plugins::*;