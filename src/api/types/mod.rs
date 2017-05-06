//! Shared types regarding the structure of a PDF.

pub mod pdf_conformance;
pub mod pdf_document;
pub mod pdf_layer;
pub mod pdf_page;
pub mod pdf_stream;
pub mod plugins;
pub mod indices;

pub use self::pdf_document::PdfDocument;
pub use self::pdf_page::PdfPage;
pub use self::pdf_layer::PdfLayer;
pub use self::pdf_stream::PdfStream;
pub use self::pdf_conformance::PdfConformance;
pub use self::plugins::*;

use self::indices::*;