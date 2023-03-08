//! These indices are for library internal use only.
//! Use the `add_*` functions to get an index instead.

/// Index of the page (0-based)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfPageIndex(pub(crate) usize);
/// Index of the layer on the nth page
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfLayerIndex(pub(crate) usize);

/// Index of the arbitrary content data
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PdfContentIndex(pub(crate) usize);

/// Index of a font
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FontIndex(pub(crate) PdfContentIndex);

impl From<FontIndex> for PdfContentIndex {
    fn from(val: FontIndex) -> Self {
        val.0
    }
}

/// Index of a svg file
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SvgIndex(pub(crate) PdfContentIndex);

impl From<SvgIndex> for PdfContentIndex {
    fn from(val: SvgIndex) -> Self {
        val.0
    }
}
