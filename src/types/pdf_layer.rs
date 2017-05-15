//! PDF layer management. Layers can contain referenced or real content.
extern crate freetype as ft;

use *;
use types::indices::*;
use types::indices::PdfContent::*;
use std::sync::{Arc, Mutex, Weak};

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the OCG
    name: String,
    /// Element instantiated in this layer
    contents: Vec<PdfContent>,
    /// Stream objects in this layer. Usually, one layer == one stream
    layer_stream: PdfStream,
    /// Page this layer is on
    document: Weak<Mutex<PdfDocument>>,
}

impl PdfLayer {
    
    /// Create a new layer
    #[inline]
    pub fn new<S>(name: S, document: Weak<Mutex<PdfDocument>>)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
            contents: Vec::new(),
            document: document,
            layer_stream: PdfStream::new(),
        }
    }

    /// Instantiate arbitrary pdf objects from the documents list of
    /// blobs / arbitrary pdf objects
    #[inline]
    pub fn use_arbitrary_content<T>(&mut self, 
                                    content_index: T)
    -> () where T: Into<PdfContentIndex>
    {
        self.contents.push(PdfContent::ReferencedContent(content_index.into()));
    }

    /// Instantiate arbitrary pdf objects by directly adding them to the layer
    #[inline]
    pub fn add_arbitrary_content(&mut self, content: Box<IntoPdfObject>)
    {
        self.contents.place_back() <- PdfContent::ActualContent(content);

    }

    /// Add a shape to the layer. Use `closed` to indicate whether the line is a closed line
    /// Use has_fill to determine if the line should be filled. 
    #[inline]
    pub fn add_shape(&mut self,
                     points: Vec<(Point, bool)>, 
                     closed: bool,
                     has_fill: bool)
    -> ::std::result::Result<(), Error>
    {
        let line = Line::new(points, closed, has_fill);
        self.layer_stream.add_operation(Box::new(line));
        Ok(())
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_fill(&mut self, fill_color: Fill)
    -> ()
    {
        self.layer_stream.add_operation(Box::new(fill_color));
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_outline(&mut self, outline: Outline)
    -> ()
    {
        self.layer_stream.add_operation(Box::new(outline));
    }

    /// Add text to the file
    #[inline]
    pub fn use_text<S>(&mut self,
                       text: S, 
                       font_size: usize,
                       x_mm: f64,
                       y_mm: f64,
                       font: FontIndex)
    -> () where S: Into<std::string::String>
    {
            use lopdf::Object::*;
            use lopdf::{Stream as LoStream, Dictionary as LoDictionary};
            use lopdf::StringFormat::*;
            use lopdf::content::Operation;

            // we need to transform the characters into glyph ids and then add them to the layer
            let doc = self.document.upgrade().expect("use_text: could not upgrade pointer to document: no document");

            // load font from in-memory buffer
            // temporarily clone the font stream. This way we can still mutably borrow the document
            let mut font_idx = {
                let idx = doc.lock().unwrap().contents[(font.0).0].clone();
                match idx {
                    lopdf::Object::Reference(r) => r,
                    _ => panic!(),
                }
            };

            let list_gid: Vec<u16>;
            let face_name;

            {
                let doc = doc.lock().unwrap();
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

                // convert into list of glyph ids
                list_gid = text_to_embed.chars().map(|x| face.get_char_index(x as usize) as u16)
                                     .collect();
            }

            // let str: Vec<u16> = string.encode_utf16().collect();
            let bytes: Vec<u8> = list_gid.iter()
                .flat_map(|x| vec!((x >> 8) as u8, (x & 255) as u8))
                .collect::<Vec<u8>>();

            // rotation missing, kerning missing

            self.layer_stream.add_operation(Box::new(Operation::new("BT", 
                vec![]
            )));

            self.layer_stream.add_operation(Box::new(Operation::new("Tf", 
                vec![face_name.into(), (font_size as i64).into()]
            )));
            self.layer_stream.add_operation(Box::new(Operation::new("Td", 
                vec![x_mm.into(), y_mm.into()]
            )));

            self.layer_stream.add_operation(Box::new(Operation::new("Tj", 
                vec![String(bytes, Hexadecimal)]
            )));

            self.layer_stream.add_operation(Box::new(Operation::new("ET", 
                vec![]
            )));
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
        let doc = self.document.upgrade().expect("use_svg: Could not upgrade weak pointer to document, 
                                                       document does not exist");
        let svg_element_ref = {
            use std::clone::Clone;
            let doc = doc.lock().unwrap();
            let element = doc.contents.get((svg_data_index.0).0).expect("invalid svg reference");
            (*element).clone()
        };

        // todo: what about width / height?
        self.contents.place_back() <- PdfContent::ReferencedContent(svg_data_index.0.clone());
    }

    /// Similar to the into_obj function, but takes the document as a second parameter (for lookup)
    /// and conformance checking
    /// Layers are prohibited if the conformance does not allow PDF layers. However, they are still
    /// used for z-indexing content
    fn into_obj(self: Box<Self>, document: &PdfDocument)
    -> Vec<lopdf::Object>
    {
        let mut final_contents = Vec::<lopdf::Object>::new();

        if document.metadata.conformance.is_layering_allowed() {
            // todo: write begin of pdf layer
        }

        /// TODO: if two items are ActualContent and the type is stream,
        /// we can merge the streams together into one

        for content in self.contents.into_iter() {
            match content {
                ActualContent(a)     => { final_contents.append(&mut a.into_obj()); },
                ReferencedContent(r) => { 
                                            let content_ref = document.contents.get(r.0).unwrap();
                                            final_contents.place_back() <- content_ref.clone();
                                        }
            }
        }

        if document.metadata.conformance.is_layering_allowed() {
            // todo: write end of pdf layer
        }

        final_contents
    }
}