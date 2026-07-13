use std::{
    collections::btree_map::BTreeMap,
    fmt,
    str::FromStr,
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
    /// Font type (TrueType vs OpenType/CFF) - needed for correct PDF serialization
    pub font_type: FontType,
    /// PDF font bounding box and metrics - needed for font descriptor in PDF
    pub pdf_font_metrics: PdfFontMetricsStub,
}

/// Minimal font metrics needed for PDF font descriptors when text_layout is disabled
#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfFontMetricsStub {
    pub units_per_em: u16,
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
}

#[cfg(not(feature = "text_layout"))]
impl Default for PdfFontMetricsStub {
    fn default() -> Self {
        Self {
            units_per_em: 1000,
            x_min: 0,
            y_min: -200,
            x_max: 1000,
            y_max: 800,
        }
    }
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
            units_per_em: 1000,
            font_metrics: FontMetrics {
                ascent: 800,
                descent: -200,
            },
            font_type: FontType::TrueType,
            pdf_font_metrics: PdfFontMetricsStub::default(),
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
            font_type: FontType::TrueType,
            pdf_font_metrics: PdfFontMetricsStub { units_per_em, ..Default::default() },
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

/// A four-byte OpenType variation-axis tag such as `wght` or `opsz`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontVariationTag([u8; 4]);

impl FontVariationTag {
    pub const WGHT: Self = Self(*b"wght");
    pub const WDTH: Self = Self(*b"wdth");
    pub const OPSZ: Self = Self(*b"opsz");
    pub const SLNT: Self = Self(*b"slnt");
    pub const ITAL: Self = Self(*b"ital");

    /// Construct a tag after validating the OpenType tag syntax.
    pub fn new(bytes: [u8; 4]) -> Result<Self, VariableFontError> {
        validate_variation_tag(bytes)?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    pub const fn as_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

impl TryFrom<&str> for FontVariationTag {
    type Error = VariableFontError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let bytes: [u8; 4] = value
            .as_bytes()
            .try_into()
            .map_err(|_| VariableFontError::InvalidTag(value.to_string()))?;
        Self::new(bytes)
    }
}

impl FromStr for FontVariationTag {
    type Err = VariableFontError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl fmt::Display for FontVariationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Construction guarantees ASCII bytes.
        let tag = std::str::from_utf8(&self.0).map_err(|_| fmt::Error)?;
        f.write_str(tag)
    }
}

fn validate_variation_tag(bytes: [u8; 4]) -> Result<(), VariableFontError> {
    let first_is_letter = bytes[0].is_ascii_alphabetic();
    let valid_bytes = bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b' ');
    let spaces_are_trailing = bytes
        .iter()
        .position(|byte| *byte == b' ')
        .map(|first_space| bytes[first_space..].iter().all(|byte| *byte == b' '))
        .unwrap_or(true);

    if first_is_letter && valid_bytes && spaces_are_trailing {
        Ok(())
    } else {
        Err(VariableFontError::InvalidTag(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    }
}

/// Metadata for one axis declared by a variable font's `fvar` table.
#[derive(Debug, Clone, PartialEq)]
pub struct FontVariationAxis {
    pub tag: FontVariationTag,
    pub name: Option<String>,
    pub min_value: f32,
    pub default_value: f32,
    pub max_value: f32,
    pub hidden: bool,
}

/// User-space coordinates selecting an instance of a variable font.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontVariationSettings {
    pub coordinates: BTreeMap<FontVariationTag, f32>,
}

impl FontVariationSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, tag: FontVariationTag, value: f32) -> Option<f32> {
        self.coordinates.insert(tag, value)
    }

    pub fn with(mut self, tag: FontVariationTag, value: f32) -> Self {
        self.insert(tag, value);
        self
    }
}

/// A parsed, PDF-compatible static instance derived from a variable font.
#[cfg(feature = "text_layout")]
#[derive(Debug, Clone)]
pub struct VariableFontInstance {
    pub font: ParsedFont,
    /// Effective coordinates after defaults, clamping, and Fixed 16.16 rounding.
    pub resolved_coordinates: BTreeMap<FontVariationTag, f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariableFontError {
    InvalidFont(String),
    InvalidCollectionIndex { index: usize },
    NotVariable,
    InvalidTag(String),
    UnknownAxis(FontVariationTag),
    NonFiniteValue { tag: FontVariationTag, value: f32 },
    UnsupportedOutlineFormat(String),
    Instancing(String),
    StaticFontParse(String),
    PdfConversion(String),
}

impl fmt::Display for VariableFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFont(error) => write!(f, "invalid font: {error}"),
            Self::InvalidCollectionIndex { index } => {
                write!(f, "invalid font collection index {index}")
            }
            Self::NotVariable => f.write_str("font does not contain a usable fvar table"),
            Self::InvalidTag(tag) => write!(f, "invalid OpenType variation tag {tag:?}"),
            Self::UnknownAxis(tag) => write!(f, "font does not declare variation axis {tag}"),
            Self::NonFiniteValue { tag, value } => {
                write!(f, "variation axis {tag} has non-finite value {value}")
            }
            Self::UnsupportedOutlineFormat(format) => {
                write!(f, "unsupported variable-font outline format: {format}")
            }
            Self::Instancing(error) => write!(f, "variable-font instancing failed: {error}"),
            Self::StaticFontParse(error) => {
                write!(f, "failed to parse generated static font: {error}")
            }
            Self::PdfConversion(error) => {
                write!(f, "failed to create a PDF-compatible static font: {error}")
            }
        }
    }
}

impl std::error::Error for VariableFontError {}

/// Inspect the axes declared by one face of a variable font.
pub fn font_variation_axes(
    bytes: &[u8],
    font_index: usize,
) -> Result<Vec<FontVariationAxis>, VariableFontError> {
    use allsorts::{
        binary::read::ReadScope,
        font_data::FontData,
        tables::{variable_fonts::fvar::FvarTable, FontTableProvider, NameTable},
        tag,
    };

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?;
    validate_font_collection_index(&font_file, font_index)?;
    let provider = font_file
        .table_provider(font_index)
        .map_err(|_| VariableFontError::InvalidCollectionIndex { index: font_index })?;
    let fvar_data = provider
        .table_data(tag::FVAR)
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?
        .ok_or(VariableFontError::NotVariable)?;
    let fvar = ReadScope::new(&fvar_data)
        .read::<FvarTable<'_>>()
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?;

    if fvar.axis_count() == 0 {
        return Err(VariableFontError::NotVariable);
    }

    let name_data = provider.table_data(tag::NAME).ok().flatten();
    let name_table = name_data
        .as_ref()
        .and_then(|data| ReadScope::new(data).read::<NameTable<'_>>().ok());

    fvar.axes()
        .map(|axis| {
            let tag = FontVariationTag::new(axis.axis_tag.to_be_bytes())?;
            let min_value = f32::from(axis.min_value);
            let default_value = f32::from(axis.default_value);
            let max_value = f32::from(axis.max_value);
            if !(min_value <= default_value && default_value <= max_value) {
                return Err(VariableFontError::InvalidFont(format!(
                    "variation axis {tag} has invalid range {min_value}..={max_value} with default {default_value}"
                )));
            }
            Ok(FontVariationAxis {
                tag,
                name: name_table
                    .as_ref()
                    .and_then(|table| table.string_for_id(axis.axis_name_id)),
                min_value,
                default_value,
                max_value,
                hidden: axis.flags & 0x0001 != 0,
            })
        })
        .collect()
}

/// Materialize a variable font as static sfnt bytes suitable for parsing and PDF embedding.
///
/// Omitted axes use their font-defined defaults. Out-of-range values are clamped and reported
/// through `warnings`. CFF2 input is converted to a static CFF1-flavored OpenType font.
pub fn instantiate_variable_font_bytes(
    bytes: &[u8],
    font_index: usize,
    settings: &FontVariationSettings,
    warnings: &mut Vec<crate::PdfWarnMsg>,
) -> Result<(Vec<u8>, BTreeMap<FontVariationTag, f32>), VariableFontError> {
    use allsorts::{
        binary::read::ReadScope,
        font_data::FontData,
        tables::{variable_fonts::fvar::FvarTable, Fixed, FontTableProvider},
        tag,
    };

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?;
    validate_font_collection_index(&font_file, font_index)?;
    let provider = font_file
        .table_provider(font_index)
        .map_err(|_| VariableFontError::InvalidCollectionIndex { index: font_index })?;
    let fvar_data = provider
        .table_data(tag::FVAR)
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?
        .ok_or(VariableFontError::NotVariable)?;
    let fvar = ReadScope::new(&fvar_data)
        .read::<FvarTable<'_>>()
        .map_err(|error| VariableFontError::InvalidFont(error.to_string()))?;

    if fvar.axis_count() == 0 {
        return Err(VariableFontError::NotVariable);
    }

    let declared_tags = fvar
        .axes()
        .map(|axis| FontVariationTag::new(axis.axis_tag.to_be_bytes()))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;

    for (&tag, &value) in &settings.coordinates {
        if !declared_tags.contains(&tag) {
            return Err(VariableFontError::UnknownAxis(tag));
        }
        if !value.is_finite() {
            return Err(VariableFontError::NonFiniteValue { tag, value });
        }
    }

    let mut user_instance = Vec::with_capacity(usize::from(fvar.axis_count()));
    let mut resolved_coordinates = BTreeMap::new();
    for axis in fvar.axes() {
        let axis_tag = FontVariationTag::new(axis.axis_tag.to_be_bytes())?;
        let min = f32::from(axis.min_value);
        let default = f32::from(axis.default_value);
        let max = f32::from(axis.max_value);
        if !(min <= default && default <= max) {
            return Err(VariableFontError::InvalidFont(format!(
                "variation axis {axis_tag} has invalid range {min}..={max} with default {default}"
            )));
        }
        let requested = settings
            .coordinates
            .get(&axis_tag)
            .copied()
            .unwrap_or(default);
        let clamped = requested.clamp(min, max);

        if requested != clamped {
            warnings.push(crate::PdfWarnMsg::warning(
                0,
                0,
                format!(
                    "Variable font axis {axis_tag} value {requested} was clamped to {clamped} (supported range {min}..={max})"
                ),
            ));
        }

        let fixed = Fixed::from(clamped);
        user_instance.push(fixed);
        resolved_coordinates.insert(axis_tag, f32::from(fixed));
    }

    let source_is_cff2 = provider.has_table(tag::CFF2);
    let source_is_truetype = provider.has_table(tag::GLYF) && provider.has_table(tag::GVAR);
    if !source_is_cff2 && !source_is_truetype {
        return Err(VariableFontError::UnsupportedOutlineFormat(
            "expected glyf/gvar or CFF2 tables".to_string(),
        ));
    }

    let (mut static_bytes, _) = allsorts::variations::instance(&provider, &user_instance)
        .map_err(|error| VariableFontError::Instancing(error.to_string()))?;

    if source_is_cff2 {
        static_bytes = convert_static_cff2_to_cff1(&static_bytes)?;
    }
    validate_static_font_for_pdf(&static_bytes)?;

    Ok((static_bytes, resolved_coordinates))
}

fn validate_font_collection_index(
    font_file: &allsorts::font_data::FontData<'_>,
    font_index: usize,
) -> Result<(), VariableFontError> {
    use allsorts::{font_data::FontData, tables::OpenTypeData};

    let definitely_single_face = match font_file {
        FontData::OpenType(font) => matches!(&font.data, OpenTypeData::Single(_)),
        FontData::Woff(_) => true,
        FontData::Woff2(font) => font.collection_directory.is_none(),
    };
    if definitely_single_face && font_index != 0 {
        Err(VariableFontError::InvalidCollectionIndex { index: font_index })
    } else {
        Ok(())
    }
}

/// Instantiate and parse a variable font before it enters the text/PDF pipeline.
#[cfg(feature = "text_layout")]
pub fn instantiate_variable_font(
    bytes: &[u8],
    font_index: usize,
    settings: &FontVariationSettings,
    warnings: &mut Vec<crate::PdfWarnMsg>,
) -> Result<VariableFontInstance, VariableFontError> {
    let (static_bytes, resolved_coordinates) =
        instantiate_variable_font_bytes(bytes, font_index, settings, warnings)?;
    let mut font_warnings = Vec::new();
    let mut parsed_font =
        ParsedFont::from_bytes(&static_bytes, 0, &mut font_warnings).ok_or_else(|| {
            VariableFontError::StaticFontParse(format_font_parse_warnings(&font_warnings))
        })?;

    forward_font_parse_warnings(font_warnings, warnings);
    set_parsed_font_type_from_bytes(&mut parsed_font, &static_bytes)?;

    Ok(VariableFontInstance {
        font: parsed_font,
        resolved_coordinates,
    })
}

#[cfg(feature = "text_layout")]
impl crate::PdfDocument {
    /// Add a selected variable-font instance as a static PDF font resource.
    pub fn add_variable_font(
        &mut self,
        bytes: &[u8],
        font_index: usize,
        settings: &FontVariationSettings,
        warnings: &mut Vec<crate::PdfWarnMsg>,
    ) -> Result<FontId, VariableFontError> {
        let instance = instantiate_variable_font(bytes, font_index, settings, warnings)?;
        Ok(self.add_font(&instance.font))
    }
}

fn convert_static_cff2_to_cff1(bytes: &[u8]) -> Result<Vec<u8>, VariableFontError> {
    use allsorts::{
        binary::read::ReadScope,
        font_data::FontData,
        subset::{CmapTarget, SubsetProfile},
        tables::{FontTableProvider, MaxpTable},
        tag,
    };

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let maxp_data = provider
        .read_table_data(tag::MAXP)
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let maxp = ReadScope::new(&maxp_data)
        .read::<MaxpTable>()
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let glyph_ids = (0..maxp.num_glyphs).collect::<Vec<_>>();
    let profile = SubsetProfile::Custom(vec![
        tag::CMAP,
        tag::HEAD,
        tag::HHEA,
        tag::HMTX,
        tag::MAXP,
        tag::NAME,
        tag::OS_2,
        tag::POST,
        tag::GPOS,
        tag::GSUB,
        tag::GDEF,
        tag::VHEA,
        tag::VMTX,
        tag::CVT,
        tag::FPGM,
        tag::PREP,
    ]);

    allsorts::subset::subset(&provider, &glyph_ids, &profile, CmapTarget::Unicode)
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))
}

pub(crate) fn validate_static_font_for_pdf(bytes: &[u8]) -> Result<(), VariableFontError> {
    validate_static_font_face_for_pdf(bytes, 0)
}

pub(crate) fn validate_static_font_face_for_pdf(
    bytes: &[u8],
    font_index: usize,
) -> Result<(), VariableFontError> {
    use allsorts::{binary::read::ReadScope, font_data::FontData, tables::FontTableProvider, tag};

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let provider = font_file
        .table_provider(font_index)
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    if let Some(table) = unresolved_variation_table(&provider) {
        return Err(VariableFontError::PdfConversion(format!(
            "font still contains unresolved variation table {}",
            allsorts::tag::DisplayTag(table)
        )));
    }
    if !provider.has_table(tag::GLYF) && !provider.has_table(tag::CFF) {
        return Err(VariableFontError::PdfConversion(
            "generated font has neither glyf nor CFF outlines".to_string(),
        ));
    }
    Ok(())
}

fn unresolved_variation_table(provider: &impl allsorts::tables::FontTableProvider) -> Option<u32> {
    use allsorts::tag;

    [
        tag::FVAR,
        tag::GVAR,
        tag::CVAR,
        tag::HVAR,
        tag::MVAR,
        tag::AVAR,
        tag::CFF2,
        u32::from_be_bytes(*b"VVAR"),
    ]
    .into_iter()
    .find(|table| provider.has_table(*table))
}

pub(crate) fn unresolved_variation_table_in_font(
    bytes: &[u8],
    font_index: usize,
) -> Result<Option<u32>, VariableFontError> {
    use allsorts::{binary::read::ReadScope, font_data::FontData};

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    let provider = font_file
        .table_provider(font_index)
        .map_err(|error| VariableFontError::PdfConversion(error.to_string()))?;
    Ok(unresolved_variation_table(&provider))
}

pub(crate) fn set_parsed_font_type_from_bytes(
    parsed_font: &mut ParsedFont,
    bytes: &[u8],
) -> Result<(), VariableFontError> {
    use allsorts::{binary::read::ReadScope, font_data::FontData, tables::FontTableProvider, tag};

    let scope = ReadScope::new(bytes);
    let font_file = scope
        .read::<FontData<'_>>()
        .map_err(|error| VariableFontError::StaticFontParse(error.to_string()))?;
    let provider = font_file
        .table_provider(0)
        .map_err(|error| VariableFontError::StaticFontParse(error.to_string()))?;

    if let Some(cff) = provider
        .table_data(tag::CFF)
        .map_err(|error| VariableFontError::StaticFontParse(error.to_string()))?
    {
        #[cfg(feature = "text_layout")]
        {
            parsed_font.font_type = FontType::OpenTypeCFF(cff.into_owned());
            parsed_font.index_to_cid = (0..parsed_font.num_glyphs)
                .map(|glyph_id| (glyph_id, glyph_id))
                .collect();
        }
        #[cfg(not(feature = "text_layout"))]
        {
            let _ = cff;
            parsed_font.font_type = FontType::OpenTypeCFF(());
        }
    } else {
        parsed_font.font_type = FontType::TrueType;
    }
    Ok(())
}

#[cfg(feature = "text_layout")]
fn format_font_parse_warnings(warnings: &[PdfFontParseWarning]) -> String {
    warnings
        .iter()
        .map(|warning| warning.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(feature = "text_layout")]
fn forward_font_parse_warnings(
    font_warnings: Vec<PdfFontParseWarning>,
    warnings: &mut Vec<crate::PdfWarnMsg>,
) {
    use azul_layout::font::parsed::FontParseWarningSeverity;

    warnings.extend(font_warnings.into_iter().filter_map(|warning| {
        match warning.severity {
            FontParseWarningSeverity::Info => None,
            FontParseWarningSeverity::Warning => {
                Some(crate::PdfWarnMsg::warning(0, 0, warning.message))
            }
            FontParseWarningSeverity::Error => {
                Some(crate::PdfWarnMsg::error(0, 0, warning.message))
            }
        }
    }));
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

    // allsorts requires .notdef (GID 0) first. Remaining IDs stay sorted.
    let ids: Vec<_> = std::iter::once(0)
        .chain(glyph_ids.keys().copied().filter(|glyph_id| *glyph_id != 0))
        .collect();

    // Use SubsetProfile::Pdf for PDF embedding and CmapTarget::Unicode for Unicode cmap
    let bytes = allsorts::subset::subset(
        &provider,
        &ids,
        &allsorts::subset::SubsetProfile::Pdf,
        CmapTarget::Unicode,
    ).map_err(|e| e.to_string())?;

    // allsorts assigns new GIDs in input order, with .notdef remaining GID 0.
    let glyph_mapping: BTreeMap<u16, (u16, char)> = ids
        .iter()
        .enumerate()
        .filter_map(|(idx, &original_gid)| {
            glyph_ids.get(&original_gid).map(|&ch| {
                let new_gid = idx as u16;
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

#[cfg(all(test, feature = "text_layout"))]
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

