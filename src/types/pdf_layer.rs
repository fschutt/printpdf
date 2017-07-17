//! PDF layer management. Layers can contain referenced or real content.
extern crate freetype as ft;

use *;
use indices::*;
use std::rc::Weak;
use std::cell::RefCell;
use lopdf::content::Operation;

/// One layer of PDF data
#[derive(Debug)]
pub struct PdfLayer {
    /// Name of the layer. Must be present for the optional content group
    name: String,
    /// Stream objects in this layer. Usually, one layer == one stream
    operations: Vec<lopdf::content::Operation>,
}

/// A "reference" to the current layer, allows for inner mutability
/// but only inside this library
#[derive(Debug, Clone)]
pub struct PdfLayerReference {
    /// A weak reference to the document, for inner mutability
    pub document: Weak<RefCell<PdfDocument>>,
    /// The index of the page this layer is on
    pub page: PdfPageIndex,
    /// The index of the layer this layer has (inside the page)
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
        /* page contents may not be compressed (todo: is this valid for XObjects?) */
        Stream::new(Dictionary::new(), stream_content.encode().unwrap())
                    .with_compression(false)
    }
}

impl PdfLayerReference {

    /// Add a shape to the layer. Use `closed` to indicate whether the line is a closed line
    /// Use has_fill to determine if the line should be filled. 
    pub fn add_shape(&self, line: Line)
    {
        let line_ops = Box::new(line).into_stream_op();
        for op in line_ops {
            self.internal_add_operation(op);
        }
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

    /// Begins a new text section
    /// You have to make sure to call `end_text_section` afterwards
    #[inline]
    pub fn begin_text_section(&self)
    -> ()
    {
        self.internal_add_operation(Operation::new("BT", vec![] ));
    }

    /// Ends a new text section
    /// Only valid if `begin_text_section` has been called
    #[inline]
    pub fn end_text_section(&self)
    -> ()
    {
        self.internal_add_operation(Operation::new("ET", vec![] ));
    }

    /// Set the current fill color for the layer
    #[inline]
    pub fn set_fill_color(&self, fill_color: Color)
    -> ()
    {
        self.internal_add_operation(PdfColor::FillColor(fill_color));
    }

    /// Set the current font, only valid in a `begin_text_section` to
    /// `end_text_section` block
    #[inline]
    pub fn set_font(&self, font: &IndirectFontRef, font_size: i64)
    -> ()
    {
        self.internal_add_operation(Operation::new("Tf", 
            vec![font.name.clone().into(), (font_size).into()]
        ));
    }

    /// Set the current line / outline color for the layer
    #[inline]
    pub fn set_outline_color(&self, color: Color)
    {
        self.internal_add_operation(PdfColor::OutlineColor(color));
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
            let translate_ctm = CurTransMat::Translate(t_x, t_y); 
            self.internal_add_operation(translate_ctm); 
        }

        if let Some(rot) = rotate_cw {
            let rotate_ctm = CurTransMat::Rotate(rot); 
            self.internal_add_operation(rotate_ctm);
        }

        if s_x != 0.0 || s_y != 0.0 {
            let scale_ctm = CurTransMat::Scale(s_x, s_y); 
            self.internal_add_operation(scale_ctm); 
        }

        // invoke object
        self.internal_invoke_xobject(xobj.name);

        // restore graphics state
        self.restore_graphics_state();
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
        self.internal_add_operation(Operation::new(OP_PATH_STATE_SET_LINE_WIDTH, vec![Integer(outline_thickness)]));
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_join_style(&self, line_join: LineJoinStyle) {
        self.internal_add_operation(line_join);
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_cap_style(&self, line_cap: LineCapStyle) {
        self.internal_add_operation(line_cap);
    }

    /// Set the current line join style for outlines
    #[inline]
    pub fn set_line_dash_pattern(&self, dash_pattern: LineDashPattern) {
        self.internal_add_operation(dash_pattern);
    }

    /// Sets (adds to) the current transformation matrix
    /// Use `save_graphics_state()` and `restore_graphics_state()`
    /// to "scope" the transformation matrix to a specific function
    #[inline]
    pub fn set_ctm(&self, ctm: CurTransMat) {
        self.internal_add_operation(ctm);
    }

    /// Sets (replaces) the current text matrix
    /// This does not have to be scoped, since the matrix is replaced
    /// instead of concatenated to the current matrix. However,
    /// you should only call this function with in a block scoped by 
    /// `begin_text_section()` and `end_text_section()`
    #[inline]
    pub fn set_text_matrix(&self, tm: TextMatrix) {
        self.internal_add_operation(tm);
    }

    /// Sets the position where the text should appear
    #[inline]
    pub fn set_text_cursor(&self, x: f64, y:f64) {
        self.internal_add_operation(Operation::new("Td", 
                vec![mm_to_pt!(x).into(), mm_to_pt!(y).into()]
        ));
    }

    /// If called inside a text block scoped by `begin_text_section` and
    /// `end_text_section`, moves the cursor to a new line. PDF does not have
    /// any concept of "alignment" except left-aligned text
    /// __Note:__ Use `set_line_height` earlier to set the line height first
    #[inline]
    pub fn add_line_break(&self) {
        self.internal_add_operation(Operation::new("T*", Vec::new()));
    }

    /// Sets the text line height inside a text block 
    /// (must be called within `begin_text_block` and `end_text_block`)
    #[inline]
    pub fn set_line_height(&self, height: i64) {
        self.internal_add_operation(Operation::new("TL", 
            vec![lopdf::Object::Integer(height)]
        ));
    }

    /// Sets the character spacing inside a text block
    /// Values are given in points. A value of 3 (pt) will increase 
    /// the spacing inside a word by 3pt.
    #[inline]
    pub fn set_character_spacing(&self, spacing: i64) {
        self.internal_add_operation(Operation::new("Tc", 
            vec![lopdf::Object::Integer(spacing)]
        ));
    }

    /// Sets the word spacing inside a text block. 
    /// Same as `set_character_spacing`, just for words
    /// __Note:__ This does currently not work, because PDF does not 
    /// recognize unicode fonts, only builtin fonts done with 
    /// PDFDoc encoding
    /// However, the function itself is valid
    #[inline]
    pub fn set_word_spacing(&self, spacing: i64) {
        self.internal_add_operation(Operation::new("Tw", 
            vec![lopdf::Object::Integer(spacing)]
        ));
    }

    /// Sets the horizontal scaling (like a "condensed" font)
    /// Default value is 100 (regular scaling). Setting it to 
    /// 50 will reduce the width of the written text by half,
    /// but stretch the text
    #[inline]
    pub fn set_text_scaling(&self, scaling: i64) {
        self.internal_add_operation(Operation::new("Tz", 
            vec![lopdf::Object::Integer(scaling)]
        ));
    }

    /// Offsets the current text positon (used for superscript 
    /// and subscript). To reset the superscript / subscript, call this 
    /// function with 0 as the offset. For superscript, use a positive
    /// number, for subscript, use a negative number. This does not 
    /// change the size of the font
    #[inline]
    pub fn set_line_offset(&self, offset: i64) {
        self.internal_add_operation(Operation::new("Ts", 
            vec![lopdf::Object::Integer(offset)]
        ));
    }

    #[inline]
    pub fn set_text_rendering_mode(&self, mode: TextRenderingMode) {
        self.internal_add_operation(Operation::new("Tr", 
            vec![lopdf::Object::Integer(mode.into())]
        ));
    }

    /// Sets the position where the text should appear (in mm)
    #[inline]
    pub fn write_text<S>(&self, text: S, font: &IndirectFontRef)
    -> () where S: Into<std::string::String>
    {
        use lopdf::Object::*;
        use lopdf::StringFormat::Hexadecimal;
        use lopdf::content::Operation;

        let text = text.into();
        
        // we need to transform the characters into glyph ids and then add them to the layer
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();

        // glyph IDs that make up this string
        let mut list_gid = Vec::<u16>::new();

        // kerning for each glyph id. If no kerning is present, will be 0
        // must be the same length as list_gid
        // let mut kerning_data = Vec::<freetype::Vector>::new();

        {
            let face_direct_ref = doc.fonts.get_font(&font).unwrap();
            let library = ft::Library::init().unwrap();
            let face = library.new_memory_face(&*face_direct_ref.data.font_bytes, 0)
                              .expect("invalid memory font in use_text()");

            // convert into list of glyph ids - unicode magic
            let char_iter = text.chars();
            let char_iter_2 = text.chars();
            let mut peekable = char_iter_2.peekable();
            peekable.next(); /* offset by 1 character */
            for ch in char_iter {
                list_gid.push(face.get_char_index(ch as usize) as u16);
/*
                // todo - kerning !!

                use freetype::face::KerningMode;

                if let Some(next) = peekable.peek() {
                    let char_next = next.clone();
                    let possible_kerning = face.get_kerning(ch as u32, char_next as u32, KerningMode::KerningDefault);
                    kerning_data.push(possible_kerning.unwrap_or(freetype::ffi::FT_Vector { x: 1000, y: 1000 }));
                }

                peekable.next();
*/
            }
        }

        let bytes: Vec<u8> = list_gid.iter()
            .flat_map(|x| vec!((x >> 8) as u8, (x & 255) as u8))
            .collect::<Vec<u8>>();

        doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap()
                .operations.push(Operation::new("Tj", 
                    vec![String(bytes, Hexadecimal)]
        ));
    }

    /// Saves the current graphic state
    #[inline]
    pub fn save_graphics_state(&self) {
        self.internal_add_operation(Operation::new("q", Vec::new()));
    }

    /// Restores the previous graphic state
    #[inline]
    pub fn restore_graphics_state(&self) {
        self.internal_add_operation(Operation::new("Q", Vec::new()));
    }

    /// Add text to the file
    #[inline]
    pub fn use_text<S>(&self, text: S, font_size: i64,
                       x_mm: f64, y_mm: f64, font: &IndirectFontRef)
    -> () where S: Into<std::string::String>
    {
            self.begin_text_section();
            self.set_font(font, font_size);
            self.set_text_cursor(x_mm, y_mm);
            self.write_text(text, font);
            self.end_text_section();
    }

/*
    /// Instantiate SVG data
    #[inline]
    pub fn use_svg(&self, width_mm: f64, height_mm: f64, 
                   x_mm: f64, y_mm: f64, svg_data_index: SvgIndex)
    {    
        let svg_element_ref = {
            let doc = self.document.upgrade().unwrap();
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
    }
*/

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
    fn internal_add_operation<T>(&self, op: T)
    -> () where T: Into<Operation>
    {
        let doc = self.document.upgrade().unwrap();
        let mut doc = doc.borrow_mut();
        let layer = doc.pages.get_mut(self.page.0).unwrap()
            .layers.get_mut(self.layer.0).unwrap();

        layer.operations.push(op.into());
    }
}