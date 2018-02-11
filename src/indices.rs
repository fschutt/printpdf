//! These indices are for library internal use only. The trick is to publicly export
//! the types, but put the indices inside a private module. This way you can't
//! construct these types outside of the library. Use the `add_*` functions to get an index instead.

/// Index of the page (0-based)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfPageIndex(pub usize);
/// Index of the layer on the nth page
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfLayerIndex(pub usize);

/// Index of the arbitrary content data
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PdfContentIndex(pub usize);

/// Index of a font
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FontIndex(pub PdfContentIndex);

impl Into<PdfContentIndex> for FontIndex {
    fn into(self) -> PdfContentIndex
    {
        self.0
    }
}

/// Index of a svg file
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SvgIndex(pub PdfContentIndex);

impl Into<PdfContentIndex> for SvgIndex {
    fn into(self) -> PdfContentIndex
    {
        self.0
    }
}
