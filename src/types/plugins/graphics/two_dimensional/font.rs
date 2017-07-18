//! Embedding fonts in 2D for Pdf
extern crate lopdf;
extern crate rusttype;

use *;
use std::collections::HashMap;
use rusttype::FontCollection;
use rusttype::CodepointOrGlyphId::Codepoint as Cpg;
use rusttype::CodepointOrGlyphId::GlyphId as Cgid;
use rusttype::Codepoint as Cp;
use rusttype::GlyphId as Gid;

/// The font
#[derive(Debug, Clone)]
pub struct Font {
    /// Font data
    pub(crate) font_bytes: Vec<u8>,
    /// Font name, for adding as a resource on the document
    pub(crate) face_name: String,
    /// Is the font written vertically? Default: false
    pub(crate) vertical_writing: bool,
}

/// The text rendering mode determines how a text is drawn
/// The default rendering mode is `Fill`. The color of the 
/// fill / stroke is determine by the current pages outline / 
/// fill color.
///
/// See PDF Reference 1.7 Page 402
pub enum TextRenderingMode {
    Fill,
    Stroke,
    FillStroke,
    Invisible,
    FillClip,
    StrokeClip,
    FillStrokeClip,
    Clip,
}

impl Into<i64> for TextRenderingMode {
    fn into(self)
    -> i64
    {
        use TextRenderingMode::*;
        match self {
            Fill => 0,
            Stroke => 1,
            FillStroke => 2,
            Invisible => 3,
            FillClip => 4,
            StrokeClip => 5,
            FillStrokeClip => 6,
            Clip => 7,
        }
    }
}

impl Font {

    /// Creates a new font. The `index` is used for naming / identifying the font
    pub fn new<R>(mut font_stream: R, font_index: usize)
    -> std::result::Result<Self, Error> where R: ::std::io::Read
    {
        // read font from stream and parse font metrics
        let mut buf = Vec::<u8>::new();
        font_stream.read_to_end(&mut buf)?;

        let face_name = {
            let collection = FontCollection::from_bytes(buf.clone());
            let font = collection.clone().into_font();

            if let None = font {
                if let None = collection.into_fonts().nth(0) {
                    return Err(Error::from_kind(ErrorKind::FontError));
                }
            }

            format!("F{}", font_index)
        };

        Ok(Self {
            font_bytes: buf,
            face_name: face_name,
            vertical_writing: false,
        })
    }

    /// Takes the font and adds it to the document and consumes the font
    pub(crate) fn into_with_document(self, doc: &mut lopdf::Document)
    ->lopdf::Dictionary
    {
        use lopdf::Object::*;
        use lopdf::Object;
        use lopdf::{Stream as LoStream, Dictionary as LoDictionary};
        use lopdf::StringFormat;
        use std::collections::BTreeMap;
        use std::iter::FromIterator;

        let face_name = self.face_name.clone();

        let font_buf_ref: Box<[u8]> = self.font_bytes.into_boxed_slice();
        let collection = FontCollection::from_bytes(font_buf_ref.clone());
        let font = collection.clone().into_font().unwrap_or(collection.into_fonts().nth(0).unwrap());

        // Extract basic font information
        let face_metrics = font.v_metrics_unscaled();

        let font_stream = LoStream::new(
            LoDictionary::from_iter(vec![
                ("Length1", Integer(font_buf_ref.len() as i64)),
                ("Subtype", Name("CIDFontType0C".into())),
                ]),
            font_buf_ref.to_vec())
        .with_compression(false); /* important! font stream must not be compressed! */

        // Begin setting required font attributes
        let mut font_vec: Vec<(std::string::String, Object)> = vec![
            ("Type".into(), Name("Font".into())),
            ("Subtype".into(), Name("Type0".into())),
            ("BaseFont".into(), Name(face_name.clone().into_bytes())),
            // Identity-H for horizontal writing, Identity-V for vertical writing
            ("Encoding".into(), Name("Identity-H".into())),
            // Missing DescendantFonts and ToUnicode
        ];

        let mut font_descriptor_vec: Vec<(std::string::String, Object)> = vec![
            ("Type".into(), Name("FontDescriptor".into())),
            ("FontName".into(), Name(face_name.clone().into_bytes())),
            ("Ascent".into(), Integer(face_metrics.ascent as i64)),
            ("Descent".into(), Integer(face_metrics.descent as i64)),
            ("CapHeight".into(), Integer(face_metrics.ascent as i64)),
            ("ItalicAngle".into(), Integer(0)),
            ("Flags".into(), Integer(32)),
            ("StemV".into(), Integer(80)),
        ];

        // End setting required font arguments

        // Maximum height of a single character in the font
        let mut max_height = 0;
        // Total width of all characters
        let mut total_width = 0;
        // Widths (or heights, depends on self.vertical_writing) 
        // of the individual characters, indexed by glyph id
        let mut widths = HashMap::<u32, u32>::new();
        // Height of the space (0x0020 character), to scale the font correctly
        let mut space_height = 1000;

        // Glyph IDs - (Unicode IDs - character width, character height)
        let mut cmap = BTreeMap::<u32, (u32, u32, u32)>::new();
        cmap.insert(0, (0, 1000, 1000));

        for unicode in 0x0000..0xffff {
            let glyph = font.glyph(Cpg(Cp(unicode)));
            if let Some(glyph) = glyph {

                if glyph.id().0 == 0 { continue; }
                let glyph_id = glyph.id().0;

                if let Some(glyph) = font.glyph(Cgid(Gid(glyph_id))) {
                    if let Some(glyph_metrics) = glyph.standalone().get_data() {                        
                        if let Some(extents) = glyph_metrics.extents {
                            let w = glyph_metrics.unit_h_metrics.advance_width;
                            let h = extents.max.y - extents.min.y - face_metrics.descent as i32;
                            
                            // large T
                            if unicode == 0x0000 { space_height = h; }

                            if h > max_height { max_height = h; };

                            total_width += w as u32;
                            cmap.insert(glyph_id, (unicode as u32, w as u32, h as u32));
                        }
                    }
                }
            }
        }

        // Maps the character index to a unicode value
        // Add this to the "ToUnicode" dictionary
        // To explain this structure: Glyph IDs have to be in segments where the first byte of the
        // first and last element have to be the same. A range from 0x1000 - 0x10FF is valid
        // but a range from 0x1000 - 0x12FF is not (0x10 != 0x12)
        // Plus, the maximum number of Glyph-IDs in one range is 100
        // Since the glyph IDs are sequential, all we really have to do is to enumerate the vector
        // and create buckets of 100 / rest to 256 if needed
        let mut cid_to_unicode_map = format!(include_str!("../../../../templates/gid_to_unicode_beg.txt"), 
                                             face_name.clone());

        let mut cur_block_id: u32 = 0;          // ID of the block, to be used it {} beginbfchar
        let mut cur_first_bit: u16 = 0_u16;     // current first bit of the glyph id (0x10 or 0x12) for example
        let mut last_block_begin: u32 = 0;      // glyph ID of the start of the current block,
                                                // to satisfy the "less than 100 entries per block" rule

        for (glyph_id, unicode_width_tuple) in cmap.iter() {

            if (*glyph_id >> 8) as u16 != cur_first_bit || *glyph_id > last_block_begin + 100 {
                cid_to_unicode_map.push_str("endbfchar\r\n");
                cur_block_id += 1;
                last_block_begin = *glyph_id;
                cur_first_bit = (*glyph_id >> 8) as u16;
                cid_to_unicode_map.push_str(format!("{} beginbfchar\r\n", cur_block_id).as_str());
            }

            let unicode = unicode_width_tuple.0;
            let width = unicode_width_tuple.1;
            let height = unicode_width_tuple.2;

            cid_to_unicode_map.push_str(format!("<{:04x}> <{:04x}>\n", glyph_id, unicode).as_str());
            widths.insert(*glyph_id, width);
        };

        if cmap.len() % 256 != 0 || cmap.len() % 100 != 0 {
            cid_to_unicode_map.push_str("endbfchar\r\n");
        }

        cid_to_unicode_map.push_str(include_str!("../../../../templates/gid_to_unicode_end.txt"));
        
        let cid_to_unicode_map_stream = LoStream::new(LoDictionary::new(), cid_to_unicode_map.as_bytes().to_vec());
        let cid_to_unicode_map_stream_id = doc.add_object(cid_to_unicode_map_stream);

        // encode widths / heights so that they fit into what PDF expects
        // see page 439 in the PDF 1.7 reference
        // basically widths_list will contain objects like this:
        // 20 [21, 99, 34, 25]
        // which means that the character with the GID 20 has a width of 21 units
        // and the character with the GID 21 has a width of 99 units
        let mut widths_list = Vec::<Object>::new();
        let mut current_low_gid = 0;
        let mut current_high_gid = 0;
        let mut current_width_vec = Vec::<Object>::new();

        // scale the font width so that it sort-of fits into an 1000 unit square
        let percentage_font_scaling = 1000.0 / (space_height as f64);

        for (gid, width) in widths.into_iter() {
            if gid == current_high_gid {
                current_width_vec.push(Integer((width as f64 * percentage_font_scaling) as i64));
                current_high_gid += 1;
            } else {
                widths_list.push(Integer(current_low_gid as i64));
                widths_list.push(Array(current_width_vec.drain(..).collect()));
                current_width_vec.push(Integer((width as f64 * percentage_font_scaling) as i64));
                current_low_gid = gid;
                current_high_gid = gid + 1;
            }
        }

        let w = { 
            if self.vertical_writing { ("W2",  Array(widths_list)) }
            else { ("W",  Array(widths_list)) }
        };

        // default width for characters
        let dw = { 
            if self.vertical_writing { ("DW2", Integer(1000)) }
            else { ("DW", Integer(1000)) }
        };
        
        let mut desc_fonts = LoDictionary::from_iter(vec![
            ("Type", Name("Font".into())),
            ("Subtype", Name("CIDFontType0".into())),
            ("BaseFont", Name(face_name.clone().into())),
            ("CIDSystemInfo", Dictionary(LoDictionary::from_iter(vec![
                    ("Registry", String("Adobe".into(), StringFormat::Literal)),
                    ("Ordering", String("Identity".into(), StringFormat::Literal)),
                    ("Supplement", Integer(0)),
            ]))),
            w, dw,
        ]);

        let font_bbox = vec![ Integer(0), Integer(max_height as i64), Integer(total_width as i64), Integer(max_height as i64) ];
        font_descriptor_vec.push(("FontFile3".into(), Reference(doc.add_object(font_stream))));
        
        // although the following entry is technically not needed, Adobe Reader needs it
        font_descriptor_vec.push(("FontBBox".into(), Array(font_bbox)));
        
        let font_descriptor_vec_id = doc.add_object(LoDictionary::from_iter(font_descriptor_vec));

        desc_fonts.set("FontDescriptor", Reference(font_descriptor_vec_id));

        font_vec.push(("DescendantFonts".into(), Array(vec![Dictionary(desc_fonts)])));
        font_vec.push(("ToUnicode".into(), Reference(cid_to_unicode_map_stream_id)));
               
        lopdf::Dictionary::from_iter(font_vec)
    }
}

impl PartialEq for Font {
    /// Two fonts are equal if their names are equal, the contents aren't checked
    fn eq(&self, other: &Font) -> bool {
        self.face_name == other.face_name
    }
}

/// Indexed reference to a font that was added to the document
/// This is a "reference by postscript name"
#[derive(Debug, Hash, Eq, Clone, PartialEq)]
pub struct IndirectFontRef {
    /// Name of the font (postscript name)
    pub(crate) name: String,
}

/// Direct reference (wrapper for lopdf::Object::Reference) 
/// for increased type safety
#[derive(Debug, Clone)]
pub struct DirectFontRef {
    /// Reference to the content in the document stream
    pub(crate) inner_obj: lopdf::ObjectId,
    /// Actual font data 
    pub(crate) data: Font,
}

impl IndirectFontRef {
    /// Creates a new IndirectFontRef from an index
    pub fn new<S>(name: S)
    -> Self where S: Into<String>
    {
        Self {
            name: name.into(),
        }
    }
}

/// Font list for tracking fonts within a single PDF document
#[derive(Debug)]
pub struct FontList {
    fonts: HashMap<IndirectFontRef, DirectFontRef>,
}

impl FontList {
    
    /// Creates a new FontList
    pub fn new()
    -> Self
    {
        Self {
            fonts: HashMap::new(),
        }
    }

    /// Adds a font to the FontList
    pub fn add_font(&mut self, font_ref: IndirectFontRef, font: DirectFontRef)
    -> IndirectFontRef
    {
        self.fonts.insert(font_ref.clone(), font);
        font_ref
    }

    /// Turns an indirect font reference into a direct one 
    /// (Warning): clones the direct font reference
    #[inline]
    pub fn get_font(&self, font: &IndirectFontRef)
    -> Option<DirectFontRef>
    {
        let font_ref = self.fonts.get(font);
        if let Some(r) = font_ref {
            Some(r.clone())
        } else {
            None
        }
    }

    /// Returns the number of fonts currenly in use
    #[inline]
    pub fn len(&self)
    -> usize 
    {
        self.fonts.len()
    }

    pub(crate) fn into_with_document(self, doc: &mut lopdf::Document)
    ->lopdf::Dictionary
    {
        let mut font_dict = lopdf::Dictionary::new();

        for (indirect_ref, direct_font_ref) in self.fonts.into_iter() {

            let font_dict_collected = direct_font_ref.data.into_with_document(doc);
            doc.objects.insert(direct_font_ref.inner_obj.clone(), lopdf::Object::Dictionary(font_dict_collected));
            font_dict.set(indirect_ref.name,lopdf::Object::Reference(direct_font_ref.inner_obj));
        }

        return font_dict;
    }
}