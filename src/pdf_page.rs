//! PDF page management

use lopdf;
use std::cell::RefCell;
use std::rc::Weak;

use crate::indices::{PdfLayerIndex, PdfPageIndex};
use crate::{
    ExtendedGraphicsState, ExtendedGraphicsStateRef, Mm, Pattern, PatternRef, PdfDocument,
    PdfLayer, PdfLayerReference, PdfResources, Pt, XObject, XObjectRef, LinkAnnotation, LinkAnnotationRef,
};

/// PDF page
#[derive(Debug, Clone)]
pub struct PdfPage {
    /// The index of the page in the document
    pub(crate) index: usize,
    /// page width in point
    pub width: Pt,
    /// page height in point
    pub height: Pt,
    /// Page layers
    pub layers: Vec<PdfLayer>,
    /// Resources used in this page
    pub(crate) resources: PdfResources,
    /// Extend the page with custom ad-hoc attributes, as an escape hatch to the low level lopdf library.
    /// Can be used to add annotations to a page.
    /// If your dictionary is wrong it will produce a broken PDF without warning or useful messages.
    pub(crate) extend_with: Option<lopdf::Dictionary>,
}

/// A "reference" to the current page, allows for inner mutability
/// but only inside this library
pub struct PdfPageReference {
    /// A weak reference to the document, for inner mutability
    pub document: Weak<RefCell<PdfDocument>>,
    /// The index of the page this layer is on
    pub page: PdfPageIndex,
}

impl PdfPage {
    /// Create a new page, notice that width / height are in millimeter.
    /// Page must contain at least one layer
    #[inline]
    pub fn new<S>(width: Mm, height: Mm, layer_name: S, page_index: usize) -> (Self, PdfLayerIndex)
    where
        S: Into<String>,
    {
        let mut page = Self {
            index: page_index,
            width: width.into(),
            height: height.into(),
            layers: Vec::new(),
            resources: PdfResources::new(),
            extend_with: None
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
    ///
    /// `layers` should be a Vec with all layers (optional content groups) that were added
    /// to the document on a document level, it should contain the indices of the layers
    /// (they will be ignored, todo) and references to the actual OCG dictionaries
    #[inline]
    pub(crate) fn collect_resources_and_streams(
        self,
        doc: &mut lopdf::Document,
        layers: &[(usize, lopdf::Object)],
    ) -> (lopdf::Dictionary, Vec<lopdf::Stream>) {
        let cur_layers = layers.iter().map(|l| l.1.clone()).collect();
        let (resource_dictionary, ocg_refs) = self
            .resources
            .into_with_document_and_layers(doc, cur_layers);

        // set contents
        let mut layer_streams = Vec::<lopdf::Stream>::new();
        use lopdf::content::Operation;
        use lopdf::Object::*;

        for (idx, mut layer) in self.layers.into_iter().enumerate() {
            // push OCG and q to the beginning of the layer
            layer.operations.insert(0, Operation::new("q", vec![]));
            layer.operations.insert(
                0,
                Operation::new(
                    "BDC",
                    vec![Name("OC".into()), Name(ocg_refs[idx].name.clone().into())],
                ),
            );

            // push OCG END and Q to the end of the layer stream
            layer.operations.push(Operation::new("Q", vec![]));
            layer.operations.push(Operation::new("EMC", vec![]));

            // should end up looking like this:

            // /OC /MC0 BDC
            // q
            // <layer stream content>
            // Q
            // EMC

            let layer_stream = layer.into();
            layer_streams.push(layer_stream);
        }

        (resource_dictionary, layer_streams)
    }

    /// Change the graphics state. Before this operation is done, you should save
    /// the graphics state using the `save_graphics_state()` function. This will change the
    /// current graphics state until the end of the page or until the page is reset to the
    /// previous state.
    /// Returns the old graphics state, in case it was overwritten, as well as a reference
    /// to the currently active graphics state
    #[inline]
    pub fn add_graphics_state(
        &mut self,
        added_state: ExtendedGraphicsState,
    ) -> ExtendedGraphicsStateRef {
        self.resources.add_graphics_state(added_state)
    }

    /// __STUB__: Adds a pattern to the pages resources
    #[inline]
    pub fn add_pattern(&mut self, pattern: Pattern) -> PatternRef {
        self.resources.add_pattern(pattern)
    }

    /// __STUB__: Adds an XObject to the pages resources.
    /// __NOTE__: Watch out for scaling. Your XObject might be invisible or only 1pt x 1pt big
    #[inline]
    pub fn add_xobject(&mut self, xobj: XObject) -> XObjectRef {
        self.resources.add_xobject(xobj)
    }

    /// __STUB__: Adds a Link Annotation to the pages resources.
    #[inline]
    pub fn add_link_annotation(&mut self, annotation: LinkAnnotation) -> LinkAnnotationRef {
        self.resources.add_link_annotation(annotation)
    }

    
}

impl PdfPageReference {
    /// Adds a page and returns the index of the currently added page
    #[inline]
    pub fn add_layer<S>(&self, layer_name: S) -> PdfLayerReference
    where
        S: Into<String>,
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let page = &mut doc.pages[self.page.0];

        let current_page_index = page.layers.len(); /* order is important */
        let layer = PdfLayer::new(layer_name);
        page.layers.push(layer);
        let index = PdfLayerIndex(current_page_index);

        PdfLayerReference {
            document: self.document.clone(),
            page: self.page,
            layer: index,
        }
    }

    /// Validates that a layer is present and returns a reference to it
    #[inline]

    pub fn get_layer(&self, layer: PdfLayerIndex) -> PdfLayerReference {
        let doc = self.document.upgrade().unwrap();
        let doc = doc.borrow();

        let _ = &doc.pages[self.page.0].layers[layer.0];

        PdfLayerReference {
            document: self.document.clone(),
            page: self.page,
            layer,
        }
    }

    #[inline]
    pub fn extend_with(&self, dict: lopdf::Dictionary) {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let page = &mut doc.pages[self.page.0];
        page.extend_with = Some(dict);
    }
}
