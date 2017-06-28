//! PDF page management

use *;
use indices::*;
use std::sync::{Mutex, Weak};

/// PDF page
#[derive(Debug)]
pub struct PdfPage {
    /// page width in point
    pub width_pt: f64,
    /// page height in point
    pub heigth_pt: f64,
    /// Page layers
    pub layers: Vec<PdfLayer>
}

/// This struct is only a marker struct to indicate the function
/// "Hey, don't use the document directly, but use the page"
/// We can't pass a reference to the page, because doing so would borrow the document
/// and make it non-mutable
pub struct PdfPageReference {
    pub document: Weak<Mutex<PdfDocument>>,
    pub page: PdfPageIndex,
}

impl PdfPage {

    /// Create a new page, notice that width / height are in millimeter.
    /// Page must contain at least one layer
    #[inline]
    pub fn new<S>(width_mm: f64, 
                  height_mm: f64, 
                  layer_name: S)
    -> (Self, PdfLayerIndex) where S: Into<String>
    {
        let mut page = Self {
            width_pt: mm_to_pt!(width_mm),
            heigth_pt: mm_to_pt!(height_mm),
            layers: Vec::new(),
        };

        let initial_layer = PdfLayer::new(layer_name);
        page.layers.push(initial_layer);

        let layer_index = page.layers.len() - 1;

        (page, PdfLayerIndex(layer_index))
    }

    /// Iterates through the layers attached to this page and gathers all resources,
    /// which the layers need. Then returns a dictonary with all the resources 
    /// (fonts, image XObjects, etc.)
    #[inline]
    pub(crate) fn collect_resources(self, contents: &Vec<lopdf::Object>)
    -> (lopdf::Dictionary, Vec<lopdf::Stream>)
    {
        let mut resource_dictionary = lopdf::Dictionary::new();
        let mut layer_streams = Vec::<lopdf::Stream>::new();

        for layer in self.layers {
            // everything returned by layer.collect_resources() is expected to be an entry in the 
            // pages resource dictionary. For example the layer.collect_resources will return ("Font", Stream("MyFont", etc.))
            // If the resources is shared with in the document, it will be ("Font", Reference(4, 0))
            let (resources, layer_stream) = layer.collect_resources_and_streams(contents);
            
            for (resource_dictionary_key, resource_dictionary_entry) in resources {
                resource_dictionary.set(resource_dictionary_key, resource_dictionary_entry);
            }
            layer_streams.push(layer_stream);
        }

        return (resource_dictionary, layer_streams);
    }
}

impl PdfPageReference {

    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer<S>(&self, layer_name: S)
    -> PdfLayerReference where S: Into<String>
    {
        let layer = PdfLayer::new(layer_name);

        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        doc.pages.get_mut(self.page.0).unwrap().layers.push(layer);
        let index = PdfLayerIndex(doc.pages.get(self.page.0).unwrap().layers.len() - 1);

        PdfLayerReference {
            document: self.document.clone(),
            page: self.page.clone(),
            layer: index,
        }
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]
    pub fn get_layer(&self, layer: PdfLayerIndex)
    -> PdfLayerReference
    {
        let doc = self.document.upgrade().unwrap();
        let doc = doc.lock().unwrap();

        doc.pages.get(self.page.0).unwrap().layers.get(layer.0).unwrap();

        PdfLayerReference {
            document: self.document.clone(),
            page: self.page.clone(),
            layer: layer,
        }
    }
}