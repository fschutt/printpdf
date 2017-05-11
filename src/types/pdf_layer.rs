//! PDF layer management. Layers can contain referenced or real content.

use *;
use types::indices::*;
use types::indices::PdfContent::*;

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the OCG
    name: String,
    /// Element instantiated in this layer
    contents: Vec<PdfContent>
}

impl PdfLayer {
    
    /// Create a new layer
    #[inline]
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
            contents: Vec::new(),
        }
    }

    /// ## `add_*` functions for arbitrary PDF content

    /// Instantiate arbitrary pdf objects from the documents list of
    /// blobs / arbitrary pdf objects
    #[inline]
    pub fn add_arbitrary_content(&mut self, content_index: Box<IntoPdfObject>)
    {
        self.contents.push(PdfContent::ActualContent(content_index));
    }

    /// Add a line to the layer
    #[inline]
    pub fn add_shape(&mut self,
                     points: Vec<(Point, bool)>, 
                     outline: Option<&Outline>, 
                     fill: Option<&Fill>)
    -> ::std::result::Result<(), Error>
    {
        // todo
        Ok(())
    }

    /// Instantiate arbitrary pdf objects from the documents list of
    /// blobs / arbitrary pdf objects
    #[inline]
    pub fn use_arbitrary_content<C>(&mut self, content_index: PdfContentIndex)
    {
        self.contents.push(PdfContent::ReferencedContent(content_index)); 
    }

    /// Add text to the file
    #[inline]
    pub fn use_text<S>(&mut self,
                      text: S, 
                      font_size: usize,
                      x_mm: f64,
                      y_mm: f64,
                      font: FontIndex)
    -> ::std::result::Result<(), Error> where S: Into<String>
    {
        // todo
        Ok(())
    }

    /// Instantiate SVG data
    #[inline]
    pub fn use_svg(&mut self,
                   width_mm: f64,
                   height_mm: f64,
                   x_mm: f64,
                   y_mm: f64,
                   svg_data_index: SvgIndex)
    {
        // todo
    }

    /// Similar to the into_obj function, but takes the document as a second parameter (for lookup)
    /// and conformance checking
    /// Layers are prohibited if the conformance does not allow PDF layers. However, they are still
    /// used for z-indexing content
    fn into_obj(self: Box<Self>, doc: &PdfDocument)
    -> Vec<lopdf::Object>
    {
        let mut final_contents = Vec::<lopdf::Object>::new();

        if doc.metadata.conformance.is_layering_allowed() {
            // todo: write begin of pdf layer
        }

        for content in self.contents.into_iter() {
            match content {
                ActualContent(a) => { final_contents.place_back() <- a.into_obj(); },
                ReferencedContent(r) => { let content_ref = doc.contents.get(r.0).unwrap();
                                            final_contents.place_back() <- content_ref.clone();
                                        }
            }
        }

        if doc.metadata.conformance.is_layering_allowed() {
            // todo: write end of pdf layer
        }

        final_contents
    }
}