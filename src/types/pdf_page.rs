//! PDF page management

use *;
use indices::*;
use indices::PdfResource::*;
use std::sync::{Mutex, Weak};
use std::collections::HashMap;

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
    /// Resources used in this page. They can either be real objects (`ActualResource`)
    /// or references to other objects defined at the document level. If you are unsure,
    /// add the content to the document and use `ReferencedResource`.
    pub(crate) resources: HashMap<std::string::String, PdfResource>,
    /// Current indent level + current graphics state
    pub(crate) latest_graphics_state: (usize, ExtendedGraphicsState),
    /// All graphics states needed for this layer, collected together with a name for each one
    /// The name should be: "GS[index of the graphics state]", so `/GS0` for the first graphics state.
    pub(crate) all_graphics_states: HashMap<std::string::String, (usize, ExtendedGraphicsState)>,
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
            resources: HashMap::new(),
            latest_graphics_state: (0, ExtendedGraphicsState::default()),
            all_graphics_states: HashMap::new(),
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
    pub(crate) fn collect_resources_and_streams(self, contents: &Vec<lopdf::Object>)
    -> (lopdf::Dictionary, Vec<lopdf::Stream>)
    {
        let mut resource_dictionary = lopdf::Dictionary::new();

        // insert graphical states, todo: move this out of here
        let mut ext_g_state_resources = lopdf::Dictionary::new();

        for (name, (_, graphics_state)) in self.all_graphics_states.into_iter() {
            let gs: lopdf::Object = graphics_state.into();
            ext_g_state_resources.set(name.to_string(), gs);
        }

        if ext_g_state_resources.len() > 0 {
            resource_dictionary.set("ExtGState".to_string(), lopdf::Object::Dictionary(ext_g_state_resources));
        }

        for resource in self.resources {
            match resource.1 {
                ActualResource(a)     => {
                    let mut current_resources =  a.into_obj();
                    // if the resource has more than one thing in it (shouldn't happen), push an array
                    if current_resources.len() > 1 {
                        resource_dictionary.set(resource.0.clone(), lopdf::Object::Array(current_resources));
                    } else {
                       resource_dictionary.set(resource.0.clone(), current_resources.pop().unwrap());
                    }
                },                         // r is here actually a reference to a PDF reference
                ReferencedResource(r) => { let content_ref = contents.get(r.0).unwrap();
                                           resource_dictionary.set(resource.0.clone(), content_ref.clone()); }
            }
        }

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
    pub fn add_graphics_state(&mut self, added_state: ExtendedGraphicsState)
    -> ExtendedGraphicsStateRef
    {
        let gs_ref = ExtendedGraphicsStateRef::new(self.all_graphics_states.len());
        self.all_graphics_states.insert(gs_ref.gs_name.clone(), (self.latest_graphics_state.0, added_state.clone()));
        self.latest_graphics_state = (self.latest_graphics_state.0, added_state);
        gs_ref
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

    /// Add a resource to the pages resource dictionary. The resources of the seperate layers
    /// will be colleted when the page is saved.
    /// The following keys are reserved, do not use them: `ExtGState`, `Font`
    #[inline]
    pub fn add_arbitrary_resource<S>(&mut self, key: S, resource: Box<IntoPdfObject>)
    -> () where S: Into<String>
    {
        let key = key.into();

        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        let page_resources_mut = &mut doc.pages.get_mut(self.page.0).unwrap().resources;

        let res = page_resources_mut.insert(key.clone(), ActualResource(resource));
        
        if res.is_some() {
            warn!("On page {}, the resource \"{}\" was overwritten!", self.page.0, key);
        }
    }
}