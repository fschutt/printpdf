//! PDF page management

use *;
use types::indices::*;
use std::sync::{Arc, Mutex, Weak};

/// PDF page
#[derive(Debug)]
pub struct PdfPage {
    /// page width in point
    pub width_pt: f64,
    /// page height in point
    pub heigth_pt: f64,
    /// Page layers
    layers: Vec<PdfLayer>,
    /// Document this page is contained in
    pub(super) document: Weak<Mutex<PdfDocument>>,
}

impl PdfPage {

    /// Create a new page, notice that width / height are in millimeter.
    /// Page must contain at least one layer
    #[inline]
    pub fn new<S>(document: Weak<Mutex<PdfDocument>>,
                  width_mm: f64, 
                  height_mm: f64, 
                  layer_name: S)
    -> (Self, PdfLayerIndex) where S: Into<String>
    {
        let mut page = Self {
            width_pt: mm_to_pt!(width_mm),
            heigth_pt: mm_to_pt!(height_mm),
            layers: Vec::new(),
            document: document,
        };

        let initial_layer = PdfLayer::new(layer_name, page.document.clone());
        page.layers.push(initial_layer);
        (page, PdfLayerIndex(0))
    }

    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer<S>(&mut self, layer_name: S)
    -> PdfLayerIndex where S: Into<String>
    {
        let layer = PdfLayer::new(layer_name, self.document.clone());
        self.layers.push(layer);
        PdfLayerIndex(self.layers.len() - 1)
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]
    pub fn get_layer(&self, layer: PdfLayerIndex)
    -> &PdfLayer
    {
        self.layers.get(layer.0).unwrap()
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]
    pub fn get_layer_mut(&mut self, layer: PdfLayerIndex)
    -> &mut PdfLayer
    {
        self.layers.get_mut(layer.0).unwrap()
    }
}
