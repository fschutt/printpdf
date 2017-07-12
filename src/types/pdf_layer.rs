//! PDF layer management. Layers can contain referenced or real content.
extern crate freetype as ft;

use *;
use indices::*;
use std::rc::Weak;
use std::cell::RefCell;

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the optional content group
    name: String,
    /// Stream objects in this layer. Usually, one layer == one stream
    operations: Vec<lopdf::content::Operation>,
}

#[derive(Debug, Clone)]
pub struct PdfLayerReference {
    pub document: Weak<RefCell<PdfDocument>>,
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
            operations: Vec::new(),
        }
    }
}

impl Into<lopdf::Stream> for PdfLayer {
    fn into(self)
    -> lopdf::Stream
    {
        use lopdf::{Stream, Dictionary};
        let stream_content = lopdf::content::Content { operations: self.operations };
        let stream = Stream::new(lopdf::Dictionary::new(), stream_content.encode().unwrap());
        /* contents may not be compressed! */
        return stream;
    }
}

impl PdfLayerReference {

    /// Add a shape to the layer. Use `closed` to indicate whether the line is a closed line
    /// Use has_fill to determine if the line should be filled. 
    pub fn add_shape(&self, line: Line)
    {
        self.internal_add_operation(Box::new(line));
    }

    /// Add an image to the layer
    /// To be called from the `image.add_to_layer()` class (see `use_xobject` documentation)
    pub(crate) fn add_image<T>(&self, image: T)
    -> XObjectRef where T: Into<ImageXObject>
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        page_mut.add_xobject(XObject::Image(image.into()))
    }

    /// Add an svg element to the layer
    /// To be called from the `svg.add_to_layer()` class (see `use_xobject` documentation)
    pub(crate) fn add_svg<T>(&self, form: T)
    -> std::result::Result<XObjectRef, T::Error> 
    where T: std::convert::TryInto<FormXObject>
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();
        let form_data = form.try_into()?;
        Ok(page_mut.add_xobject(XObject::Form(form_data)))
    }

    /// Instantiate layers, forms and postscript items on the page
    /// __WARNING__: Object must be added to the same page, since the XObjectRef is just a
    /// String, essentially, it can't be checked that this is the case. The caller is 
    /// responsible for ensuring this. However, you can use the `Image` struct 
    /// and use `image.add_to(layer)`, which will essentially do the same thing, but ensures
    /// that the image is referenced correctly
    ///
    /// Function is limited to this library to ensure that outside code cannot call it
    pub(crate) fn use_xobject(&self, xobj: XObjectRef, 
                        translate_x: Option<f64>, translate_y: Option<f64>,
                        rotate_cw: Option<f64>,
                        scale_x: Option<f64>, scale_y: Option<f64>)
    {
        // save graphics state
        self.save_graphics_state();

        // apply ctm if any
        let (mut s_x, mut s_y) = (0.0, 0.0);
        let (mut t_x, mut t_y) = (0.0, 0.0);

        if let Some(sc_x) = scale_x { s_x = sc_x; }
        if let Some(sc_y) = scale_y { s_y = sc_y; }
        if let Some(tr_x) = translate_x { t_x = tr_x; }
        if let Some(tr_y) = translate_y { t_y = tr_y; }

        // translate, rotate, scale - order does not matter

        if t_x != 0.0 || t_y != 0.0 { 
            let translate_ctm = CurTransMat::translate(t_x, t_y); 
            self.internal_add_operation(Box::new(translate_ctm)); 
        }

        if let Some(rot) = rotate_cw {
            let rotate_ctm = CurTransMat::rotate(rot); 
            self.internal_add_operation(Box::new(rotate_ctm));
        }

        if s_x != 0.0 || s_y != 0.0 {
            let scale_ctm = CurTransMat::scale(s_x, s_y); 
            self.internal_add_operation(Box::new(scale_ctm)); 
        }

        // invoke object
        self.internal_invoke_xobject(xobj.name);

        // restore graphics state
        self.restore_graphics_state();
    }

    // internal function to invoke an xobject
    fn internal_invoke_xobject(&self, name: String)
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        page_mut.layers.get_mut(self.layer.0).unwrap()
          .operations.push(lopdf::content::Operation::new(
              "Do", vec![lopdf::Object::Name(name.as_bytes().to_vec())]
        ));  
    }

    // internal function to add an operation (prevents locking)
    fn internal_add_operation(&self, op: Box<IntoPdfStreamOperation>)
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let op = op.into_stream_op();
        let layer = doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap();

        for o in op {
            layer.operations.push(o);
        }
                
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_fill_color(&self, fill_color: Color)
    -> ()
    {
        self.internal_add_operation(Box::new(PdfColor::FillColor(fill_color)));
    }

    /// Set the current line / outline color for the layer
    #[inline]
    pub fn set_outline_color(&self, color: Color)
    {
        self.internal_add_operation(Box::new(PdfColor::OutlineColor(color)));
    }

    /// Set the overprint mode of the stroke color to true (overprint) or false (no overprint)
    pub fn set_overprint_fill(&self, overprint: bool)
    {
        let new_overprint_state = ExtendedGraphicsStateBuilder::new()
                                      .with_overprint_fill(overprint)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_overprint_state);
        // add gs operator to stream
        page_mut.layers.get_mut(self.layer.0).unwrap()
            .operations.push(lopdf::content::Operation::new(
                "gs", vec![lopdf::Object::Name(new_ref.gs_name.as_bytes().to_vec())]
        ));
    }

    /// Set the overprint mode of the fill color to true (overprint) or false (no overprint)
    /// This changes the graphics state of the current page, don't do it too often or you'll bloat the file size
    pub fn set_overprint_stroke(&self, overprint: bool)
    {
        // this is technically an operation on the page level
        let new_overprint_state = ExtendedGraphicsStateBuilder::new()
                                      .with_overprint_stroke(overprint)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_overprint_state);
        page_mut.layers.get_mut(self.layer.0).unwrap()
            .operations.push(lopdf::content::Operation::new(
                "gs", vec![lopdf::Object::Name(new_ref.gs_name.as_bytes().to_vec())]
        ));
    }

    /// Set the overprint mode of the fill color to true (overprint) or false (no overprint)
    /// This changes the graphics state of the current page, don't do it too often or you'll bloat the file size
    pub fn set_blend_mode(&self, blend_mode: BlendMode)
    {
        // this is technically an operation on the page level
        let new_blend_mode_state = ExtendedGraphicsStateBuilder::new()
                                      .with_blend_mode(blend_mode)
                                      .build();
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let mut page_mut = doc.pages.get_mut(self.page.0).unwrap();

        let new_ref = page_mut.add_graphics_state(new_blend_mode_state);

        page_mut.layers.get_mut(self.layer.0).unwrap()
            .operations.push(lopdf::content::Operation::new(
                "gs", vec![lopdf::Object::Name(new_ref.gs_name.as_bytes().to_vec())]
        ));
    }

    /// Set the current line thickness
    #[inline]
    pub fn set_outline_thickness(&self, outline_thickness: i64)
    {
        use lopdf::Object::*;
        use lopdf::content::Operation;
        self.internal_add_operation(Box::new(Operation::new(OP_PATH_STATE_SET_LINE_WIDTH, vec![Integer(outline_thickness)])));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_join_style(&self, line_join: LineJoinStyle) {
        self.internal_add_operation(Box::new(line_join));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_cap_style(&self, line_cap: LineCapStyle) {
        self.internal_add_operation(Box::new(line_cap));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_dash_pattern(&self, dash_pattern: LineDashPattern) {
        self.internal_add_operation(Box::new(dash_pattern));
    }

    /// Set the current transformation matrix (TODO)
    #[inline]
    pub fn set_ctm(&self, ctm: CurTransMat) {
        self.internal_add_operation(Box::new(ctm));
    }

    /// Saves the current graphic state (q operator) (TODO)
    #[inline]
    pub fn save_graphics_state(&self) {
        self.internal_add_operation(Box::new(lopdf::content::Operation::new("q", Vec::new())));
    }

    /// Restores the previous graphic state (Q operator) (TODO)
    #[inline]
    pub fn restore_graphics_state(&self) {
        self.internal_add_operation(Box::new(lopdf::content::Operation::new("Q", Vec::new())));
    }

    /// Add text to the file
    #[inline]
    pub fn use_text<S>(&self, text: S, font_size: usize, rotation: f64,
                       x_mm: f64, y_mm: f64, font: FontRef)
    -> () where S: Into<std::string::String>
    {
            use lopdf::Object::*;
            use lopdf::StringFormat::Hexadecimal;
            use lopdf::content::Operation;

            // TODO !!! 
            // self.add_arbitrary_resource("Font", font.clone());

            // we need to transform the characters into glyph ids and then add them to the layer
            let doc = self.document.upgrade().unwrap();
            let mut doc = doc.borrow_mut();

            let library = ft::Library::init().unwrap();
            let face = library.new_memory_face(&*font.font_data, 0)
                              .expect("invalid memory font in use_text()");

            let text_to_embed = text.into();

            // convert into list of glyph ids - unicode magic
            let list_gid: Vec<u16> = text_to_embed
                       .chars()
                       .map(|x| face.get_char_index(x as usize) as u16)
                       .collect();

            let bytes: Vec<u8> = list_gid.iter()
                .flat_map(|x| vec!((x >> 8) as u8, (x & 255) as u8))
                .collect::<Vec<u8>>();

            // rotation missing, kerning missing

            let ref_mut_layer = doc.pages.get_mut(self.page.0).unwrap()
                                    .layers.get_mut(self.layer.0).unwrap();

            ref_mut_layer.operations.push(Operation::new("BT", 
                vec![]
            ));

            ref_mut_layer.operations.push(Operation::new("Tf", 
                vec![font.name.into(), (font_size as i64).into()]
            ));
            ref_mut_layer.operations.push(Operation::new("Td", 
                vec![x_mm.into(), y_mm.into()]
            ));

            ref_mut_layer.operations.push(Operation::new("Tj", 
                vec![String(bytes, Hexadecimal)]
            ));

            ref_mut_layer.operations.push(Operation::new("ET", 
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
            let doc = doc.borrow_mut();
            let element = doc.contents.get((svg_data_index.0).0).expect("invalid svg reference");
            (*element).clone()
        }; 
        
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();

        // todo: what about width / height?
        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .layer.place_back() <- PdfResource::ReferencedResource(svg_data_index.0.clone());
        */
    }
}