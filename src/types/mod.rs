//! Shared types regarding the structure of a PDF.

pub mod pdf_conformance;
pub mod pdf_document;
pub mod pdf_layer;
pub mod pdf_metadata;
pub mod pdf_page;
pub mod plugins;

pub use self::pdf_document::{PdfDocument, PdfDocumentReference};
pub use self::pdf_layer::{PdfLayer, PdfLayerReference};
pub use self::pdf_page::{PdfPage, PdfPageReference};
pub use self::pdf_conformance::{PdfConformance, CustomPdfConformance};
pub use self::pdf_metadata::PdfMetadata;
pub use self::plugins::*;