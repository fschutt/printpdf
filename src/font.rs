use std::{
    collections::btree_map::BTreeMap,
    vec::Vec,
};

use serde_derive::{Deserialize, Serialize};

use crate::{
    FontId,
};

// Use azul-layout's types instead of redefining them
#[cfg(feature = "text_layout")]
pub use azul_layout::{
    PdfFontMetrics as FontMetrics, FontParseWarning as PdfFontParseWarning, FontType, OwnedGlyph, ParsedFont,
};

// Stub types when text_layout is disabled
#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFont {
    pub original_bytes: Vec<u8>,
    pub font_index: u32,
    pub font_name: Option<String>,
    /// Manual Unicode codepoint -> glyph ID mapping
    /// Used when text_layout is disabled to provide character to glyph mapping
    pub codepoint_to_glyph: BTreeMap<u32, u16>,
    /// Manual glyph widths mapping (glyph_id -> width in font units)
    /// Used when text_layout is disabled to provide font metrics
    pub glyph_widths: BTreeMap<u16, u16>,
    /// Manual units per em value (typically 1000 or 2048)
    pub units_per_em: u16,
    /// Manual font metrics
    pub font_metrics: FontMetrics,
}

#[cfg(not(feature = "text_layout"))]
impl ParsedFont {
    pub fn from_bytes(bytes: &[u8], index: u32, _warnings: &mut Vec<String>) -> Option<Self> {
        Some(ParsedFont {
            original_bytes: bytes.to_vec(),
            font_index: index,
            font_name: None,
            codepoint_to_glyph: BTreeMap::new(),
            glyph_widths: BTreeMap::new(),
            units_per_em: 1000, // Default value
            font_metrics: FontMetrics {
                ascent: 800,
                descent: -200,
            },
        })
    }
    
    /// Create a ParsedFont with manual glyph mappings and widths
    pub fn with_glyph_data(
        bytes: Vec<u8>,
        index: u32,
        font_name: Option<String>,
        codepoint_to_glyph: BTreeMap<u32, u16>,
        glyph_widths: BTreeMap<u16, u16>,
        units_per_em: u16,
        font_metrics: FontMetrics,
    ) -> Self {
        ParsedFont {
            original_bytes: bytes,
            font_index: index,
            font_name,
            codepoint_to_glyph,
            glyph_widths,
            units_per_em,
            font_metrics,
        }
    }
    
    /// Set Unicode codepoint to glyph ID mapping
    pub fn set_codepoint_mapping(&mut self, codepoint: u32, gid: u16) {
        self.codepoint_to_glyph.insert(codepoint, gid);
    }
    
    /// Set glyph width for a specific glyph ID
    pub fn set_glyph_width(&mut self, gid: u16, width: u16) {
        self.glyph_widths.insert(gid, width);
    }
    
    /// Get glyph width for a specific glyph ID
    pub fn get_glyph_width(&self, gid: u16) -> Option<u16> {
        self.glyph_widths.get(&gid).copied()
    }
    
    /// Lookup glyph index for a Unicode codepoint
    pub fn lookup_glyph_index(&self, codepoint: u32) -> Option<u16> {
        self.codepoint_to_glyph.get(&codepoint).copied()
    }
    
    /// Returns None without panicking - reverse lookup is not available without text_layout feature
    pub fn get_glyph_primary_char(&self, _gid: u16) -> Option<char> {
        None
    }
}

#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontType {
    TrueType,
    OpenTypeCFF(()),
}

#[cfg(not(feature = "text_layout"))]
pub type FontParseWarning = String;

#[cfg(not(feature = "text_layout"))]
pub type PdfFontParseWarning = String;

#[cfg(not(feature = "text_layout"))]
pub type OwnedGlyph = ();

#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontMetrics {
    pub ascent: i16,
    pub descent: i16,
}

/// Result of subsetting a font
#[derive(Debug, Clone)]
pub struct SubsetFont {
    pub bytes: Vec<u8>,
    pub glyph_mapping: BTreeMap<u16, (u16, char)>,
}

/// PDF-specific metadata for fonts that doesn't belong in azul_layout::ParsedFont
/// This stores information needed for PDF generation but not for layout
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintpdfFontMeta {
    /// Original GID -> CID mapping (if this font was loaded from a PDF)
    pub original_gid_to_cid: Option<BTreeMap<u16, u16>>,
    /// ToUnicode CMap data (if this font was loaded from a PDF)
    pub original_to_unicode_map: Option<String>,
    /// Font embedding preferences
    pub embedding_mode: FontEmbeddingMode,
    /// Whether this font requires special handling
    pub requires_subsetting: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FontEmbeddingMode {
    /// Embed the full font
    Full,
    /// Subset the font (default)
    Subset,
    /// Reference only (for system fonts)
    Reference,
}

impl Default for PrintpdfFontMeta {
    fn default() -> Self {
        Self {
            original_gid_to_cid: None,
            original_to_unicode_map: None,
            embedding_mode: FontEmbeddingMode::Subset,
            requires_subsetting: true,
        }
    }
}

/// Combined font data for PDF generation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfFont {
    /// The actual font data
    pub parsed_font: ParsedFont,
    /// PDF-specific metadata
    pub meta: PrintpdfFontMeta,
}

impl PdfFont {
    pub fn new(parsed_font: ParsedFont) -> Self {
        Self {
            parsed_font,
            meta: PrintpdfFontMeta::default(),
        }
    }

    pub fn with_meta(parsed_font: ParsedFont, meta: PrintpdfFontMeta) -> Self {
        Self { parsed_font, meta }
    }
}

/// Builtin or external font
#[derive(Debug, Clone)]
pub enum Font {
    /// Represents one of the 14 built-in fonts (Arial, Helvetica, etc.)
    BuiltinFont(BuiltinFont),
    /// Represents a font loaded from an external file
    /// Contains both the ParsedFont and PDF-specific metadata
    ExternalFont(ParsedFont, PrintpdfFontMeta),
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

impl Default for BuiltinFont {
    fn default() -> Self {
        Self::TimesRoman // HTML default is serif (Times New Roman)
    }
}

include!("../defaultfonts/mapping.rs");

impl BuiltinFont {
    pub fn check_if_matches(bytes: &[u8]) -> Option<Self> {
        let matching_based_on_len = match_len(bytes)?;
        // if the length is equal, check for equality
        if bytes == matching_based_on_len.get_subset_font().bytes.as_slice() {
            Some(matching_based_on_len)
        } else {
            None
        }
    }

    /// Get a ParsedFont for this builtin font
    /// This allows builtin fonts to support text shaping
    pub fn get_parsed_font(&self) -> Option<ParsedFont> {
        let subset = self.get_subset_font();
        ParsedFont::from_bytes(&subset.bytes, 0, &mut Vec::new())
    }

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
                TimesRoman => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Times-Roman.subset.ttf"
                )),
                TimesBold => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Times-Bold.subset.ttf"
                )),
                TimesItalic => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Times-Italic.subset.ttf"
                )),
                TimesBoldItalic => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Times-BoldItalic.subset.ttf"
                )),
                Helvetica => {
                    crate::utils::uncompress(include_bytes!("../defaultfonts/Helvetica.subset.ttf"))
                }
                HelveticaBold => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Helvetica-Bold.subset.ttf"
                )),
                HelveticaOblique => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Helvetica-Oblique.subset.ttf"
                )),
                HelveticaBoldOblique => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Helvetica-BoldOblique.subset.ttf"
                )),
                Courier => {
                    crate::utils::uncompress(include_bytes!("../defaultfonts/Courier.subset.ttf"))
                }
                CourierOblique => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Courier-Oblique.subset.ttf"
                )),
                CourierBold => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Courier-Bold.subset.ttf"
                )),
                CourierBoldOblique => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/Courier-BoldOblique.subset.ttf"
                )),
                Symbol => {
                    crate::utils::uncompress(include_bytes!("../defaultfonts/Symbol.subset.ttf"))
                }
                ZapfDingbats => crate::utils::uncompress(include_bytes!(
                    "../defaultfonts/ZapfDingbats.subset.ttf"
                )),
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
            "Times-Roman" | "F1" => Some(TimesRoman),
            "Times-Bold" | "F2" => Some(TimesBold),
            "Times-Italic" | "F3" => Some(TimesItalic),
            "Times-BoldItalic" | "F4" => Some(TimesBoldItalic),
            "Helvetica" | "F5" => Some(Helvetica),
            "Helvetica-Bold" | "F6" => Some(HelveticaBold),
            "Helvetica-Oblique" | "F7" => Some(HelveticaOblique),
            "Helvetica-BoldOblique" | "F8" => Some(HelveticaBoldOblique),
            "Courier" | "F9" => Some(Courier),
            "Courier-Oblique" | "F10" => Some(CourierOblique),
            "Courier-Bold" | "F11" => Some(CourierBold),
            "Courier-BoldOblique" | "F12" => Some(CourierBoldOblique),
            "Symbol" | "F13" => Some(Symbol),
            "ZapfDingbats" | "F14" => Some(ZapfDingbats),
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

    pub fn all_ids() -> [BuiltinFont; 14] {
        use self::BuiltinFont::*;
        [
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
        ]
    }
}

impl Font {
    /// Get the ParsedFont if this is an ExternalFont, None otherwise
    pub fn get_parsed_font(&self) -> Option<&ParsedFont> {
        match self {
            Font::BuiltinFont(_) => None,
            Font::ExternalFont(parsed, _) => Some(parsed),
        }
    }

    /// Get mutable reference to the ParsedFont if this is an ExternalFont
    pub fn get_parsed_font_mut(&mut self) -> Option<&mut ParsedFont> {
        match self {
            Font::BuiltinFont(_) => None,
            Font::ExternalFont(parsed, _) => Some(parsed),
        }
    }

    /// Get the font metadata if this is an ExternalFont
    pub fn get_font_meta(&self) -> Option<&PrintpdfFontMeta> {
        match self {
            Font::BuiltinFont(_) => None,
            Font::ExternalFont(_, meta) => Some(meta),
        }
    }

    /// Get mutable reference to the font metadata if this is an ExternalFont
    pub fn get_font_meta_mut(&mut self) -> Option<&mut PrintpdfFontMeta> {
        match self {
            Font::BuiltinFont(_) => None,
            Font::ExternalFont(_, meta) => Some(meta),
        }
    }
}

#[cfg(feature = "text_layout")]
pub fn subset_font(font: &ParsedFont, glyph_ids: &BTreeMap<u16, char>) -> Result<SubsetFont, String> {
    use allsorts::{binary::read::ReadScope, font_data::FontData, subset::CmapTarget};

    let scope = ReadScope::new(&font.original_bytes);
    let font_file = scope.read::<FontData<'_>>().map_err(|e| e.to_string())?;
    let provider = font_file
        .table_provider(font.original_index)
        .map_err(|e| e.to_string())?;

    // Collect glyph IDs in a consistent order (BTreeMap gives sorted order)
    let ids: Vec<_> = glyph_ids.keys().copied().collect();
    
    // Use SubsetProfile::Pdf for PDF embedding and CmapTarget::Unicode for Unicode cmap
    let bytes = allsorts::subset::subset(
        &provider,
        &ids,
        &allsorts::subset::SubsetProfile::Pdf,
        CmapTarget::Unicode,
    ).map_err(|e| e.to_string())?;

    // Build glyph mapping: allsorts subset assigns new GIDs starting at 1
    // (GID 0 is always .notdef), following the order of input glyph IDs
    let glyph_mapping: BTreeMap<u16, (u16, char)> = ids
        .iter()
        .enumerate()
        .filter_map(|(idx, &original_gid)| {
            glyph_ids.get(&original_gid).map(|&ch| {
                // New GID = index + 1 (because GID 0 is .notdef)
                let new_gid = (idx + 1) as u16;
                (original_gid, (new_gid, ch))
            })
        })
        .collect();

    Ok(SubsetFont {
        bytes,
        glyph_mapping,
    })
}

#[cfg(not(feature = "text_layout"))]
pub fn subset_font(font: &ParsedFont, _glyph_ids: &BTreeMap<u16, char>) -> Result<SubsetFont, String> {
    Ok(SubsetFont {
        // Without text_layout, just return the original font bytes without subsetting
        bytes: font.original_bytes.clone(),
        // Empty mapping - user provides glyph info via Codepoint
        glyph_mapping: BTreeMap::new(),
    })
}

// PDF-specific helper functions for ParsedFont

pub fn generate_cmap_string(_font: &ParsedFont, font_id: &FontId, glyph_ids: &[(u16, char)]) -> String {
    let mappings = glyph_ids
        .iter()
        .map(|&(gid, unicode)| (gid as u32, vec![unicode as u32]))
        .collect();

    let cmap = crate::cmap::ToUnicodeCMap { mappings };
    cmap.to_cmap_string(&font_id.0)
}

#[cfg(feature = "text_layout")]
pub fn generate_gid_to_cid_map(font: &ParsedFont, glyph_ids: &[(u16, char)]) -> Vec<(u16, u16)> {
    glyph_ids
        .iter()
        .filter_map(|(gid, _)| font.index_to_cid.get(gid).map(|cid| (*gid, *cid)))
        .collect()
}

#[cfg(feature = "text_layout")]
fn get_glyph_width(font: &ParsedFont, gid: u16) -> Option<u16> {
    font.glyph_records_decoded.get(&gid).map(|g| g.horz_advance)
}

#[cfg(feature = "text_layout")]
pub fn get_normalized_widths_ttf(font: &ParsedFont, glyph_ids: &[(u16, char)]) -> Vec<lopdf::Object> {
    let mut widths_list = Vec::new();
    let mut current_low_gid = 0;
    let mut current_high_gid = 0;
    let mut current_width_vec = Vec::new();

    let percentage_font_scaling = 1000.0 / (font.pdf_font_metrics.units_per_em as f32);

    for (gid, _) in glyph_ids {
        let glyph_width = get_glyph_width(font, *gid)
            .map(|w| (w as f32 * percentage_font_scaling) as i64)
            .unwrap_or(0);

        if current_width_vec.is_empty() {
            current_low_gid = *gid;
            current_high_gid = *gid;
            current_width_vec.push(glyph_width);
        } else if *gid == current_high_gid + 1 {
            current_high_gid = *gid;
            current_width_vec.push(glyph_width);
        } else {
            widths_list.push(lopdf::Object::Integer(current_low_gid as i64));
            widths_list.push(lopdf::Object::Array(
                current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
            ));
            current_low_gid = *gid;
            current_high_gid = *gid;
            current_width_vec = vec![glyph_width];
        }
    }

    if !current_width_vec.is_empty() {
        widths_list.push(lopdf::Object::Integer(current_low_gid as i64));
        widths_list.push(lopdf::Object::Array(
            current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
        ));
    }

    widths_list
}

#[cfg(feature = "text_layout")]
pub fn get_normalized_widths_cff(font: &ParsedFont, gid_to_cid_map: &[(u16, u16)]) -> Vec<lopdf::Object> {
    let percentage_font_scaling = 1000.0 / (font.pdf_font_metrics.units_per_em as f32);

    gid_to_cid_map
        .iter()
        .map(|(gid, _cid)| {
            let width = get_glyph_width(font, *gid)
                .map(|w| (w as f32 * percentage_font_scaling) as i64)
                .unwrap_or(0);
            lopdf::Object::Integer(width)
        })
        .collect()
}

pub const FONT_B64_START: &str = "data:font/ttf;base64,";

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use crate::*;

    pub const WIN_1252: &[char; 214] = &[
        '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2',
        '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C', 'D',
        'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V',
        'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h',
        'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
        '{', '|', '}', '~', '€', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', 'Ž', '‘',
        '’', '“', '•', '–', '—', '˜', '™', 'š', '›', 'œ', 'ž', 'Ÿ', '¡', '¢', '£', '¤', '¥', '¦',
        '§', '¨', '©', 'ª', '«', '¬', '®', '¯', '°', '±', '²', '³', '´', 'µ', '¶', '·', '¸', '¹',
        'º', '»', '¼', '½', '¾', '¿', 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å', 'Æ', 'Ç', 'È', 'É', 'Ê', 'Ë',
        'Ì', 'Í', 'Î', 'Ï', 'Ð', 'Ñ', 'Ò', 'Ó', 'Ô', 'Õ', 'Ö', '×', 'Ø', 'Ù', 'Ú', 'Û', 'Ü', 'Ý',
        'Þ', 'ß', 'à', 'á', 'â', 'ã', 'ä', 'å', 'æ', 'ç', 'è', 'é', 'ê', 'ë', 'ì', 'í', 'î', 'ï',
        'ð', 'ñ', 'ò', 'ó', 'ô', 'õ', 'ö', '÷', 'ø', 'ù', 'ú', 'û', 'ü', 'ý', 'þ', 'ÿ',
    ];

    const FONTS: &[(BuiltinFont, &[u8])] = &[
        (
            BuiltinFont::Courier,
            include_bytes!("../examples/assets/fonts/Courier.ttf"),
        ),
        (
            BuiltinFont::CourierOblique,
            include_bytes!("../examples/assets/fonts/Courier-Oblique.ttf"),
        ),
        (
            BuiltinFont::CourierBold,
            include_bytes!("../examples/assets/fonts/Courier-Bold.ttf"),
        ),
        (
            BuiltinFont::CourierBoldOblique,
            include_bytes!("../examples/assets/fonts/Courier-BoldOblique.ttf"),
        ),
        (
            BuiltinFont::Helvetica,
            include_bytes!("../examples/assets/fonts/Helvetica.ttf"),
        ),
        (
            BuiltinFont::HelveticaBold,
            include_bytes!("../examples/assets/fonts/Helvetica-Bold.ttf"),
        ),
        (
            BuiltinFont::HelveticaOblique,
            include_bytes!("../examples/assets/fonts/Helvetica-Oblique.ttf"),
        ),
        (
            BuiltinFont::HelveticaBoldOblique,
            include_bytes!("../examples/assets/fonts/Helvetica-BoldOblique.ttf"),
        ),
        (
            BuiltinFont::Symbol,
            include_bytes!("../examples/assets/fonts/PDFASymbol.woff2"),
        ),
        (
            BuiltinFont::TimesRoman,
            include_bytes!("../examples/assets/fonts/Times.ttf"),
        ),
        (
            BuiltinFont::TimesBold,
            include_bytes!("../examples/assets/fonts/Times-Bold.ttf"),
        ),
        (
            BuiltinFont::TimesItalic,
            include_bytes!("../examples/assets/fonts/Times-Oblique.ttf"),
        ),
        (
            BuiltinFont::TimesBoldItalic,
            include_bytes!("../examples/assets/fonts/Times-BoldOblique.ttf"),
        ),
        (
            BuiltinFont::ZapfDingbats,
            include_bytes!("../examples/assets/fonts/ZapfDingbats.ttf"),
        ),
    ];

    #[test]
    fn subset_test() {
        use std::collections::BTreeSet;
        
        let charmap: BTreeSet<char> = WIN_1252.iter().copied().collect();
        let mut target_map = vec![];

        let mut tm2 = BTreeMap::new();
        for (name, bytes) in FONTS {
            let mut warnings = Vec::new();
            let font = ParsedFont::from_bytes(bytes, 0, &mut warnings).unwrap();
            // Convert charmap to Vec<(u16, char)> format for subset()
            let glyph_ids: Vec<(u16, char)> = charmap.iter()
                .filter_map(|&ch| font.lookup_glyph_index(ch as u32).map(|gid| (gid, ch)))
                .collect();
            let (subset_bytes, glyph_mapping) = font.subset(&glyph_ids, allsorts::subset::CmapTarget::Unicode).unwrap();
            let subset = crate::font::SubsetFont { bytes: subset_bytes, glyph_mapping };
            tm2.insert(name.clone(), subset.bytes.len());
            let _ = std::fs::write(
                format!(
                    "{}/defaultfonts/{}.subset.ttf",
                    env!("CARGO_MANIFEST_DIR"),
                    name.get_id()
                ),
                crate::utils::compress(&subset.bytes),
            );
            for (old_gid, (new_gid, char)) in subset.glyph_mapping.iter() {
                target_map.push(format!(
                    "    ({}, {old_gid}, {new_gid}, '{c}'),",
                    name.get_num(),
                    c = if *char == '\'' {
                        "\\'".to_string()
                    } else if *char == '\\' {
                        "\\\\".to_string()
                    } else {
                        char.to_string()
                    }
                ));
            }
        }

        let mut tm = vec![format!(
            "const FONTS: &[(usize, u16, u16, char);{}] = &[",
            target_map.len()
        )];
        tm.append(&mut target_map);
        tm.push("];".to_string());

        tm.push("fn match_len(bytes: &[u8]) -> Option<BuiltinFont> {".to_string());
        tm.push("match bytes.len() {".to_string());
        for (f, b) in tm2.iter() {
            tm.push(format!("{b} => Some(BuiltinFont::{f:?}),"));
        }
        tm.push("_ => None,".to_string());
        tm.push("}".to_string());
        tm.push("}".to_string());

        let _ = std::fs::write(
            format!("{}/defaultfonts/mapping.rs", env!("CARGO_MANIFEST_DIR")),
            tm.join("\r\n"),
        );
    }
}

