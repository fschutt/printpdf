//! Shared types regarding the structure of a PDF.

pub mod pdf_document;
pub mod pdf_layer;
pub mod pdf_page;
pub mod pdf_stream;
pub mod plugins;

pub use self::pdf_document::PdfDocument;
pub use self::pdf_page::PdfPage;
pub use self::pdf_layer::PdfLayer;
pub use self::pdf_stream::PdfStream;
pub use self::plugins::*;

/// These indices are for library internal use only. The trick is to publicly export 
/// the types, but put the indices inside a private module. This way you can't 
/// construct these types outside of the library. Use the `add_*` functions to get an index instead.
mod indices {
    /// Index of the page (0-based)
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct PdfPageIndex(pub usize);
    /// Index of the layer on the nth page
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct PdfLayerIndex(pub usize);
    /// Index of the arbitrary content data
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct PdfContentIndex(pub usize);

    /// Index of a font
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct FontIndex(pub PdfContentIndex);
    /// Index of a svg file
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct SvgIndex(pub PdfContentIndex);
}


