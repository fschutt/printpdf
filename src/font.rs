/// Builtin or external font
#[derive(Debug, Clone, PartialEq)]
pub enum Font {
    /// Represents one of the 14 built-in fonts (Arial, Helvetica, etc.)
    BuiltinFont(BuiltinFont),
    /// Represents a font loaded from an external file
    ExternalFont(Parse),
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

impl BuiltinFont {
    pub fn get_id(val: BuiltinFont) -> &'static str {
        use self::BuiltinFont::*;
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


use time::error::Parse;
use core::fmt;
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;
use std::vec::Vec;
use std::boxed::Box;
use allsorts::{
    binary::read::ReadScope,
    font_data::FontData,
    layout::{LayoutCache, GDEFTable, GPOS, GSUB},
    tables::{
        FontTableProvider, HheaTable, MaxpTable, HeadTable,
        loca::LocaTable,
        cmap::CmapSubtable,
        glyf::{GlyfTable, Glyph, GlyfRecord},
    },
    tables::cmap::owned::CmapSubtable as OwnedCmapSubtable,
};

#[derive(Clone)]
pub struct ParsedFont {
    pub font_metrics: FontMetrics,
    pub num_glyphs: u16,
    pub hhea_table: HheaTable,
    pub hmtx_data: Box<[u8]>,
    pub maxp_table: MaxpTable,
    pub gsub_cache: LayoutCache<GSUB>,
    pub gpos_cache: LayoutCache<GPOS>,
    pub opt_gdef_table: Option<Rc<GDEFTable>>,
    pub glyph_records_decoded: BTreeMap<u16, OwnedGlyph>,
    pub space_width: Option<usize>,
    pub cmap_subtable: OwnedCmapSubtable,
}

impl PartialEq for ParsedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font_metrics == other.font_metrics && 
        self.num_glyphs == other.num_glyphs && 
        self.hhea_table == other.hhea_table && 
        self.hmtx_data == other.hmtx_data && 
        self.maxp_table == other.maxp_table && 
        self.space_width == other.space_width && 
        self.cmap_subtable == other.cmap_subtable
    }
}

impl fmt::Debug for ParsedFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParsedFont")
        .field("font_metrics", &self.font_metrics)
        .field("num_glyphs", &self.num_glyphs)
        .field("hhea_table", &self.hhea_table)
        .field("hmtx_data", &self.hmtx_data)
        .field("maxp_table", &self.maxp_table)
        .field("glyph_records_decoded", &self.glyph_records_decoded)
        .field("space_width", &self.space_width)
        .field("cmap_subtable", &self.cmap_subtable)
        .finish()
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(C, u8)]
pub enum GlyphOutlineOperation {
    MoveTo(OutlineMoveTo),
    LineTo(OutlineLineTo),
    QuadraticCurveTo(OutlineQuadTo),
    CubicCurveTo(OutlineCubicTo),
    ClosePath,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct OutlineMoveTo {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct OutlineLineTo {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct OutlineQuadTo {
    pub ctrl_1_x: f32,
    pub ctrl_1_y: f32,
    pub end_x: f32,
    pub end_y: f32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct OutlineCubicTo {
    pub ctrl_1_x: f32,
    pub ctrl_1_y: f32,
    pub ctrl_2_x: f32,
    pub ctrl_2_y: f32,
    pub end_x: f32,
    pub end_y: f32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct GlyphOutline {
    pub operations: Vec<GlyphOutlineOperation>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
struct GlyphOutlineBuilder {
    operations: Vec<GlyphOutlineOperation>
}

impl Default for GlyphOutlineBuilder {
    fn default() -> Self {
        GlyphOutlineBuilder { operations: Vec::new() }
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::MoveTo(OutlineMoveTo { x, y })); }
    fn line_to(&mut self, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::LineTo(OutlineLineTo { x, y })); }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::QuadraticCurveTo(OutlineQuadTo { ctrl_1_x: x1, ctrl_1_y: y1, end_x: x, end_y: y })); }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::CubicCurveTo(OutlineCubicTo { ctrl_1_x: x1, ctrl_1_y: y1, ctrl_2_x: x2, ctrl_2_y: y2, end_x: x, end_y: y })); }
    fn close(&mut self) { self.operations.push(GlyphOutlineOperation::ClosePath); }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct OwnedGlyphBoundingBox {
    pub max_x: i16,
    pub max_y: i16,
    pub min_x: i16,
    pub min_y: i16,
}

#[derive(Debug, Clone)]
pub struct OwnedGlyph {
    pub bounding_box: OwnedGlyphBoundingBox,
    pub horz_advance: u16,
    pub outline: Option<GlyphOutline>,
}

impl OwnedGlyph {
    fn from_glyph_data<'a>(glyph: &Glyph<'a>, horz_advance: u16) -> Option<Self> {
        let bbox = glyph.bounding_box()?;
        Some(Self {
            bounding_box: OwnedGlyphBoundingBox {
                max_x: bbox.x_max,
                max_y: bbox.y_max,
                min_x: bbox.x_min,
                min_y: bbox.y_min,
            },
            horz_advance,
            outline: None,
        })
    }
}

impl ParsedFont {

    pub fn from_bytes(font_bytes: &[u8], font_index: usize, parse_glyph_outlines: bool) -> Option<Self> {

        use allsorts::tag;

        let scope = ReadScope::new(font_bytes);
        let font_file = scope.read::<FontData<'_>>().ok()?;
        let provider = font_file.table_provider(font_index).ok()?;

        let head_data = provider.table_data(tag::HEAD).ok()??.into_owned();
        let head_table = ReadScope::new(&head_data).read::<HeadTable>().ok()?;

        let maxp_data = provider.table_data(tag::MAXP).ok()??.into_owned();
        let maxp_table = ReadScope::new(&maxp_data).read::<MaxpTable>().ok()?;

        let loca_data = provider.table_data(tag::LOCA).ok()??.into_owned();
        let loca_table = ReadScope::new(&loca_data).read_dep::<LocaTable<'_>>((maxp_table.num_glyphs as usize, head_table.index_to_loc_format)).ok()?;

        let glyf_data = provider.table_data(tag::GLYF).ok()??.into_owned();
        let mut glyf_table = ReadScope::new(&glyf_data).read_dep::<GlyfTable<'_>>(&loca_table).ok()?;

        let hmtx_data = provider.table_data(tag::HMTX).ok()??.into_owned().into_boxed_slice();

        let hhea_data = provider.table_data(tag::HHEA).ok()??.into_owned();
        let hhea_table = ReadScope::new(&hhea_data).read::<HheaTable>().ok()?;

        let font_metrics = FontMetrics::from_bytes(font_bytes, font_index);

        // not parsing glyph outlines can save lots of memory
        let glyph_records_decoded = glyf_table.records_mut()
            .into_iter()
            .enumerate()
            .filter_map(|(glyph_index, glyph_record)| {
                if glyph_index > (u16::MAX as usize) {
                    return None;
                }
                glyph_record.parse().ok()?;
                let glyph_index = glyph_index as u16;
                let horz_advance = allsorts::glyph_info::advance(
                    &maxp_table,
                    &hhea_table,
                    &hmtx_data,
                    glyph_index
                ).unwrap_or_default();

                match glyph_record {
                    GlyfRecord::Present { .. } => None,
                    GlyfRecord::Parsed(g) => OwnedGlyph::from_glyph_data(g, horz_advance)
                        .map(|s| (glyph_index, s)),
                }
            }).collect::<Vec<_>>();

        let glyph_records_decoded = glyph_records_decoded.into_iter().collect();

        let mut font_data_impl = allsorts::font::Font::new(provider).ok()?;

        // required for font layout: gsub_cache, gpos_cache and gdef_table
        let gsub_cache = font_data_impl.gsub_cache().ok()??;
        let gpos_cache = font_data_impl.gpos_cache().ok()??;
        let opt_gdef_table = font_data_impl.gdef_table().ok().and_then(|o| o);
        let num_glyphs = font_data_impl.num_glyphs();

        let cmap_subtable = ReadScope::new(font_data_impl.cmap_subtable_data()).read::<CmapSubtable<'_>>().ok()?.to_owned()?;

        let mut font = ParsedFont {
            font_metrics,
            num_glyphs,
            hhea_table,
            hmtx_data,
            maxp_table,
            gsub_cache,
            gpos_cache,
            opt_gdef_table,
            cmap_subtable,
            glyph_records_decoded,
            space_width: None,
        };

        let space_width = font.get_space_width_internal();
        font.space_width = space_width;

        Some(font)
    }

    fn get_space_width_internal(&mut self) -> Option<usize> {
        let glyph_index = self.lookup_glyph_index(' ' as u32)?;
        allsorts::glyph_info::advance(&self.maxp_table, &self.hhea_table, &self.hmtx_data, glyph_index).ok().map(|s| s as usize)
    }

    /// Returns the width of the space " " character (unscaled units)
    #[inline]
    pub const fn get_space_width(&self) -> Option<usize> {
        self.space_width
    }

    /// Get the horizontal advance of a glyph index (unscaled units)
    pub fn get_horizontal_advance(&self, glyph_index: u16) -> u16 {
        self.glyph_records_decoded.get(&glyph_index).map(|gi| gi.horz_advance).unwrap_or_default()
    }

    // get the x and y size of a glyph (unscaled units)
    pub fn get_glyph_size(&self, glyph_index: u16) -> Option<(i32, i32)> {
        let g = self.glyph_records_decoded.get(&glyph_index)?;
        let glyph_width = g.bounding_box.max_x as i32 - g.bounding_box.min_x as i32; // width
        let glyph_height = g.bounding_box.max_y as i32 - g.bounding_box.min_y as i32; // height
        Some((glyph_width, glyph_height))
    }

    pub fn lookup_glyph_index(&self, c: u32) -> Option<u16> {
        match self.cmap_subtable.map_glyph(c) {
            Ok(Some(c)) => Some(c),
            _ => None,
        }
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct FontMetrics {
    // head table
    pub units_per_em: u16,
    pub font_flags: u16,
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,

    // hhea table
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub advance_width_max: u16,
    pub min_left_side_bearing: i16,
    pub min_right_side_bearing: i16,
    pub x_max_extent: i16,
    pub caret_slope_rise: i16,
    pub caret_slope_run: i16,
    pub caret_offset: i16,
    pub num_h_metrics: u16,

    // os/2 table
    pub x_avg_char_width: i16,
    pub us_weight_class: u16,
    pub us_width_class: u16,
    pub fs_type: u16,
    pub y_subscript_x_size: i16,
    pub y_subscript_y_size: i16,
    pub y_subscript_x_offset: i16,
    pub y_subscript_y_offset: i16,
    pub y_superscript_x_size: i16,
    pub y_superscript_y_size: i16,
    pub y_superscript_x_offset: i16,
    pub y_superscript_y_offset: i16,
    pub y_strikeout_size: i16,
    pub y_strikeout_position: i16,
    pub s_family_class: i16,
    pub panose: [u8; 10],
    pub ul_unicode_range1: u32,
    pub ul_unicode_range2: u32,
    pub ul_unicode_range3: u32,
    pub ul_unicode_range4: u32,
    pub ach_vend_id: u32,
    pub fs_selection: u16,
    pub us_first_char_index: u16,
    pub us_last_char_index: u16,

    // os/2 version 0 table
    pub s_typo_ascender: Option<i16>,
    pub s_typo_descender: Option<i16>,
    pub s_typo_line_gap: Option<i16>,
    pub us_win_ascent: Option<u16>,
    pub us_win_descent: Option<u16>,

    // os/2 version 1 table
    pub ul_code_page_range1: Option<u32>,
    pub ul_code_page_range2: Option<u32>,

    // os/2 version 2 table
    pub sx_height: Option<i16>,
    pub s_cap_height: Option<i16>,
    pub us_default_char: Option<u16>,
    pub us_break_char: Option<u16>,
    pub us_max_context: Option<u16>,

    // os/2 version 3 table
    pub us_lower_optical_point_size: Option<u16>,
    pub us_upper_optical_point_size: Option<u16>,
}

impl Default for FontMetrics {
    fn default() -> Self {
        FontMetrics::zero()
    }
}

impl FontMetrics {

    /// Only for testing, zero-sized font, will always return 0 for every metric (`units_per_em = 1000`)
    pub const fn zero() -> Self {
        FontMetrics {
            units_per_em: 1000,
            font_flags: 0,
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
            ascender: 0,
            descender: 0,
            line_gap: 0,
            advance_width_max: 0,
            min_left_side_bearing: 0,
            min_right_side_bearing: 0,
            x_max_extent: 0,
            caret_slope_rise: 0,
            caret_slope_run: 0,
            caret_offset: 0,
            num_h_metrics: 0,
            x_avg_char_width: 0,
            us_weight_class: 0,
            us_width_class: 0,
            fs_type: 0,
            y_subscript_x_size: 0,
            y_subscript_y_size: 0,
            y_subscript_x_offset: 0,
            y_subscript_y_offset: 0,
            y_superscript_x_size: 0,
            y_superscript_y_size: 0,
            y_superscript_x_offset: 0,
            y_superscript_y_offset: 0,
            y_strikeout_size: 0,
            y_strikeout_position: 0,
            s_family_class: 0,
            panose: [0; 10],
            ul_unicode_range1: 0,
            ul_unicode_range2: 0,
            ul_unicode_range3: 0,
            ul_unicode_range4: 0,
            ach_vend_id: 0,
            fs_selection: 0,
            us_first_char_index: 0,
            us_last_char_index: 0,
            s_typo_ascender: None,
            s_typo_descender: None,
            s_typo_line_gap: None,
            us_win_ascent: None,
            us_win_descent: None,
            ul_code_page_range1: None,
            ul_code_page_range2: None,
            sx_height: None,
            s_cap_height: None,
            us_default_char: None,
            us_break_char: None,
            us_max_context: None,
            us_lower_optical_point_size: None,
            us_upper_optical_point_size: None,
        }
    }

    /// Parses `FontMetrics` from a font
    pub fn from_bytes(font_bytes: &[u8], font_index: usize) -> Self {

        #[derive(Default)]
        struct Os2Info {
            x_avg_char_width: i16,
            us_weight_class: u16,
            us_width_class: u16,
            fs_type: u16,
            y_subscript_x_size: i16,
            y_subscript_y_size: i16,
            y_subscript_x_offset: i16,
            y_subscript_y_offset: i16,
            y_superscript_x_size: i16,
            y_superscript_y_size: i16,
            y_superscript_x_offset: i16,
            y_superscript_y_offset: i16,
            y_strikeout_size: i16,
            y_strikeout_position: i16,
            s_family_class: i16,
            panose: [u8; 10],
            ul_unicode_range1: u32,
            ul_unicode_range2: u32,
            ul_unicode_range3: u32,
            ul_unicode_range4: u32,
            ach_vend_id: u32,
            fs_selection: u16,
            us_first_char_index: u16,
            us_last_char_index: u16,
            s_typo_ascender: Option<i16>,
            s_typo_descender: Option<i16>,
            s_typo_line_gap: Option<i16>,
            us_win_ascent: Option<u16>,
            us_win_descent: Option<u16>,
            ul_code_page_range1: Option<u32>,
            ul_code_page_range2: Option<u32>,
            sx_height: Option<i16>,
            s_cap_height: Option<i16>,
            us_default_char: Option<u16>,
            us_break_char: Option<u16>,
            us_max_context: Option<u16>,
            us_lower_optical_point_size: Option<u16>,
            us_upper_optical_point_size: Option<u16>,
        }

        let scope = ReadScope::new(font_bytes);
        let font_file = match scope.read::<FontData<'_>>() {
            Ok(o) => o,
            Err(_) => return FontMetrics::default(),
        };
        let provider = match font_file.table_provider(font_index) {
            Ok(o) => o,
            Err(_) => return FontMetrics::default(),
        };
        let font = match allsorts::font::Font::new(provider).ok() {
            Some(s) => s,
            _ => return FontMetrics::default(),
        };

        // read the HHEA table to get the metrics for horizontal layout
        let hhea_table = &font.hhea_table;
        let head_table = match font.head_table().ok() {
            Some(Some(s)) => s,
            _ => return FontMetrics::default(),
        };

        let os2_table = match font.os2_table().ok() {
            Some(Some(s)) => {
                Os2Info {
                    x_avg_char_width: s.x_avg_char_width,
                    us_weight_class: s.us_weight_class,
                    us_width_class: s.us_width_class,
                    fs_type: s.fs_type,
                    y_subscript_x_size: s.y_subscript_x_size,
                    y_subscript_y_size: s.y_subscript_y_size,
                    y_subscript_x_offset: s.y_subscript_x_offset,
                    y_subscript_y_offset: s.y_subscript_y_offset,
                    y_superscript_x_size: s.y_superscript_x_size,
                    y_superscript_y_size: s.y_superscript_y_size,
                    y_superscript_x_offset: s.y_superscript_x_offset,
                    y_superscript_y_offset: s.y_superscript_y_offset,
                    y_strikeout_size: s.y_strikeout_size,
                    y_strikeout_position: s.y_strikeout_position,
                    s_family_class: s.s_family_class,
                    panose: s.panose,
                    ul_unicode_range1: s.ul_unicode_range1,
                    ul_unicode_range2: s.ul_unicode_range2,
                    ul_unicode_range3: s.ul_unicode_range3,
                    ul_unicode_range4: s.ul_unicode_range4,
                    ach_vend_id: s.ach_vend_id,
                    fs_selection: s.fs_selection.bits(),
                    us_first_char_index: s.us_first_char_index,
                    us_last_char_index: s.us_last_char_index,

                    s_typo_ascender: s.version0.as_ref().map(|q| q.s_typo_ascender),
                    s_typo_descender: s.version0.as_ref().map(|q| q.s_typo_descender),
                    s_typo_line_gap: s.version0.as_ref().map(|q| q.s_typo_line_gap),
                    us_win_ascent: s.version0.as_ref().map(|q| q.us_win_ascent),
                    us_win_descent: s.version0.as_ref().map(|q| q.us_win_descent),

                    ul_code_page_range1: s.version1.as_ref().map(|q| q.ul_code_page_range1),
                    ul_code_page_range2: s.version1.as_ref().map(|q| q.ul_code_page_range2),

                    sx_height: s.version2to4.as_ref().map(|q| q.sx_height),
                    s_cap_height: s.version2to4.as_ref().map(|q| q.s_cap_height),
                    us_default_char: s.version2to4.as_ref().map(|q| q.us_default_char),
                    us_break_char: s.version2to4.as_ref().map(|q| q.us_break_char),
                    us_max_context: s.version2to4.as_ref().map(|q| q.us_max_context),

                    us_lower_optical_point_size: s.version5.as_ref().map(|q| q.us_lower_optical_point_size),
                    us_upper_optical_point_size: s.version5.as_ref().map(|q| q.us_upper_optical_point_size),
                }
            },
            _ => Os2Info::default(),
        };

        FontMetrics {

            // head table
            units_per_em: if head_table.units_per_em == 0 {
                1000_u16
            } else {
                head_table.units_per_em
            },
            font_flags: head_table.flags,
            x_min: head_table.x_min,
            y_min: head_table.y_min,
            x_max: head_table.x_max,
            y_max: head_table.y_max,

            // hhea table
            ascender: hhea_table.ascender,
            descender: hhea_table.descender,
            line_gap: hhea_table.line_gap,
            advance_width_max: hhea_table.advance_width_max,
            min_left_side_bearing: hhea_table.min_left_side_bearing,
            min_right_side_bearing: hhea_table.min_right_side_bearing,
            x_max_extent: hhea_table.x_max_extent,
            caret_slope_rise: hhea_table.caret_slope_rise,
            caret_slope_run: hhea_table.caret_slope_run,
            caret_offset: hhea_table.caret_offset,
            num_h_metrics: hhea_table.num_h_metrics,

            // os/2 table

            x_avg_char_width: os2_table.x_avg_char_width,
            us_weight_class: os2_table.us_weight_class,
            us_width_class: os2_table.us_width_class,
            fs_type: os2_table.fs_type,
            y_subscript_x_size: os2_table.y_subscript_x_size,
            y_subscript_y_size: os2_table.y_subscript_y_size,
            y_subscript_x_offset: os2_table.y_subscript_x_offset,
            y_subscript_y_offset: os2_table.y_subscript_y_offset,
            y_superscript_x_size: os2_table.y_superscript_x_size,
            y_superscript_y_size: os2_table.y_superscript_y_size,
            y_superscript_x_offset: os2_table.y_superscript_x_offset,
            y_superscript_y_offset: os2_table.y_superscript_y_offset,
            y_strikeout_size: os2_table.y_strikeout_size,
            y_strikeout_position: os2_table.y_strikeout_position,
            s_family_class: os2_table.s_family_class,
            panose: os2_table.panose,
            ul_unicode_range1: os2_table.ul_unicode_range1,
            ul_unicode_range2: os2_table.ul_unicode_range2,
            ul_unicode_range3: os2_table.ul_unicode_range3,
            ul_unicode_range4: os2_table.ul_unicode_range4,
            ach_vend_id: os2_table.ach_vend_id,
            fs_selection: os2_table.fs_selection,
            us_first_char_index: os2_table.us_first_char_index,
            us_last_char_index: os2_table.us_last_char_index,
            s_typo_ascender: os2_table.s_typo_ascender.into(),
            s_typo_descender: os2_table.s_typo_descender.into(),
            s_typo_line_gap: os2_table.s_typo_line_gap.into(),
            us_win_ascent: os2_table.us_win_ascent.into(),
            us_win_descent: os2_table.us_win_descent.into(),
            ul_code_page_range1: os2_table.ul_code_page_range1.into(),
            ul_code_page_range2: os2_table.ul_code_page_range2.into(),
            sx_height: os2_table.sx_height.into(),
            s_cap_height: os2_table.s_cap_height.into(),
            us_default_char: os2_table.us_default_char.into(),
            us_break_char: os2_table.us_break_char.into(),
            us_max_context: os2_table.us_max_context.into(),
            us_lower_optical_point_size: os2_table.us_lower_optical_point_size.into(),
            us_upper_optical_point_size: os2_table.us_upper_optical_point_size.into(),
        }
    }

    /// If set, use `OS/2.sTypoAscender - OS/2.sTypoDescender + OS/2.sTypoLineGap` to calculate the height
    ///
    /// See [`USE_TYPO_METRICS`](https://docs.microsoft.com/en-us/typography/opentype/spec/os2#fss)
    pub fn use_typo_metrics(&self) -> bool {
        self.fs_selection & (1 << 7) != 0
    }

    pub fn get_ascender_unscaled(&self) -> i16 {
        let use_typo = if !self.use_typo_metrics() {
            None
        } else {
            self.s_typo_ascender.into()
        };
        match use_typo {
            Some(s) => s,
            None => self.ascender,
        }
    }

    /// NOTE: descender is NEGATIVE
    pub fn get_descender_unscaled(&self) -> i16 {
        let use_typo = if !self.use_typo_metrics() {
            None
        } else {
            self.s_typo_descender.into()
        };
        match use_typo {
            Some(s) => s,
            None => self.descender,
        }
    }

    pub fn get_line_gap_unscaled(&self) -> i16 {
        let use_typo = if !self.use_typo_metrics() {
            None
        } else {
            self.s_typo_line_gap.into()
        };
        match use_typo {
            Some(s) => s,
            None => self.line_gap,
        }
    }

    pub fn get_ascender(&self, target_font_size: f32) -> f32 {
        self.get_ascender_unscaled() as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_descender(&self, target_font_size: f32) -> f32 {
        self.get_descender_unscaled() as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_line_gap(&self, target_font_size: f32) -> f32 {
        self.get_line_gap_unscaled() as f32 / self.units_per_em as f32 * target_font_size
    }

    pub fn get_x_min(&self, target_font_size: f32) -> f32 {
        self.x_min as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_min(&self, target_font_size: f32) -> f32 {
        self.y_min as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_x_max(&self, target_font_size: f32) -> f32 {
        self.x_max as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_max(&self, target_font_size: f32) -> f32 {
        self.y_max as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_advance_width_max(&self, target_font_size: f32) -> f32 {
        self.advance_width_max as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_min_left_side_bearing(&self, target_font_size: f32) -> f32 {
        self.min_left_side_bearing as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_min_right_side_bearing(&self, target_font_size: f32) -> f32 {
        self.min_right_side_bearing as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_x_max_extent(&self, target_font_size: f32) -> f32 {
        self.x_max_extent as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_x_avg_char_width(&self, target_font_size: f32) -> f32 {
        self.x_avg_char_width as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_subscript_x_size(&self, target_font_size: f32) -> f32 {
        self.y_subscript_x_size as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_subscript_y_size(&self, target_font_size: f32) -> f32 {
        self.y_subscript_y_size as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_subscript_x_offset(&self, target_font_size: f32) -> f32 {
        self.y_subscript_x_offset as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_subscript_y_offset(&self, target_font_size: f32) -> f32 {
        self.y_subscript_y_offset as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_superscript_x_size(&self, target_font_size: f32) -> f32 {
        self.y_superscript_x_size as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_superscript_y_size(&self, target_font_size: f32) -> f32 {
        self.y_superscript_y_size as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_superscript_x_offset(&self, target_font_size: f32) -> f32 {
        self.y_superscript_x_offset as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_superscript_y_offset(&self, target_font_size: f32) -> f32 {
        self.y_superscript_y_offset as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_strikeout_size(&self, target_font_size: f32) -> f32 {
        self.y_strikeout_size as f32 / self.units_per_em as f32 * target_font_size
    }
    pub fn get_y_strikeout_position(&self, target_font_size: f32) -> f32 {
        self.y_strikeout_position as f32 / self.units_per_em as f32 * target_font_size
    }

    pub fn get_s_typo_ascender(&self, target_font_size: f32) -> Option<f32> {
        self.s_typo_ascender
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_s_typo_descender(&self, target_font_size: f32) -> Option<f32> {
        self.s_typo_descender
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_s_typo_line_gap(&self, target_font_size: f32) -> Option<f32> {
        self.s_typo_line_gap
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_us_win_ascent(&self, target_font_size: f32) -> Option<f32> {
        self.us_win_ascent
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_us_win_descent(&self, target_font_size: f32) -> Option<f32> {
        self.us_win_descent
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_sx_height(&self, target_font_size: f32) -> Option<f32> {
        self.sx_height
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
    pub fn get_s_cap_height(&self, target_font_size: f32) -> Option<f32> {
        self.s_cap_height
            .map(|s| s as f32 / self.units_per_em as f32 * target_font_size)
    }
}
