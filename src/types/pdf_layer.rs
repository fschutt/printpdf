//! PDF layer management. Layers can contain referenced or real content.
extern crate freetype as ft;

use *;
use indices::*;
use std::sync::{Mutex, Weak};

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the optional content group
    name: String,
    /// Stream objects in this layer. Usually, one layer == one stream
    layer_stream: PdfStream,
}

pub struct PdfLayerReference {
    pub document: Weak<Mutex<PdfDocument>>,
    pub page: PdfPageIndex,
    pub layer: PdfLayerIndex,
}

impl PdfLayer {
    
    /// Create a new layer, with a name and what index the layer has in the page
    #[inline]
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
            layer_stream: PdfStream::new(),
        }
    }

    /// Returns the layer stream
    pub fn into_obj(self)
    -> lopdf::Stream
    {
        self.layer_stream.into_obj()
    }
}

impl PdfLayerReference {

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
        let mut new_overprint_state = ExtendedGraphicsStateBuilder::new()
                                      .with_overprint_fill(true)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_overprint_state);
        if let Some(new) = new_ref {
            page_mut.layers.get_mut(self.layer.0).unwrap()
                .layer_stream.add_operations(Box::new(lopdf::content::Operation::new("gs", vec![lopdf::Object::Name(new.gs_name.as_bytes().to_vec())])));
        }
    }

    /// Set the overprint mode of the fill color to true (overprint) or false (no overprint)
    /// This changes the graphics state of the current page, don't do it too often or you'll bloat the file size
    pub fn set_overprint_stroke(&mut self, overprint: bool)
    {
        // this is technically an operation on the page level
        let mut new_overprint_state = ExtendedGraphicsStateBuilder::new()
                                      .with_overprint_stroke(true)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_overprint_state);
        if let Some(new) = new_ref {
            page_mut.layers.get_mut(self.layer.0).unwrap()
                .layer_stream.add_operations(Box::new(lopdf::content::Operation::new("gs", vec![lopdf::Object::Name(new.gs_name.as_bytes().to_vec())])));
        }
    }

    /// Set the overprint mode of the fill color to true (overprint) or false (no overprint)
    /// This changes the graphics state of the current page, don't do it too often or you'll bloat the file size
    pub fn set_blend_mode(&mut self, blend_mode: BlendMode)
    {
        // this is technically an operation on the page level
        let mut new_overprint_state = ExtendedGraphicsStateBuilder::new()
                                      .with_blend_mode(blend_mode)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.lock().unwrap();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_overprint_state);
        if let Some(new) = new_ref {
            page_mut.layers.get_mut(self.layer.0).unwrap()
                .layer_stream.add_operations(Box::new(lopdf::content::Operation::new("gs", vec![lopdf::Object::Name(new.gs_name.as_bytes().to_vec())])));
        }
    }

    /// Set the current outline for the layer
    #[inline]
    pub fn set_outline_color(&mut self, outline: Outline)
    {
        add_operation!(self, Box::new(outline));
    }

    /// Set the current line thickness
    #[inline]
    pub fn set_outline_thickness(&mut self, outline_thickness: i64)
    {
        use lopdf::Object::*;
        use lopdf::content::Operation;
        add_operation!(self, Box::new(Operation::new(OP_PATH_STATE_SET_LINE_WIDTH, vec![Integer(outline_thickness)])));
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
        add_operation_once!(self, dash_pattern);
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

            ref_mut_layer.layer_stream.add_operation(Operation::new("BT", 
                vec![]
            ));

            ref_mut_layer.layer_stream.add_operation(Operation::new("Tf", 
                vec![face_name.into(), (font_size as i64).into()]
            ));
            ref_mut_layer.layer_stream.add_operation(Operation::new("Td", 
                vec![x_mm.into(), y_mm.into()]
            ));

            ref_mut_layer.layer_stream.add_operation(Operation::new("Tj", 
                vec![String(bytes, Hexadecimal)]
            ));

            ref_mut_layer.layer_stream.add_operation(Operation::new("ET", 
                vec![]
            ));
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