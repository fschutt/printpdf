use core::fmt;
use std::{
    collections::{BTreeSet, btree_map::BTreeMap},
    rc::Rc,
    vec::Vec,
};

use allsorts::{
    binary::read::{ReadArray, ReadScope},
    font_data::FontData,
    layout::{GDEFTable, GPOS, GSUB, LayoutCache},
    tables::{
        FontTableProvider, HeadTable, HheaTable, IndexToLocFormat, MaxpTable,
        cmap::{CmapSubtable, owned::CmapSubtable as OwnedCmapSubtable},
        glyf::{GlyfRecord, GlyfTable, Glyph},
        loca::{LocaOffsets, LocaTable},
    },
};
use base64::Engine;
use lopdf::Object::{Array, Integer};
use serde_derive::{Deserialize, Serialize};
use time::error::Parse;

use crate::{FontId, Op, PdfPage, TextItem};

/// Builtin or external font
#[derive(Debug, Clone, PartialEq)]
pub enum Font {
    /// Represents one of the 14 built-in fonts (Arial, Helvetica, etc.)
    BuiltinFont(BuiltinFont),
    /// Represents a font loaded from an external file
    ExternalFont(Parse),
}

/// Standard built-in PDF fonts
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

include!("../defaultfonts/mapping.rs");

impl BuiltinFont {
    /// Returns a CSS font-family string appropriate for the built-in PDF font.
    /// For example, TimesRoman maps to "Times New Roman, Times, serif".
    pub fn get_svg_font_family(&self) -> &'static str {
        match self {
            BuiltinFont::TimesRoman => "Times New Roman, Times, serif",
            BuiltinFont::TimesBold => "Times New Roman, Times, serif",
            BuiltinFont::TimesItalic => "Times New Roman, Times, serif",
            BuiltinFont::TimesBoldItalic => "Times New Roman, Times, serif",
            BuiltinFont::Helvetica => "Helvetica, Arial, sans-serif",
            BuiltinFont::HelveticaBold => "Helvetica, Arial, sans-serif",
            BuiltinFont::HelveticaOblique => "Helvetica, Arial, sans-serif",
            BuiltinFont::HelveticaBoldOblique => "Helvetica, Arial, sans-serif",
            BuiltinFont::Courier => "Courier New, Courier, monospace",
            BuiltinFont::CourierOblique => "Courier New, Courier, monospace",
            BuiltinFont::CourierBold => "Courier New, Courier, monospace",
            BuiltinFont::CourierBoldOblique => "Courier New, Courier, monospace",
            BuiltinFont::Symbol => "Symbol",
            BuiltinFont::ZapfDingbats => "Zapf Dingbats",
        }
    }

    /// Returns the CSS font-weight for the built-in font.
    pub fn get_font_weight(&self) -> &'static str {
        match self {
            BuiltinFont::TimesRoman
            | BuiltinFont::TimesItalic
            | BuiltinFont::Helvetica
            | BuiltinFont::HelveticaOblique
            | BuiltinFont::Courier
            | BuiltinFont::CourierOblique
            | BuiltinFont::Symbol
            | BuiltinFont::ZapfDingbats => "normal",
            BuiltinFont::TimesBold
            | BuiltinFont::TimesBoldItalic
            | BuiltinFont::HelveticaBold
            | BuiltinFont::HelveticaBoldOblique
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierBoldOblique => "bold",
        }
    }

    /// Returns the CSS font-style for the built-in font.
    pub fn get_font_style(&self) -> &'static str {
        match self {
            BuiltinFont::TimesItalic
            | BuiltinFont::TimesBoldItalic
            | BuiltinFont::HelveticaOblique
            | BuiltinFont::HelveticaBoldOblique
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => "italic",
            _ => "normal",
        }
    }

    /// Returns the already-subsetted font (Win-1252 codepage)
    pub fn get_subset_font(&self) -> SubsetFont {
        use self::BuiltinFont::*;

        SubsetFont {
            bytes: match self {
                TimesRoman => {
                    crate::uncompress(include_bytes!("../defaultfonts/Times-Roman.subset.ttf"))
                }
                TimesBold => {
                    crate::uncompress(include_bytes!("../defaultfonts/Times-Bold.subset.ttf"))
                }
                TimesItalic => {
                    crate::uncompress(include_bytes!("../defaultfonts/Times-Italic.subset.ttf"))
                }
                TimesBoldItalic => crate::uncompress(include_bytes!(
                    "../defaultfonts/Times-BoldItalic.subset.ttf"
                )),
                Helvetica => {
                    crate::uncompress(include_bytes!("../defaultfonts/Helvetica.subset.ttf"))
                }
                HelveticaBold => {
                    crate::uncompress(include_bytes!("../defaultfonts/Helvetica-Bold.subset.ttf"))
                }
                HelveticaOblique => crate::uncompress(include_bytes!(
                    "../defaultfonts/Helvetica-Oblique.subset.ttf"
                )),
                HelveticaBoldOblique => crate::uncompress(include_bytes!(
                    "../defaultfonts/Helvetica-BoldOblique.subset.ttf"
                )),
                Courier => crate::uncompress(include_bytes!("../defaultfonts/Courier.subset.ttf")),
                CourierOblique => {
                    crate::uncompress(include_bytes!("../defaultfonts/Courier-Oblique.subset.ttf"))
                }
                CourierBold => {
                    crate::uncompress(include_bytes!("../defaultfonts/Courier-Bold.subset.ttf"))
                }
                CourierBoldOblique => crate::uncompress(include_bytes!(
                    "../defaultfonts/Courier-BoldOblique.subset.ttf"
                )),
                Symbol => crate::uncompress(include_bytes!("../defaultfonts/Symbol.subset.ttf")),
                ZapfDingbats => {
                    crate::uncompress(include_bytes!("../defaultfonts/ZapfDingbats.subset.ttf"))
                }
            },
            glyph_mapping: FONTS
                .iter()
                .filter_map(|(font_id, old_gid, new_gid, char)| {
                    if *font_id == self.get_num() {
                        Some((*old_gid, (*new_gid, *char)))
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    pub fn get_pdf_id(&self) -> &'static str {
        use self::BuiltinFont::*;
        match self {
            TimesRoman => "F1",
            TimesBold => "F2",
            TimesItalic => "F3",
            TimesBoldItalic => "F4",
            Helvetica => "F5",
            HelveticaBold => "F6",
            HelveticaOblique => "F7",
            HelveticaBoldOblique => "F8",
            Courier => "F9",
            CourierOblique => "F10",
            CourierBold => "F11",
            CourierBoldOblique => "F12",
            Symbol => "F13",
            ZapfDingbats => "F14",
        }
    }

    pub fn get_num(&self) -> usize {
        use self::BuiltinFont::*;
        match self {
            TimesRoman => 0,
            TimesBold => 1,
            TimesItalic => 2,
            TimesBoldItalic => 3,
            Helvetica => 4,
            HelveticaBold => 5,
            HelveticaOblique => 6,
            HelveticaBoldOblique => 7,
            Courier => 8,
            CourierOblique => 9,
            CourierBold => 10,
            CourierBoldOblique => 11,
            Symbol => 12,
            ZapfDingbats => 13,
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        use self::BuiltinFont::*;
        match s {
            "Times-Roman" => Some(TimesRoman),
            "Times-Bold" => Some(TimesBold),
            "Times-Italic" => Some(TimesItalic),
            "Times-BoldItalic" => Some(TimesBoldItalic),
            "Helvetica" => Some(Helvetica),
            "Helvetica-Bold" => Some(HelveticaBold),
            "Helvetica-Oblique" => Some(HelveticaOblique),
            "Helvetica-BoldOblique" => Some(HelveticaBoldOblique),
            "Courier" => Some(Courier),
            "Courier-Oblique" => Some(CourierOblique),
            "Courier-Bold" => Some(CourierBold),
            "Courier-BoldOblique" => Some(CourierBoldOblique),
            "Symbol" => Some(Symbol),
            "ZapfDingbats" => Some(ZapfDingbats),
            _ => None,
        }
    }

    pub fn get_id(&self) -> &'static str {
        use self::BuiltinFont::*;
        match self {
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

#[derive(Clone, Default)]
pub struct ParsedFont {
    pub font_metrics: FontMetrics,
    pub num_glyphs: u16,
    pub hhea_table: Option<HheaTable>,
    pub hmtx_data: Vec<u8>,
    pub vmtx_data: Vec<u8>,
    pub maxp_table: Option<MaxpTable>,
    pub gsub_cache: Option<LayoutCache<GSUB>>,
    pub gpos_cache: Option<LayoutCache<GPOS>>,
    pub opt_gdef_table: Option<Rc<GDEFTable>>,
    pub glyph_records_decoded: BTreeMap<u16, OwnedGlyph>,
    pub space_width: Option<usize>,
    pub cmap_subtable: Option<OwnedCmapSubtable>,
    pub original_bytes: Vec<u8>,
    pub original_index: usize,
}

const FONT_B64_START: &str = "data:font/ttf;base64,";

impl serde::Serialize for ParsedFont {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = format!(
            "{FONT_B64_START}{}",
            base64::prelude::BASE64_STANDARD.encode(&self.original_bytes)
        );
        s.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ParsedFont {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<ParsedFont, D::Error> {
        let s = String::deserialize(deserializer)?;
        let b64 = if s.starts_with(FONT_B64_START) {
            let b = &s[FONT_B64_START.len()..];
            base64::prelude::BASE64_STANDARD.decode(&b).ok()
        } else {
            None
        };
        Ok(ParsedFont::from_bytes(&b64.unwrap_or_default(), 0).unwrap_or_default())
    }
}

impl PartialEq for ParsedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font_metrics == other.font_metrics
            && self.num_glyphs == other.num_glyphs
            && self.hhea_table == other.hhea_table
            && self.hmtx_data == other.hmtx_data
            && self.maxp_table == other.maxp_table
            && self.space_width == other.space_width
            && self.cmap_subtable == other.cmap_subtable
            && self.original_bytes.len() == other.original_bytes.len()
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

#[derive(Debug, Clone)]
pub struct SubsetFont {
    pub bytes: Vec<u8>,
    pub glyph_mapping: BTreeMap<u16, (u16, char)>,
}

impl SubsetFont {
    /// Return the changed text so that when rendering with the subset font (instead of the
    /// original) the renderer will end up at the same glyph IDs as if we used the original text
    /// on the original font
    pub fn subset_text(&self, text: &str) -> String {
        text.chars()
            .filter_map(|c| {
                self.glyph_mapping.values().find_map(|(ngid, ch)| {
                    if *ch == c {
                        char::from_u32(*ngid as u32)
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

impl ParsedFont {
    /// Returns the glyph IDs used in the PDF file
    pub(crate) fn get_used_glyph_ids(
        &self,
        font_id: &FontId,
        pages: &[PdfPage],
    ) -> BTreeMap<u16, char> {
        enum CharsOrCodepoint {
            Chars(String),
            Cp(Vec<(u16, char)>),
        }

        let chars_or_codepoints = pages
            .iter()
            .flat_map(|p| {
                p.ops.iter().filter_map(|s| match s {
                    Op::WriteText { font, items, .. } => {
                        if font_id == font {
                            Some(CharsOrCodepoint::Chars(
                                items
                                    .iter()
                                    .filter_map(|s| match s {
                                        TextItem::Text(t) => Some(t.clone()),
                                        TextItem::Offset(_) => None,
                                    })
                                    .collect(),
                            ))
                        } else {
                            None
                        }
                    }
                    Op::WriteCodepoints { font, cp, .. } => {
                        if font_id == font {
                            Some(CharsOrCodepoint::Cp(cp.clone()))
                        } else {
                            None
                        }
                    }
                    Op::WriteCodepointsWithKerning { font, cpk, .. } => {
                        if font_id == font {
                            Some(CharsOrCodepoint::Cp(
                                cpk.iter().map(|s| (s.1, s.2)).collect(),
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
            })
            .collect::<Vec<_>>();

        if chars_or_codepoints.is_empty() {
            return BTreeMap::new(); // font added, but never used
        }

        let chars_to_resolve = chars_or_codepoints
            .iter()
            .flat_map(|s| match s {
                CharsOrCodepoint::Chars(c) => c.chars().collect(),
                _ => Vec::new(),
            })
            .collect::<BTreeSet<_>>();

        /*
        chars_to_resolve.extend(DEFAULT_ALPHABET.iter().flat_map(|line| {
            line.chars().skip_while(|c| char::is_whitespace(*c))
        }));
        */

        let mut map = chars_to_resolve
            .iter()
            .filter_map(|c| self.lookup_glyph_index(*c as u32).map(|f| (f, *c)))
            .collect::<BTreeMap<_, _>>();

        map.extend(chars_or_codepoints.iter().flat_map(|s| match s {
            CharsOrCodepoint::Cp(c) => c.clone(),
            _ => Vec::new(),
        }));

        if let Some(sp) = self.lookup_glyph_index(' ' as u32) {
            map.insert(sp, ' ');
        }

        map
    }

    pub fn subset_simple(&self, chars: &BTreeSet<char>) -> Result<SubsetFont, String> {
        let scope = ReadScope::new(&self.original_bytes);
        let font_file = scope.read::<FontData<'_>>().map_err(|e| e.to_string())?;
        let provider = font_file
            .table_provider(self.original_index)
            .map_err(|e| e.to_string())?;

        let p = chars
            .iter()
            .filter_map(|s| self.lookup_glyph_index(*s as u32).map(|q| (q, *s)))
            .collect::<BTreeSet<_>>();

        let glyph_mapping = p
            .iter()
            .enumerate()
            .map(|(new_glyph_id, (original_glyph_id, ch))| {
                (*original_glyph_id, (new_glyph_id as u16, *ch))
            })
            .collect::<BTreeMap<_, _>>();

        let mut gids = p.iter().map(|s| s.0).collect::<Vec<_>>();
        gids.sort();
        gids.dedup();

        let bytes = allsorts::subset::subset(&provider, &gids).map_err(|e| e.to_string())?;

        Ok(SubsetFont {
            bytes,
            glyph_mapping,
        })
    }

    /// Generates a new font file from the used glyph IDs
    pub fn subset(&self, glyph_ids: &[(u16, char)]) -> Result<SubsetFont, String> {
        let glyph_mapping = glyph_ids
            .iter()
            .enumerate()
            .map(|(new_glyph_id, (original_glyph_id, ch))| {
                (*original_glyph_id, (new_glyph_id as u16, *ch))
            })
            .collect();

        let scope = ReadScope::new(&self.original_bytes);

        let font_file = scope.read::<FontData<'_>>().map_err(|e| e.to_string())?;

        let provider = font_file
            .table_provider(self.original_index)
            .map_err(|e| e.to_string())?;

        let font = allsorts::subset::subset(
            &provider,
            &glyph_ids.iter().map(|s| s.0).collect::<Vec<_>>(),
        )
        .map_err(|e| e.to_string())?;

        Ok(SubsetFont {
            bytes: font,
            glyph_mapping,
        })
    }

    pub(crate) fn generate_cid_to_unicode_map(
        &self,
        font_id: &FontId,
        glyph_ids: &BTreeMap<u16, char>,
    ) -> String {
        // current first bit of the glyph id (0x10 or 0x12) for example
        let mut cur_first_bit: u16 = 0_u16;
        let mut all_cmap_blocks = Vec::new();
        let mut current_cmap_block = Vec::new();

        for (glyph_id, unicode) in glyph_ids.iter() {
            // end the current (beginbfchar endbfchar) block if necessary
            if (*glyph_id >> 8) != cur_first_bit || current_cmap_block.len() >= 100 {
                all_cmap_blocks.push(current_cmap_block.clone());
                current_cmap_block = Vec::new();
                cur_first_bit = *glyph_id >> 8;
            }

            current_cmap_block.push((*glyph_id, *unicode as u32));
        }

        all_cmap_blocks.push(current_cmap_block);

        generate_cid_to_unicode_map(font_id.0.clone(), all_cmap_blocks)
    }

    pub(crate) fn get_normalized_widths(
        &self,
        glyph_ids: &BTreeMap<u16, char>,
    ) -> Vec<lopdf::Object> {
        let mut widths_list = Vec::new();
        let mut current_low_gid = 0;
        let mut current_high_gid = 0;
        let mut current_width_vec = Vec::new();

        // scale the font width so that it sort-of fits into an 1000 unit square
        let percentage_font_scaling = 1000.0 / (self.font_metrics.units_per_em as f32);

        for gid in glyph_ids.keys() {
            let (width, _) = match self.get_glyph_size(*gid) {
                Some(s) => s,
                None => match self.get_space_width() {
                    Some(w) => (w as i32, 0),
                    None => (0, 0),
                },
            };

            if *gid == current_high_gid {
                // subsequent GID
                current_width_vec.push(Integer((width as f32 * percentage_font_scaling) as i64));
                current_high_gid += 1;
            } else {
                // non-subsequent GID
                widths_list.push(Integer(current_low_gid as i64));
                widths_list.push(Array(std::mem::take(&mut current_width_vec)));

                current_width_vec.push(Integer((width as f32 * percentage_font_scaling) as i64));
                current_low_gid = *gid;
                current_high_gid = gid + 1;
            }
        }

        // push the last widths, because the loop is delayed by one iteration
        widths_list.push(Integer(current_low_gid as i64));
        widths_list.push(Array(std::mem::take(&mut current_width_vec)));

        widths_list
        /*
        let mut cmap = glyph_ids.iter()
        .filter_map(|(glyph_id, c)| {
            let (glyph_width, glyph_height) = self.get_glyph_size(*glyph_id)?;
            let k = *glyph_id as u32;
            let v = (*c as u32, glyph_width.abs() as u32, glyph_height.abs() as u32);
            Some((k, v))
        }).collect::<BTreeMap<_, _>>();

        cmap.insert(0, (0, 1000, 1000));

        widths.push((*glyph_id, width));
        */
    }

    /// Returns the maximum height in UNSCALED units of the used glyph IDs
    pub(crate) fn get_max_height(&self, glyph_ids: &BTreeMap<u16, char>) -> i64 {
        let mut max_height = 0;
        for (glyph_id, _) in glyph_ids.iter() {
            if let Some((_, glyph_height)) = self.get_glyph_size(*glyph_id) {
                max_height = max_height.max(glyph_height as i64);
            }
        }
        max_height
    }

    /// Returns the total width in UNSCALED units of the used glyph IDs
    pub(crate) fn get_total_width(&self, glyph_ids: &BTreeMap<u16, char>) -> i64 {
        let mut total_width = 0;
        for (glyph_id, _) in glyph_ids.iter() {
            if let Some((glyph_width, _)) = self.get_glyph_size(*glyph_id) {
                total_width += glyph_width as i64;
            }
        }
        total_width
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
    operations: Vec<GlyphOutlineOperation>,
}

/*
impl ttf_parser::OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::MoveTo(OutlineMoveTo { x, y })); }
    fn line_to(&mut self, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::LineTo(OutlineLineTo { x, y })); }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::QuadraticCurveTo(OutlineQuadTo { ctrl_1_x: x1, ctrl_1_y: y1, end_x: x, end_y: y })); }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) { self.operations.push(GlyphOutlineOperation::CubicCurveTo(OutlineCubicTo { ctrl_1_x: x1, ctrl_1_y: y1, ctrl_2_x: x2, ctrl_2_y: y2, end_x: x, end_y: y })); }
    fn close(&mut self) { self.operations.push(GlyphOutlineOperation::ClosePath); }
}
*/

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
    fn from_glyph_data(glyph: &Glyph<'_>, horz_advance: u16) -> Option<Self> {
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
    pub fn from_bytes(font_bytes: &[u8], font_index: usize) -> Option<Self> {
        use allsorts::tag;

        let scope = ReadScope::new(font_bytes);
        let font_file = scope.read::<FontData<'_>>().ok()?;
        let provider = font_file.table_provider(font_index).ok()?;

        let head_table = provider
            .table_data(tag::HEAD)
            .ok()
            .and_then(|head_data| ReadScope::new(&head_data?).read::<HeadTable>().ok());

        let maxp_table = provider
            .table_data(tag::MAXP)
            .ok()
            .and_then(|maxp_data| ReadScope::new(&maxp_data?).read::<MaxpTable>().ok())
            .unwrap_or(MaxpTable {
                num_glyphs: 0,
                version1_sub_table: None,
            });

        let index_to_loc = head_table
            .map(|s| s.index_to_loc_format)
            .unwrap_or(IndexToLocFormat::Long);
        let num_glyphs = maxp_table.num_glyphs as usize;

        let loca_table = provider.table_data(tag::LOCA).ok();
        let loca_table = loca_table
            .as_ref()
            .and_then(|loca_data| {
                ReadScope::new(loca_data.as_ref()?)
                    .read_dep::<LocaTable<'_>>((num_glyphs, index_to_loc))
                    .ok()
            })
            .unwrap_or(LocaTable {
                offsets: LocaOffsets::Long(ReadArray::empty()),
            });

        let glyf_table = provider.table_data(tag::GLYF).ok();
        let mut glyf_table = glyf_table
            .as_ref()
            .and_then(|glyf_data| {
                ReadScope::new(glyf_data.as_ref()?)
                    .read_dep::<GlyfTable<'_>>(&loca_table)
                    .ok()
            })
            .unwrap_or(GlyfTable::new(Vec::new()).unwrap());

        let second_scope = ReadScope::new(font_bytes);
        let second_font_file = second_scope.read::<FontData<'_>>().ok()?;
        let second_provider = second_font_file.table_provider(font_index).ok()?;

        let font_data_impl = allsorts::font::Font::new(second_provider).ok()?;

        // required for font layout: gsub_cache, gpos_cache and gdef_table
        let gsub_cache = None; // font_data_impl.gsub_cache().ok().and_then(|s| s);
        let gpos_cache = None; // font_data_impl.gpos_cache().ok().and_then(|s| s);
        let opt_gdef_table = None; // font_data_impl.gdef_table().ok().and_then(|o| o);
        let num_glyphs = font_data_impl.num_glyphs();

        let cmap_subtable = ReadScope::new(font_data_impl.cmap_subtable_data());
        let cmap_subtable = cmap_subtable
            .read::<CmapSubtable<'_>>()
            .ok()
            .and_then(|s| s.to_owned());

        if cmap_subtable.is_none() {
            println!("warning: no cmap subtable");
        }

        let hmtx_data = provider
            .table_data(tag::HMTX)
            .ok()
            .and_then(|s| Some(s?.into_owned()))
            .unwrap_or_default();

        let vmtx_data = provider
            .table_data(tag::VMTX)
            .ok()
            .and_then(|s| Some(s?.into_owned()))
            .unwrap_or_default();

        let hhea_table = provider
            .table_data(tag::HHEA)
            .ok()
            .and_then(|hhea_data| ReadScope::new(&hhea_data?).read::<HheaTable>().ok())
            .unwrap_or(HheaTable {
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
            });

        let font_metrics = FontMetrics::from_bytes(font_bytes, font_index);

        // not parsing glyph outlines can save lots of memory
        let glyph_records_decoded = glyf_table
            .records_mut()
            .iter_mut()
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
                    glyph_index,
                )
                .unwrap_or_default();

                match glyph_record {
                    GlyfRecord::Present { .. } => None,
                    GlyfRecord::Parsed(g) => {
                        OwnedGlyph::from_glyph_data(g, horz_advance).map(|s| (glyph_index, s))
                    }
                }
            })
            .collect::<Vec<_>>();

        let glyph_records_decoded = glyph_records_decoded.into_iter().collect();

        let mut font = ParsedFont {
            font_metrics,
            num_glyphs,
            hhea_table: Some(hhea_table),
            hmtx_data,
            vmtx_data,
            maxp_table: Some(maxp_table),
            gsub_cache,
            gpos_cache,
            opt_gdef_table,
            cmap_subtable,
            glyph_records_decoded,
            original_bytes: font_bytes.to_vec(),
            original_index: font_index,
            space_width: None,
        };

        let space_width = font.get_space_width_internal();
        font.space_width = space_width;

        Some(font)
    }

    fn get_space_width_internal(&mut self) -> Option<usize> {
        let glyph_index = self.lookup_glyph_index(' ' as u32)?;
        let maxp_table = self.maxp_table.as_ref()?;
        let hhea_table = self.hhea_table.as_ref()?;
        allsorts::glyph_info::advance(&maxp_table, &hhea_table, &self.hmtx_data, glyph_index)
            .ok()
            .map(|s| s as usize)
    }

    /// Returns the width of the space " " character (unscaled units)
    #[inline]
    pub const fn get_space_width(&self) -> Option<usize> {
        self.space_width
    }

    /// Get the horizontal advance of a glyph index (unscaled units)
    pub fn get_horizontal_advance(&self, glyph_index: u16) -> u16 {
        self.glyph_records_decoded
            .get(&glyph_index)
            .map(|gi| gi.horz_advance)
            .unwrap_or_default()
    }

    // get the x and y size of a glyph (unscaled units)
    pub fn get_glyph_size(&self, glyph_index: u16) -> Option<(i32, i32)> {
        let g = self.glyph_records_decoded.get(&glyph_index)?;
        let glyph_width = g.horz_advance as i32;
        let glyph_height = g.bounding_box.max_y as i32 - g.bounding_box.min_y as i32; // height
        Some((glyph_width, glyph_height))
    }

    pub fn lookup_glyph_index(&self, c: u32) -> Option<u16> {
        match self.cmap_subtable.as_ref()?.map_glyph(c) {
            Ok(Some(c)) => Some(c),
            _ => None,
        }
    }
}

type GlyphId = u16;
type UnicodeCodePoint = u32;
type CmapBlock = Vec<(GlyphId, UnicodeCodePoint)>;

/// Generates a CMAP (character map) from valid cmap blocks
fn generate_cid_to_unicode_map(face_name: String, all_cmap_blocks: Vec<CmapBlock>) -> String {
    let mut cid_to_unicode_map = format!(include_str!("./res/gid_to_unicode_beg.txt"), face_name);

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

    cid_to_unicode_map.push_str(include_str!("./res/gid_to_unicode_end.txt"));
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
    /// Only for testing, zero-sized font, will always return 0 for every metric (`units_per_em =
    /// 1000`)
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
            Some(Some(s)) => Os2Info {
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

                us_lower_optical_point_size: s
                    .version5
                    .as_ref()
                    .map(|q| q.us_lower_optical_point_size),
                us_upper_optical_point_size: s
                    .version5
                    .as_ref()
                    .map(|q| q.us_upper_optical_point_size),
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
            s_typo_ascender: os2_table.s_typo_ascender,
            s_typo_descender: os2_table.s_typo_descender,
            s_typo_line_gap: os2_table.s_typo_line_gap,
            us_win_ascent: os2_table.us_win_ascent,
            us_win_descent: os2_table.us_win_descent,
            ul_code_page_range1: os2_table.ul_code_page_range1,
            ul_code_page_range2: os2_table.ul_code_page_range2,
            sx_height: os2_table.sx_height,
            s_cap_height: os2_table.s_cap_height,
            us_default_char: os2_table.us_default_char,
            us_break_char: os2_table.us_break_char,
            us_max_context: os2_table.us_max_context,
            us_lower_optical_point_size: os2_table.us_lower_optical_point_size,
            us_upper_optical_point_size: os2_table.us_upper_optical_point_size,
        }
    }

    /// If set, use `OS/2.sTypoAscender - OS/2.sTypoDescender + OS/2.sTypoLineGap` to calculate the
    /// height
    ///
    /// See [`USE_TYPO_METRICS`](https://docs.microsoft.com/en-us/typography/opentype/spec/os2#fss)
    pub fn use_typo_metrics(&self) -> bool {
        self.fs_selection & (1 << 7) != 0
    }

    pub fn get_ascender_unscaled(&self) -> i16 {
        let use_typo = if !self.use_typo_metrics() {
            None
        } else {
            self.s_typo_ascender
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
            self.s_typo_descender
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
            self.s_typo_line_gap
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
