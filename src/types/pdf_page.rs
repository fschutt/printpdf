//! PDF page management

use *;
use indices::*;
use std::sync::{Mutex, Weak};

/// PDF page
#[derive(Debug)]
pub struct PdfPage {
    /// The index of the page in the document
    index: usize,
    /// page width in point
    pub width_pt: f64,
    /// page height in point
    pub heigth_pt: f64,
    /// Page layers
    pub layers: Vec<PdfLayer>,
    /// Resources used in this page
    pub(crate) resources: PdfResources,
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
                  layer_name: S,
                  page_index: usize)
    -> (Self, PdfLayerIndex) where S: Into<String>
    {
        let mut page = Self {
            index: page_index,
            width_pt: mm_to_pt!(width_mm),
            heigth_pt: mm_to_pt!(height_mm),
            layers: Vec::new(),
            resources: PdfResources::new(),
        };

        let initial_layer = PdfLayer::new(layer_name);
        page.layers.push(initial_layer);

        let layer_index = page.layers.len() - 1;

        (page, PdfLayerIndex(layer_index))
    }

    /// Iterates through the layers attached to this page and gathers all resources,
    /// which the layers need. Then returns a dictonary with all the resources 
    /// (fonts, image XObjects, etc.)
    ///
    /// While originally I had planned to build a system where you can reference contents
    /// from all over the document, this turned out to be a problem, because each type had
    /// to be handled differently (PDF weirdness)
    #[inline]
    pub(crate) fn collect_resources_and_streams(self, doc: &mut lopdf::Document /* contents: &Vec<lopdf::Object> */)
    -> (lopdf::Dictionary, Vec<lopdf::Stream>)
    {
        let resource_dictionary: lopdf::Dictionary = self.resources.into_with_document(doc);

        // set contents
        let mut layer_streams = Vec::<lopdf::Stream>::new();
        for layer in self.layers {
            // everything returned by layer.collect_resources() is expected to be an entry in the 
            // pages resource dictionary. For example the layer.collect_resources will return ("Font", Stream("MyFont", etc.))
            // If the resources is shared with in the document, it will be ("Font", Reference(4, 0))
            let layer_stream = layer.into_obj();
            layer_streams.push(layer_stream);
        }

        return (resource_dictionary, layer_streams);
    }

    /// Change the graphics state. Before this operation is done, you should save 
    /// the graphics state using the `save_graphics_state()` function. This will change the 
    /// current graphics state until the end of the page or until the page is reset to the 
    /// previous state.
    /// Returns the old graphics state, in case it was overwritten, as well as a reference 
    /// to the currently active graphics state
    #[inline]
    pub fn add_graphics_state(&mut self, added_state: ExtendedGraphicsState)
    -> ExtendedGraphicsStateRef
    {
        self.resources.add_graphics_state(added_state)
    }

    #[inline]
    pub fn add_pattern(&mut self, pattern: Pattern)
    -> PatternRef
    {
        self.resources.add_pattern(pattern)
    }

    #[inline]
    pub fn add_xobject(&mut self, xobj: XObject)
    -> XObjectRef
    {
        self.resources.add_xobject(xobj)
    }

    #[inline]
    pub fn add_font(&mut self, font: Font)
    -> FontRef
    {
        self.resources.add_font(font)
    }

}

impl PdfPageReference {

    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer<S>(&self, layer_name: S)
    -> PdfLayerReference where S: Into<String>
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();
        let mut page = doc.pages.get_mut(self.page.0).unwrap();

        let current_page_index = page.layers.len(); /* order is important */
        let layer = PdfLayer::new(layer_name);
        page.layers.push(layer);
        let index = PdfLayerIndex(current_page_index);

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