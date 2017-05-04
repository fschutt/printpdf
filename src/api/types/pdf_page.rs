//! Module for operation 
use super::*;
use errors::*;

/// PDF page
#[derive(Debug, Clone)]
pub struct PdfPage {
    /// page width in point
    pub width_pt: f64,
    /// page height in point
    pub heigth_pt: f64,
    /// Page layers
    layers: Vec<PdfLayer>
}

impl PdfPage {

    /// Create a new page, notice that width / height are in millimeter
    /// Page must contain at least one layer
    #[inline]
    pub fn new(width_mm: f64, height_mm: f64)
    -> Self
    {
        Self {
            width_pt: mm_to_pt!(width_mm),
            heigth_pt: mm_to_pt!(height_mm),
            layers: Vec::new(),
        }
    }

    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer<S: Into<String>>(&mut self, name: S)
    -> usize
    {
        self.layers.push(PdfLayer::new(name));
        self.layers.len() - 1
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]
    pub fn get_layer(&self, layer: &usize)
    -> ::std::result::Result<&PdfLayer, Error>
    {
        use errors::index_error::ErrorKind::*;
        self.layers.get(*layer)
                  .ok_or(Error::from_kind(IndexError(PdfLayerIndexError)))
    }


    /// Validates that a layer is present and returns a mutable reference to it
    #[inline]
    pub fn get_mut_layer(&mut self, layer: &usize)
    -> ::std::result::Result<&mut PdfLayer, Error>
    {
        use errors::index_error::ErrorKind::*;
        self.layers.get_mut(*layer)
                   .ok_or(Error::from_kind(IndexError(PdfLayerIndexError)))
    }
}