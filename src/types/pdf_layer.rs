//! PDF layer management. Layers can contain referenced or real content.
extern crate freetype as ft;

use *;
use indices::*;
use indices::PdfResource::*;
use std::sync::{Arc, Mutex, Weak};

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the OCG
    name: String,
    /// Resources used in this layer. They can either be real objects (`ActualResource`)
    /// or references to other objects defined at the document level. If you are unsure,
    /// add the content to the document and use `ReferencedResource`.
    resources: Vec<(std::string::String, PdfResource)>,
    /// Stream objects in this layer. Usually, one layer == one stream
    layer_stream: PdfStream,
}

pub struct PdfLayerReference {
    pub document: Weak<Mutex<PdfDocument>>,
    pub page: PdfPageIndex,
    pub layer: PdfLayerIndex,
}

impl PdfLayer {
    
    /// Create a new layer
    #[inline]
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
            resources: Vec::new(),
            layer_stream: PdfStream::new(),
        }
    }

    /// Builds a dictionary-like thing from the resources needed by this page
    /// First tuple item: (string, dictionary) pair that should be added to the pages resources dictionary
    /// Second tuple struct: Stream object
    pub(crate) fn collect_resources_and_streams(self, contents: &Vec<lopdf::Object>)
    -> (Vec<(std::string::String, lopdf::Object)>, lopdf::Stream)
    {
        let mut layer_resources = Vec::<(std::string::String, lopdf::Object)>::new();

        for resource in self.resources.into_iter() {
            match resource.1 {
                ActualResource(a)     => {
                    let current_resources =  a.into_obj();
                    // if the resource has more than one thing in it (shouldn't happen), push an array
                    if current_resources.len() > 1 {
                        layer_resources.push((resource.0.clone(), lopdf::Object::Array(current_resources)));
                    } else {
                       layer_resources.push((resource.0.clone(), current_resources[0].clone()));
                    }
                },
                ReferencedResource(r) => { let content_ref = contents.get(r.0).unwrap();
                                           layer_resources.push((resource.0.clone(), content_ref.clone())); }
            }
        }

        let layer_streams = self.layer_stream.into_obj();
        return (layer_resources, layer_streams);
    }
}

impl PdfLayerReference {

    /// Add a resource to the pages resource dictionary. The resources of the seperate layers
    /// will be colleted when the page is saved.
    #[inline]
    pub fn add_arbitrary_resource<S>(&mut self, key: S, resource: Box<IntoPdfObject>)
    -> () where S: Into<String>
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .resources.push((key.into(), PdfResource::ActualResource(resource)));

    }

    /// Add a shape to the layer. Use `closed` to indicate whether the line is a closed line
    /// Use has_fill to determine if the line should be filled. 
    #[inline]
    pub fn add_shape(&self, line: Line)
    {
        add_operation!(self, Box::new(line));
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_fill(&self, fill_color: Fill)
    -> ()
    {
        add_operation!(self, Box::new(fill_color));
    }

    /// Set the overprint mode of the stroke color to true (overprint) or false (no overprint)
    pub fn set_overprint_fill(&self, overprint: bool)
    {
        /* let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        use lopdf::content::Operation;
        let operation = Operation::new("OP", vec![lopdf::Object::Boolean(overprint)]);
        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .layer_stream.add_operation(Box::new(operation)); */
    }

    /// Set the overprint mode of the fill color to true (overprint) or false (no overprint)
    pub fn set_overprint_stroke(&self, overprint: bool)
    {
        /* let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        use lopdf::content::Operation;
        let operation = Operation::new("op", vec![lopdf::Object::Boolean(overprint)]);
        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .layer_stream.add_operation(Box::new(operation));*/
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_outline(&mut self, outline: Outline)
    {
        add_operation!(self, Box::new(outline));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_join_style(&mut self, line_join: LineJoinStyle) {
        add_operation!(self, Box::new(line_join));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_cap_style(&mut self, line_cap: LineCapStyle) {
        add_operation!(self, Box::new(line_cap));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_dash_pattern(&mut self, dash_pattern: LineDashPattern) {
        add_operation!(self, Box::new(dash_pattern));
    }

    /// Set the current transformation matrix (TODO)
    #[inline]
    pub fn set_ctm(&mut self, ctm: CurrentTransformationMatrix) {
        add_operation!(self, Box::new(ctm));
    }

    /// Saves the current graphic state (q operator) (TODO)
    #[inline]
    pub fn save_graphics_state(&mut self) {
        add_operation!(self, Box::new(lopdf::content::Operation::new("q", Vec::new())));
    }

    /// Restores the previous graphic state (Q operator) (TODO)
    #[inline]
    pub fn restore_graphics_state(&mut self) {
        add_operation!(self, Box::new(lopdf::content::Operation::new("Q", Vec::new())));
    }

    /// Add text to the file
    #[inline]
    pub fn use_text<S>(&self, text: S, font_size: usize, rotation: f64,
                       x_mm: f64, y_mm: f64, font: FontIndex)
    -> () where S: Into<std::string::String>
    {
            use lopdf::Object::*;
            use lopdf::StringFormat::Hexadecimal;
            use lopdf::content::Operation;

            // TODO !!! 
            // self.add_arbitrary_resource("Font", font.clone());

            // we need to transform the characters into glyph ids and then add them to the layer
            let doc = self.document.upgrade().unwrap();
            let mut doc = doc.lock().unwrap();

            // load font from in-memory buffer
            // temporarily clone the font stream. This way we can still mutably borrow the document
            let font_idx = {
                let idx = doc.contents[(font.0).0].clone();
                match idx {
                    lopdf::Object::Reference(r) => r,
                    _ => panic!(),
                }
            };


            let list_gid: Vec<u16>;
            let face_name;

            {
                let font =  doc.inner_doc.get_object(font_idx).unwrap();
                                
                let font_data = match *font {
                    lopdf::Object::Stream(ref s) => s,
                    _ => { panic!("use_text() called with a corrupt font index!") }
                };

                let library = ft::Library::init().unwrap();
                let face = library.new_memory_face(&*font_data.content, 0)
                                  .expect("invalid memory font in use_text()");

                face_name = face.postscript_name().unwrap();
                let text_to_embed = text.into();

                // convert into list of glyph ids - unicode magic
                list_gid = text_to_embed
                           .chars()
                           .map(|x| face.get_char_index(x as usize) as u16)
                           .collect();
            }

            let bytes: Vec<u8> = list_gid.iter()
                .flat_map(|x| vec!((x >> 8) as u8, (x & 255) as u8))
                .collect::<Vec<u8>>();

            // rotation missing, kerning missing

            let ref_mut_layer = doc.pages.get_mut(self.page.0).unwrap()
                                    .layers.get_mut(self.layer.0).unwrap();

            ref_mut_layer.layer_stream.add_operation(Box::new(Operation::new("BT", 
                vec![]
            )));

            ref_mut_layer.layer_stream.add_operation(Box::new(Operation::new("Tf", 
                vec![face_name.into(), (font_size as i64).into()]
            )));
            ref_mut_layer.layer_stream.add_operation(Box::new(Operation::new("Td", 
                vec![x_mm.into(), y_mm.into()]
            )));

            ref_mut_layer.layer_stream.add_operation(Box::new(Operation::new("Tj", 
                vec![String(bytes, Hexadecimal)]
            )));

            ref_mut_layer.layer_stream.add_operation(Box::new(Operation::new("ET", 
                vec![]
            )));
    }

    /// Instantiate SVG data
    #[inline]
    pub fn use_svg(&self, width_mm: f64, height_mm: f64, 
                   x_mm: f64, y_mm: f64, svg_data_index: SvgIndex)
    {
        /* 
        
        let svg_element_ref = {
            use std::clone::Clone;
            let doc = doc.lock().unwrap();
            let element = doc.contents.get((svg_data_index.0).0).expect("invalid svg reference");
            (*element).clone()
        }; 
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();

        // todo: what about width / height?
        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .layer.place_back() <- PdfResource::ReferencedResource(svg_data_index.0.clone());
        */
    }
}