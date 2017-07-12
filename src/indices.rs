//! These indices are for library internal use only. The trick is to publicly export 
//! the types, but put the indices inside a private module. This way you can't 
//! construct these types outside of the library. Use the `add_*` functions to get an index instead.

use *;

/// Index of the page (0-based)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfPageIndex(pub usize);
/// Index of the layer on the nth page
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfLayerIndex(pub usize);

impl PdfLayerIndex {

}

/// Index of the arbitrary content data
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PdfContentIndex(pub usize);

/// Index of a font
#[derive(Clone, Debug, Eq, PartialEq)]
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

/// Index to an Icc Profile, so that we can copy the reference to an
/// ICC profile around, without worrying about lifetimes.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct IccProfileIndex {
    /// The reference to the ICC profile in the documents content list
    icc_profile: PdfContentIndex,
}