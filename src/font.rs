#![allow(trivial_numeric_casts)]

//! Embedding fonts in 2D for Pdf
use crate::{Error, PdfPage};
use lopdf;
use lopdf::StringFormat;
use lopdf::{Dictionary as LoDictionary, Stream as LoStream};
use owned_ttf_parser::{AsFaceRef as _, Face, OwnedFace};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;
use std::rc::Rc;

/// The font
#[derive(Debug, Clone, PartialEq)]
pub enum Font {
    /// Represents one of the 14 built-in fonts (Arial, Helvetica, etc.)
    BuiltinFont(BuiltinFont),
    /// Represents a font loaded from an external file
    ExternalFont(ExternalFont),
}

/// Standard built-in PDF fonts
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BuiltinFont {
    TimesRoman,
    TimesBold,
    TimesItalic,
    TimesBoldItalic,
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    Courier,
    CourierOblique,
    CourierBold,
    CourierBoldOblique,
    Symbol,
    ZapfDingbats,
}

impl From<BuiltinFont> for &'static str {
    fn from(val: BuiltinFont) -> Self {
        use crate::BuiltinFont::*;
        match val {
            TimesRoman => "Times-Roman",
            TimesBold => "Times-Bold",
            TimesItalic => "Times-Italic",
            TimesBoldItalic => "Times-BoldItalic",
            Helvetica => "Helvetica",
            HelveticaBold => "Helvetica-Bold",
            HelveticaOblique => "Helvetica-Oblique",
            HelveticaBoldOblique => "Helvetica-BoldOblique",
            Courier => "Courier",
            CourierOblique => "Courier-Oblique",
            CourierBold => "Courier-Bold",
            CourierBoldOblique => "Courier-BoldOblique",
            Symbol => "Symbol",
            ZapfDingbats => "ZapfDingbats",
        }
    }
}

impl From<BuiltinFont> for LoDictionary {
    fn from(val: BuiltinFont) -> Self {
        use lopdf::Object;
        use lopdf::Object::*;

        let font_id: &'static str = val.into();

        // Begin setting required font attributes
        let font_vec: Vec<(::std::string::String, Object)> = vec![
            ("Type".into(), Name("Font".into())),
            ("Subtype".into(), Name("Type1".into())),
            ("BaseFont".into(), Name(font_id.into())),
            ("Encoding".into(), Name("WinAnsiEncoding".into())),
            // Missing DescendantFonts and ToUnicode
        ];

        LoDictionary::from_iter(font_vec)
    }
}

#[derive(Debug, Clone)]
pub struct ExternalFont {
    /// Raw font data
    pub(crate) font_bytes: Vec<u8>,
    /// Parsed font data
    pub(crate) font_data: Box<dyn FontData>,
    /// Font name, for adding as a resource on the document
    pub(crate) face_name: String,
    /// Is the font written vertically? Default: false
    pub(crate) vertical_writing: bool,
    /// Is the font allowed to be subsetted (removing unused glyphs before embedding)? Default: true
    #[allow(unused)]
    pub(crate) allow_subsetting: Rc<RefCell<bool>>,
}

/// The text rendering mode determines how a text is drawn
/// The default rendering mode is `Fill`. The color of the
/// fill / stroke is determine by the current pages outline /
/// fill color.
///
/// See PDF Reference 1.7 Page 402
#[derive(Debug, Copy, Clone)]
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

impl From<TextRenderingMode> for i64 {
    fn from(val: TextRenderingMode) -> Self {
        use crate::TextRenderingMode::*;
        match val {
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

impl ExternalFont {
    /// Creates a new font. The `index` is used for naming / identifying the font
    ///
    /// This method uses [`owned_ttf_parser`][] to parse the font data.  If you want to use a different
    /// font backend, use the [`with_font_data`][] method instead.
    ///
    /// [`owned_ttf_parser`]: https://docs.rs/owned_ttf_parser/latest/owned_ttf_parser/
    /// [`with_font_data`]: #method.with_font_data
    pub fn new<R>(mut font_stream: R, font_index: usize) -> Result<Self, Error>
    where
        R: ::std::io::Read,
    {
        // read font from stream and parse font metrics
        let mut buf = Vec::<u8>::new();
        font_stream.read_to_end(&mut buf)?;

        let font = TtfFace::from_vec(buf.clone())?;

        Ok(Self::with_font_data(buf, font_index, Box::new(font)))
    }

    /// Creates a new font. The `index` is used for naming / identifying the font
    pub fn with_font_data(bytes: Vec<u8>, font_index: usize, font_data: Box<dyn FontData>) -> Self {
        let face_name = format!("F{font_index}");
        Self {
            font_bytes: bytes,
            font_data,
            face_name,
            vertical_writing: false,
            allow_subsetting: Rc::new(RefCell::new(true)),
        }
    }

    /// Set whether or not to allow subsetting for the font. If subsetting is set to true, unused
    /// glyphs will be removed before embedding the font into the PDF file. By default this is set
    /// to `true`
    #[cfg(feature = "font_subsetting")]
    pub fn set_allow_subsetting(&self, allow_subsetting: bool) {
        *self.allow_subsetting.borrow_mut() = allow_subsetting;
    }

    /// Set whether or not to allow subsetting for the font. If subsetting is set to true, unused
    /// glyphs will be removed before embedding the font into the PDF file. By default this is set
    /// to `true`
    #[cfg(feature = "font_subsetting")]
    pub fn with_allow_subsetting(self, allow_subsetting: bool) -> Self {
        *self.allow_subsetting.borrow_mut() = allow_subsetting;
        self
    }

    /// Iterate through all layers of the provided pages and build a Set of all Glyph IDs that were
    /// used at least once
    #[cfg(feature = "font_subsetting")]
    fn find_used_glyphs(&self, pages: &[PdfPage]) -> std::collections::HashSet<u16> {
        // TODO: This is almost the same code as `replace_glyphs`. Somehow reduce redundancy
        let mut used_glyphs = std::collections::HashSet::new();

        let layers = pages.iter().map(|page| page.layers.iter()).flatten();
        for layer in layers {
            let operations = layer
                .operations
                .iter()
                .map(|op| (&op.operator, &op.operands));

            let mut font_active = false;

            for (operator, operands) in operations {
                match operator.as_str() {
                    "Tf" => {
                        let font_name = operands[0]
                            .as_name_str()
                            .expect("PDF Command 'Tf' not followed by Name operand");
                        font_active = font_name == &self.face_name;
                    }
                    "Tj" if font_active => {
                        let gid_stream = operands[0]
                            .as_str()
                            .expect("PDF Command 'Tj' not followed by String operand");
                        for b in gid_stream.chunks_exact(2) {
                            let gid = b[1] as u16 | ((b[0] as u16) << 8);
                            used_glyphs.insert(gid);
                        }
                    }
                    "TJ" if font_active => {
                        let text_sections = operands[0]
                            .as_array()
                            .expect("PDF Command 'TJ' not followed by Array operand")
                            .into_iter()
                            .filter_map(|obj| obj.as_str().ok());

                        for gid_stream in text_sections {
                            for b in gid_stream.chunks_exact(2) {
                                let gid = b[1] as u16 | ((b[0] as u16) << 8);
                                used_glyphs.insert(gid);
                            }
                        }
                    }
                    _ => (),
                }
            }
        }

        used_glyphs
    }

    /// Iterate through all layers of the provided pages and replace the Glyph ID according to the
    /// mapping
    #[cfg(feature = "font_subsetting")]
    fn replace_glyphs(&self, pages: &mut [PdfPage], gid_mapping: &HashMap<u16, u16>) {
        // TODO: This is almost the same code as `find_used_glyphs`. Somehow reduce redundancy
        let layers = pages
            .iter_mut()
            .map(|page| page.layers.iter_mut())
            .flatten();
        for layer in layers {
            let operations = layer
                .operations
                .iter_mut()
                .map(|op| (&mut op.operator, &mut op.operands));

            let mut font_active = false;

            for (operator, operands) in operations {
                match operator.as_str() {
                    "Tf" => {
                        let font_name = operands[0]
                            .as_name_str()
                            .expect("PDF Command 'Tf' not followed by Name operand");
                        font_active = font_name == &self.face_name;
                    }
                    "Tj" if font_active => {
                        let gid_stream = operands[0]
                            .as_str_mut()
                            .expect("PDF Command 'Tj' not followed by String operand");
                        for b in gid_stream.chunks_exact_mut(2) {
                            let gid = b[1] as u16 | ((b[0] as u16) << 8);

                            let new_gid = gid_mapping.get(&gid).unwrap_or(&0);
                            b[0] = (new_gid >> 8) as u8;
                            b[1] = (new_gid & 0xff) as u8;
                        }
                    }
                    "TJ" if font_active => {
                        let text_sections = operands[0]
                            .as_array_mut()
                            .expect("PDF Command 'TJ' not followed by Array operand")
                            .into_iter()
                            .filter_map(|obj| obj.as_str_mut().ok());

                        for gid_stream in text_sections {
                            for b in gid_stream.chunks_exact_mut(2) {
                                let gid = b[1] as u16 | ((b[0] as u16) << 8);

                                let new_gid = gid_mapping.get(&gid).unwrap_or(&0);
                                b[0] = (new_gid >> 8) as u8;
                                b[1] = (new_gid & 0xff) as u8;
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    /// Takes the font and adds it to the document and consumes the font.
    ///
    /// Returns None if the font doesn't need to be embedded
    pub(crate) fn into_with_document(
        self,
        doc: &mut lopdf::Document,
        _pages: &mut [PdfPage],
    ) -> Option<LoDictionary> {
        use lopdf::Object;
        use lopdf::Object::*;

        let face_name = self.face_name.clone();

        let font_data;
        let font_bytes;

        #[cfg(feature = "font_subsetting")]
        {
            if *self.allow_subsetting.borrow() {
                let mut used_glyphs = self.find_used_glyphs(_pages);
                // Don't include font if none of its glyphs are utilized
                if used_glyphs.is_empty() {
                    return None;
                }

                // TODO: This is ugly and will silently fall back to no subsetting on errors
                match crate::subsetting::subset(&self.font_bytes, &mut used_glyphs) {
                    Ok(font_subset) => {
                        match TtfFace::from_vec(font_subset.new_font_bytes.clone()) {
                            Ok(face) => {
                                let font: Box<dyn FontData> = Box::new(face);

                                self.replace_glyphs(_pages, &font_subset.gid_mapping);

                                font_data = font;
                                font_bytes = font_subset.new_font_bytes;
                            }
                            Err(_) => {
                                font_data = self.font_data;
                                font_bytes = self.font_bytes;
                            }
                        }
                    }
                    Err(_) => {
                        font_data = self.font_data;
                        font_bytes = self.font_bytes;
                    }
                }
            } else {
                font_data = self.font_data;
                font_bytes = self.font_bytes;
            }
        }

        #[cfg(not(feature = "font_subsetting"))]
        {
            font_data = self.font_data;
            font_bytes = self.font_bytes;
        }

        // Extract basic font information
        let face_metrics = font_data.font_metrics();

        let font_stream = LoStream::new(
            LoDictionary::from_iter(vec![("Length1", Integer(font_bytes.len() as i64))]),
            font_bytes,
        )
        .with_compression(false); /* important! font stream must not be compressed! */

        // Begin setting required font attributes
        let mut font_vec: Vec<(::std::string::String, Object)> = vec![
            ("Type".into(), Name("Font".into())),
            ("Subtype".into(), Name("Type0".into())),
            ("BaseFont".into(), Name(face_name.clone().into_bytes())),
            // Identity-H for horizontal writing, Identity-V for vertical writing
            ("Encoding".into(), Name("Identity-H".into())),
            // Missing DescendantFonts and ToUnicode
        ];

        let mut font_descriptor_vec: Vec<(::std::string::String, Object)> = vec![
            ("Type".into(), Name("FontDescriptor".into())),
            ("FontName".into(), Name(face_name.clone().into_bytes())),
            ("Ascent".into(), Integer(i64::from(face_metrics.ascent))),
            ("Descent".into(), Integer(i64::from(face_metrics.descent))),
            ("CapHeight".into(), Integer(i64::from(face_metrics.ascent))),
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
        let mut widths = Vec::<(u32, u32)>::new();

        // Glyph IDs - (Unicode IDs - character width, character height)
        let mut cmap = BTreeMap::<u32, (u32, u32, u32)>::new();
        cmap.insert(0, (0, 1000, 1000));

        for (glyph_id, c) in font_data.glyph_ids() {
            if let Some(glyph_metrics) = font_data.glyph_metrics(glyph_id) {
                if glyph_metrics.height > max_height {
                    max_height = glyph_metrics.height;
                }

                total_width += glyph_metrics.width;
                cmap.insert(
                    glyph_id as u32,
                    (c as u32, glyph_metrics.width, glyph_metrics.height),
                );
            }
        }

        // Maps the character index to a unicode value - add this to the "ToUnicode" dictionary!
        //
        // To explain this structure: Glyph IDs have to be in segments where the first byte of the
        // first and last element have to be the same. A range from 0x1000 - 0x10FF is valid
        // but a range from 0x1000 - 0x12FF is not (0x10 != 0x12)
        // Plus, the maximum number of Glyph-IDs in one range is 100
        //
        // Since the glyph IDs are sequential, all we really have to do is to enumerate the vector
        // and create buckets of 100 / rest to 256 if needed

        let mut cur_first_bit: u16 = 0_u16; // current first bit of the glyph id (0x10 or 0x12) for example

        let mut all_cmap_blocks = Vec::new();

        {
            let mut current_cmap_block = Vec::new();

            for (glyph_id, unicode_width_tuple) in &cmap {
                if (*glyph_id >> 8) as u16 != cur_first_bit || current_cmap_block.len() >= 100 {
                    // end the current (beginbfchar endbfchar) block
                    all_cmap_blocks.push(current_cmap_block.clone());
                    current_cmap_block = Vec::new();
                    cur_first_bit = (*glyph_id >> 8) as u16;
                }

                let (unicode, width, _) = *unicode_width_tuple;
                current_cmap_block.push((*glyph_id, unicode));
                widths.push((*glyph_id, width));
            }

            all_cmap_blocks.push(current_cmap_block);
        }

        let cid_to_unicode_map = generate_cid_to_unicode_map(face_name.clone(), all_cmap_blocks);

        let cid_to_unicode_map_stream =
            LoStream::new(LoDictionary::new(), cid_to_unicode_map.as_bytes().to_vec());
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
        let percentage_font_scaling = 1000.0 / (face_metrics.units_per_em as f32);

        for gid in 0..font_data.glyph_count() {
            if let Some(GlyphMetrics { width, .. }) = font_data.glyph_metrics(gid) {
                if gid == current_high_gid {
                    current_width_vec
                        .push(Integer((width as f32 * percentage_font_scaling) as i64));
                    current_high_gid += 1;
                } else {
                    widths_list.push(Integer(current_low_gid as i64));
                    widths_list.push(Array(current_width_vec.drain(..).collect()));

                    current_width_vec
                        .push(Integer((width as f32 * percentage_font_scaling) as i64));
                    current_low_gid = gid;
                    current_high_gid = gid + 1;
                }
            } else {
                continue;
            }
        }
        // push the last widths, because the loop is delayed by one iteration
        widths_list.push(Integer(current_low_gid as i64));
        widths_list.push(Array(current_width_vec.drain(..).collect()));

        let w = {
            if self.vertical_writing {
                ("W2", Array(widths_list))
            } else {
                ("W", Array(widths_list))
            }
        };

        // default width for characters
        let dw = {
            if self.vertical_writing {
                ("DW2", Integer(1000))
            } else {
                ("DW", Integer(1000))
            }
        };

        let mut desc_fonts = LoDictionary::from_iter(vec![
            ("Type", Name("Font".into())),
            ("Subtype", Name("CIDFontType2".into())),
            ("BaseFont", Name(face_name.into())),
            (
                "CIDSystemInfo",
                Dictionary(LoDictionary::from_iter(vec![
                    ("Registry", String("Adobe".into(), StringFormat::Literal)),
                    ("Ordering", String("Identity".into(), StringFormat::Literal)),
                    ("Supplement", Integer(0)),
                ])),
            ),
            w,
            dw,
        ]);

        let font_bbox = vec![
            Integer(0),
            Integer(max_height as i64),
            Integer(total_width as i64),
            Integer(max_height as i64),
        ];
        font_descriptor_vec.push(("FontFile2".into(), Reference(doc.add_object(font_stream))));

        // although the following entry is technically not needed, Adobe Reader needs it
        font_descriptor_vec.push(("FontBBox".into(), Array(font_bbox)));

        let font_descriptor_vec_id = doc.add_object(LoDictionary::from_iter(font_descriptor_vec));

        desc_fonts.set("FontDescriptor", Reference(font_descriptor_vec_id));

        font_vec.push((
            "DescendantFonts".into(),
            Array(vec![Dictionary(desc_fonts)]),
        ));
        font_vec.push(("ToUnicode".into(), Reference(cid_to_unicode_map_stream_id)));

        Some(LoDictionary::from_iter(font_vec))
    }
}

type GlyphId = u32;
type UnicodeCodePoint = u32;
type CmapBlock = Vec<(GlyphId, UnicodeCodePoint)>;

/// Generates a CMAP (character map) from valid cmap blocks
fn generate_cid_to_unicode_map(face_name: String, all_cmap_blocks: Vec<CmapBlock>) -> String {
    let mut cid_to_unicode_map =
        format!(include_str!("../assets/gid_to_unicode_beg.txt"), face_name);

    for cmap_block in all_cmap_blocks
        .into_iter()
        .filter(|block| !block.is_empty() || block.len() < 100)
    {
        cid_to_unicode_map.push_str(format!("{} beginbfchar\r\n", cmap_block.len()).as_str());
        for (glyph_id, unicode) in cmap_block {
            cid_to_unicode_map.push_str(format!("<{glyph_id:04x}> <{unicode:04x}>\n").as_str());
        }
        cid_to_unicode_map.push_str("endbfchar\r\n");
    }

    cid_to_unicode_map.push_str(include_str!("../assets/gid_to_unicode_end.txt"));
    cid_to_unicode_map
}

impl PartialEq for ExternalFont {
    /// Two fonts are equal if their names are equal, the contents aren't checked
    fn eq(&self, other: &ExternalFont) -> bool {
        self.face_name == other.face_name
    }
}

/// Indexed reference to a font that was added to the document
/// This is a "reference by postscript name"
#[derive(Debug, Hash, Eq, Ord, Clone, PartialEq, PartialOrd)]
pub struct IndirectFontRef {
    /// Name of the font (postscript name)
    pub(crate) name: String,
}

/// Direct reference (wrapper for `lopdf::Object::Reference`)
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
    pub fn new<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self { name: name.into() }
    }
}

/// Font list for tracking fonts within a single PDF document
#[derive(Default, Debug, Clone)]
pub struct FontList {
    fonts: BTreeMap<IndirectFontRef, DirectFontRef>,
}

impl FontList {
    /// Creates a new FontList
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a font to the FontList
    pub fn add_font(&mut self, font_ref: IndirectFontRef, font: DirectFontRef) -> IndirectFontRef {
        self.fonts.insert(font_ref.clone(), font);
        font_ref
    }

    /// Turns an indirect font reference into a direct one
    /// (Warning): clones the direct font reference
    #[inline]
    pub fn get_font(&self, font: &IndirectFontRef) -> Option<DirectFontRef> {
        self.fonts.get(font).cloned()
    }

    /// Returns the number of fonts currenly in use
    #[inline]
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    /// Returns if the font list is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Converts the fonts into a dictionary
    pub(crate) fn into_with_document(
        self,
        doc: &mut lopdf::Document,
        pages: &mut [PdfPage],
    ) -> lopdf::Dictionary {
        let mut font_dict = lopdf::Dictionary::new();

        for (indirect_ref, direct_font_ref) in self.fonts {
            let font_dict_collected = match direct_font_ref.data {
                Font::ExternalFont(font) => match font.into_with_document(doc, pages) {
                    Some(dict) => dict,
                    None => continue,
                },
                Font::BuiltinFont(font) => font.into(),
            };

            doc.objects.insert(
                direct_font_ref.inner_obj,
                lopdf::Object::Dictionary(font_dict_collected),
            );
            font_dict.set(
                indirect_ref.name,
                lopdf::Object::Reference(direct_font_ref.inner_obj),
            );
        }

        font_dict
    }
}

/// The unscaled base metrics for a font provided by a [`FontData`](trait.FontData.html)
/// implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontMetrics {
    /// The ascent of the font.
    pub ascent: i16,
    /// The descent of the font.
    pub descent: i16,
    /// The units per em square for this font.
    pub units_per_em: u16,
}

/// The metrics for a glyph provided by a [`FontData`](trait.FontData.html) implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphMetrics {
    /// The width of the glyph, typically the horizontal advance.
    pub width: u32,
    /// The height of the glyph, typically the difference between the ascent and the descent.
    pub height: u32,
}

/// Provides access to font metrics.
///
/// Per default, printpdf uses [`owned_ttf_parser`][] to extract the font data.  You can implement
/// this trait for other types if you want to use a different font backend.
///
/// [`owned_ttf_parser`]: https://docs.rs/owned_ttf_parser/latest/owned_ttf_parser/
pub trait FontData: FontDataClone + std::fmt::Debug {
    /// Returns the unscaled metrics for this font.
    fn font_metrics(&self) -> FontMetrics;

    /// Returns the glyph id for a Unicode character if it is present in this font.
    fn glyph_id(&self, c: char) -> Option<u16>;

    /// Returns a mapping from glyph IDs to Unicode characters for all supported characters.
    fn glyph_ids(&self) -> HashMap<u16, char>;

    /// Returns the number of glyphs in this font.
    fn glyph_count(&self) -> u16;

    /// Returns the glyph metrics for a glyph of this font, if available.
    fn glyph_metrics(&self, glyph_id: u16) -> Option<GlyphMetrics>;
}

/// Wrapper struct for `owned_ttf_parser::OwnedFace` that implements `Clone` and that makes sure
/// that the font is scalable.
#[derive(Clone, Debug)]
struct TtfFace {
    inner: std::sync::Arc<OwnedFace>,
    units_per_em: u16,
}

impl TtfFace {
    pub fn from_vec(v: Vec<u8>) -> Result<Self, Error> {
        let face = OwnedFace::from_vec(v, 0)?;
        let units_per_em = face.as_face_ref().units_per_em();
        Ok(Self {
            inner: std::sync::Arc::new(face),
            units_per_em,
        })
    }

    fn face(&self) -> &Face<'_> {
        self.inner.as_face_ref()
    }
}

impl FontData for TtfFace {
    fn font_metrics(&self) -> FontMetrics {
        FontMetrics {
            ascent: self.face().ascender(),
            descent: self.face().descender(),
            units_per_em: self.units_per_em,
        }
    }

    fn glyph_id(&self, c: char) -> Option<u16> {
        self.face().glyph_index(c).map(|id| id.0)
    }

    fn glyph_ids(&self) -> HashMap<u16, char> {
        let subtables = self
            .face()
            .tables()
            .cmap
            .map(|cmap| cmap.subtables.into_iter().filter(|v| v.is_unicode()));
        let Some(subtables) = subtables else {
            return HashMap::new();
        };
        let mut map = HashMap::with_capacity(self.face().number_of_glyphs().into());
        for subtable in subtables {
            subtable.codepoints(|c| {
                use std::convert::TryFrom as _;

                if let Ok(ch) = char::try_from(c) {
                    if let Some(idx) = subtable.glyph_index(c).filter(|idx| idx.0 > 0) {
                        map.entry(idx.0).or_insert(ch);
                    }
                }
            })
        }
        map
    }

    fn glyph_count(&self) -> u16 {
        self.face().number_of_glyphs()
    }

    fn glyph_metrics(&self, glyph_id: u16) -> Option<GlyphMetrics> {
        let glyph_id = owned_ttf_parser::GlyphId(glyph_id);
        if let Some(width) = self.face().glyph_hor_advance(glyph_id) {
            let width = width as u32;
            let height = self
                .face()
                .glyph_bounding_box(glyph_id)
                .map(|bbox| bbox.y_max - bbox.y_min - self.face().descender())
                .unwrap_or(1000) as u32;
            Some(GlyphMetrics { width, height })
        } else {
            None
        }
    }
}

/// Helper trait for cloning boxed [`FontData`](trait.FontData.html) implementors.
pub trait FontDataClone {
    /// Clones this font data and returns a box with the cloned data.
    fn clone_font_data(&self) -> Box<dyn FontData>;
}

impl<T: FontData + Clone + 'static> FontDataClone for T {
    fn clone_font_data(&self) -> Box<dyn FontData> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FontData> {
    fn clone(&self) -> Box<dyn FontData> {
        self.clone_font_data()
    }
}
