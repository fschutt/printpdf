//! Module for operation 
use super::*;
use super::indices::*;
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
    pub fn new(width_mm: f64, height_mm: f64, initial_layer: PdfLayer)
    -> Self
    {
        Self {
            width_pt: mm_to_pt!(width_mm),
            heigth_pt: mm_to_pt!(height_mm),
            layers: vec![initial_layer],
        }
    }

    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer(&mut self, layer: PdfLayer)
    -> PdfLayerIndex
    {
        self.layers.push(layer);
        PdfLayerIndex(self.layers.len() - 1)
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]
    pub fn get_layer(&self, layer: usize)
    -> ::std::result::Result<PdfLayerIndex, Error>
    {
        use errors::index_error::ErrorKind::*;
        let index = self.layers.get(layer)
                               .ok_or(Error::from_kind(IndexError(PdfLayerIndexError)));
        Ok(PdfLayerIndex(layer))
    }
}