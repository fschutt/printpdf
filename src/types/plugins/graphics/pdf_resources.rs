use *;
use extgstate::ExtendedGraphicsState;

/// Struct for storing the PDF Resources, to be used on a PDF page
#[derive(Debug)]
pub struct PdfResources {
    /// External graphics objects
    pub xobjects: XObjectList,
    /// Fonts used on this page. May be references (todo)
    pub fonts: FontList,
    /// Patterns used on this page. Do not yet, use, placeholder.
    pub patterns: PatternList,
    /// Graphics states used on this page
    pub graphics_states: ExtendedGraphicsStateList,
}

impl PdfResources {

    /// Creates a new PdfResources struct (resources for exactly one PDF page)
    pub fn new()
    -> Self
    {
        Self {
            xobjects: XObjectList::new(),
            fonts: FontList::new(),
            patterns: PatternList::new(),
            graphics_states: ExtendedGraphicsStateList::new(),
        }
    }

    /// Add a graphics state to the resources
    #[inline]
    pub fn add_graphics_state(&mut self, added_state: ExtendedGraphicsState)
    -> ExtendedGraphicsStateRef
    {
        self.graphics_states.add_graphics_state(added_state)
    }

    /// Adds a font to the resources
    #[inline]
    pub fn add_font(&mut self, font: DirectFontRef)
    -> IndirectFontRef
    {
        self.fonts.add_font(font)
    }

    /// Returns a direct reference (object ID) to the font from an 
    /// indirect reference (postscript name)
    #[inline]
    pub fn get_font(&self, font: &IndirectFontRef)
    -> Option<&DirectFontRef>
    {
        self.fonts.get_font(font)
    }

    /// Adds an XObject to the page
    #[inline]
    pub fn add_xobject(&mut self, xobj: XObject)
    -> XObjectRef
    {
        self.xobjects.add_xobject(xobj)
    }

    /// __STUB__: Adds a pattern to the resources, to be used like a color
    #[inline]
    pub fn add_pattern(&mut self, pattern: Pattern)
    -> PatternRef
    {
        self.patterns.add_pattern(pattern)
    }
    
    /// See `XObject::Into_with_document`.
    pub fn into_with_document(self, doc: &mut lopdf::Document)
    -> lopdf::Dictionary
    {
            let mut dict = lopdf::Dictionary::new();
            let xobjects_dict: lopdf::Dictionary = self.xobjects.into_with_document(doc);
            let fonts_dict: lopdf::Dictionary = self.fonts.into();
            let patterns_dict: lopdf::Dictionary = self.patterns.into();
            let graphics_state_dict: lopdf::Dictionary = self.graphics_states.into();

            if xobjects_dict.len() > 0 {
                dict.set("XObject", lopdf::Object::Dictionary(xobjects_dict));
            }

            if fonts_dict.len() > 0 {
                dict.set("Font", lopdf::Object::Dictionary(fonts_dict));
            }

            if patterns_dict.len() > 0 {
                dict.set("Pattern", lopdf::Object::Dictionary(patterns_dict));
            }

            if graphics_state_dict.len() > 0 {
                dict.set("ExtGState", lopdf::Object::Dictionary(graphics_state_dict));
            }

            return dict;
    }
}