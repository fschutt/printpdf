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
    PdfFontMetrics as FontMetrics, FontParseWarning as PdfFontParseWarning, FontType, OwnedGlyph,
};

/// azul-layout's raw font face.
///
/// Prefer printpdf's [`ParsedFont`], which wraps this and guarantees the source
/// bytes stay attached — PDF embedding cannot work without them.
#[cfg(feature = "text_layout")]
pub use azul_layout::ParsedFont as AzulParsedFont;

#[cfg(feature = "text_layout")]
pub use self::parsed_font::ParsedFont;

#[cfg(feature = "text_layout")]
mod parsed_font {
    use std::{
        ops::{Deref, DerefMut},
        sync::Arc,
    };

    use super::{AzulParsedFont, PdfFontParseWarning};

    /// The `data:` URI prefix azul-layout (de)serializes a font face as.
    ///
    /// Duplicated here on purpose: printpdf needs its *own* `Deserialize`, because
    /// azul's routes through `AzulParsedFont::from_bytes`, which drops the source
    /// bytes on azul-layout 0.0.9 (see [`ParsedFont`]). Round-tripping a `PdfFont`
    /// through serde would otherwise silently produce an unembeddable font.
    const FONT_B64_START: &str = "data:font/ttf;base64,";

    /// A parsed font face whose source bytes are guaranteed to still be attached.
    ///
    /// # Why this is a newtype rather than a re-export of `azul_layout::ParsedFont`
    ///
    /// azul-layout deliberately does not retain the source bytes in
    /// `ParsedFont::from_bytes`: layout, shaping and the rasterizer never read them,
    /// and keeping them duplicated a 4.27 MiB `.ttc` once per parsed face. PDF
    /// embedding is the one consumer that *does* need them — the raw bytes literally
    /// *are* the `/FontFile2` stream, and subsetting reads the sfnt tables back out
    /// of them.
    ///
    /// So printpdf attaches them itself, via `AzulParsedFont::with_source_bytes`.
    /// Doing that here — instead of trusting whatever the linked azul-layout happens
    /// to default to — is what makes font embedding correct against *any*
    /// azul-layout version, including the published 0.0.9, whose `from_bytes` yields
    /// `original_bytes: None`. That default is exactly why every external font in
    /// printpdf 0.10.0 embedded as an empty, corrupt `/FontFile2` (issue #277).
    ///
    /// Deref gives you the full `AzulParsedFont` API (`lookup_glyph_index`,
    /// `num_glyphs`, `font_metrics`, …) unchanged.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ParsedFont(AzulParsedFont);

    impl ParsedFont {
        /// Parse a font face from raw sfnt bytes, retaining those bytes for embedding.
        ///
        /// `font_index` selects the face inside a TrueType/OpenType *collection*
        /// (`.ttc`/`.otc`); pass `0` for a plain single-face `.ttf`/`.otf`.
        pub fn from_bytes(
            bytes: &[u8],
            font_index: usize,
            warnings: &mut Vec<PdfFontParseWarning>,
        ) -> Option<Self> {
            // A bare CID-keyed CFF table (what we embed as `/FontFile3`
            // `/CIDFontType0C` — see `crate::font::extract_cid_keyed_cff`) has no
            // `head`/`hhea`/`maxp`/`hmtx` of its own, which `AzulParsedFont::from_bytes`
            // requires. Wrap it in a synthetic sfnt just to satisfy that parse.
            //
            // The face keeps the SYNTHETIC wrapper (not the original bare bytes) as
            // its retained source: azul indexes into those retained bytes by
            // byte-range (`hmtx_range` etc.) computed against whatever it actually
            // parsed, so swapping in the shorter original afterwards would leave
            // those ranges pointing past the end of the buffer. This doesn't cost us
            // round-trip fidelity — `extract_cid_keyed_cff` (the only thing that
            // reads these bytes back out for embedding) just extracts the `CFF `
            // table again, which is byte-for-byte the original bare table regardless
            // of whether the sfnt around it is real or synthesized here (#280).
            if let Some(wrapped) = crate::font::wrap_bare_cff_as_sfnt(bytes) {
                let inner = AzulParsedFont::from_bytes(&wrapped, 0, warnings)?;
                return Some(Self::attach_source_bytes(inner, &wrapped));
            }
            let inner = AzulParsedFont::from_bytes(bytes, font_index, warnings)?;
            Some(Self::attach_source_bytes(inner, bytes))
        }

        /// Adopt a face that azul already parsed (e.g. one handed back by the HTML
        /// layout font cache).
        ///
        /// The cache builds faces through `from_bytes_shared`, which keeps the source
        /// `Arc<FontBytes>` alive in its lazy loca/glyf slot, so those already satisfy
        /// the byte-retention invariant and are adopted as-is. A face that somehow
        /// arrives without bytes is returned unchanged — [`ParsedFont::has_source_bytes`]
        /// reports that, and serialization refuses to embed it rather than writing an
        /// empty font program.
        pub fn from_azul(inner: AzulParsedFont) -> Self {
            Self(inner)
        }

        /// Re-attach `bytes` unless the face already carries its source bytes.
        fn attach_source_bytes(inner: AzulParsedFont, bytes: &[u8]) -> Self {
            if inner.source_bytes_for_subset().is_some() {
                return Self(inner);
            }
            Self(inner.with_source_bytes(Arc::new(rust_fontconfig::FontBytes::Owned(
                Arc::from(bytes.to_vec()),
            ))))
        }

        /// The sfnt bytes this face was parsed from, if still available.
        ///
        /// Always `Some` for faces built by [`ParsedFont::from_bytes`]. Checks both
        /// places azul may keep them: the explicit `original_bytes` slot and the lazy
        /// loca/glyf slot used by the font cache.
        pub fn source_bytes(&self) -> Option<Arc<rust_fontconfig::FontBytes>> {
            self.0.source_bytes_for_subset()
        }

        /// Whether this face can still be embedded into a PDF.
        pub fn has_source_bytes(&self) -> bool {
            self.source_bytes().is_some()
        }

        /// Borrow the underlying azul face.
        pub fn as_azul(&self) -> &AzulParsedFont {
            &self.0
        }

        /// Unwrap to the underlying azul face.
        pub fn into_inner(self) -> AzulParsedFont {
            self.0
        }
    }

    impl Deref for ParsedFont {
        type Target = AzulParsedFont;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for ParsedFont {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl From<AzulParsedFont> for ParsedFont {
        fn from(inner: AzulParsedFont) -> Self {
            Self::from_azul(inner)
        }
    }

    impl serde::Serialize for ParsedFont {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use base64::Engine;

            // Same wire format as azul, but it must not silently emit an empty font:
            // a `ParsedFont` that can't produce its bytes cannot be embedded, and a
            // caller round-tripping one through serde needs to hear about it here
            // rather than get a corrupt PDF later.
            //
            // Prefer the retained source bytes verbatim. Rebuilding via
            // `to_bytes(None)` walks a hardcoded TrueType table list, which fails on
            // CFF faces (no glyf/loca) and on the subset fonts printpdf itself
            // writes (no OS/2/NAME/POST) — i.e. documents we saved could never be
            // deserialized again. `to_bytes` stays only as a fallback for faces
            // that somehow lost their bytes.
            let encoded = match self.source_bytes() {
                Some(b) => base64::prelude::BASE64_STANDARD.encode(b.as_slice()),
                None => {
                    let bytes = self.0.to_bytes(None).map_err(|e| {
                        serde::ser::Error::custom(format!("font has no source bytes: {e}"))
                    })?;
                    base64::prelude::BASE64_STANDARD.encode(&bytes)
                }
            };
            let s = format!("{FONT_B64_START}{encoded}");
            s.serialize(serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for ParsedFont {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            use base64::Engine;

            let s = String::deserialize(deserializer)?;
            let b64 = s.strip_prefix(FONT_B64_START).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "font must be a {FONT_B64_START}… data URI, got {:.32?}",
                    s
                ))
            })?;
            let bytes = base64::prelude::BASE64_STANDARD
                .decode(b64)
                .map_err(serde::de::Error::custom)?;

            // Route through *our* `from_bytes`, not azul's: this is what re-attaches
            // the source bytes, so a deserialized font is still embeddable.
            let mut warnings = Vec::new();
            ParsedFont::from_bytes(&bytes, 0, &mut warnings).ok_or_else(|| {
                serde::de::Error::custom(format!("font deserialization error: {warnings:?}"))
            })
        }
    }
}

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
    /// hhea caret slope — the FontDescriptor's `/ItalicAngle` is derived from it.
    pub caret_slope_rise: i16,
    pub caret_slope_run: i16,
    /// OS/2 usWeightClass (100–900). `/StemV` is estimated from it. 0 when OS/2 is absent.
    pub us_weight_class: u16,
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
            // Upright (rise 1, run 0) and "no OS/2", which /StemV falls back on.
            caret_slope_rise: 1,
            caret_slope_run: 0,
            us_weight_class: 0,
        }
    }
}

#[cfg(not(feature = "text_layout"))]
impl ParsedFont {
    /// The sfnt bytes this face was parsed from, if any (mirrors the
    /// `text_layout` build's API so shared call sites compile in both
    /// configurations — the `text_layout` version returns the retained
    /// `Arc<FontBytes>`, this one clones the stub's `original_bytes`).
    pub fn source_bytes(&self) -> Option<Vec<u8>> {
        (!self.original_bytes.is_empty()).then(|| self.original_bytes.clone())
    }

    /// Whether this face can still be embedded into a PDF.
    pub fn has_source_bytes(&self) -> bool {
        !self.original_bytes.is_empty()
    }

    /// Parse a font face from raw sfnt bytes.
    ///
    /// Same signature as the `text_layout` build (#260), so calling code is portable
    /// across the feature.
    ///
    /// This used to return a shell: `codepoint_to_glyph` was left **empty**, so
    /// `lookup_glyph_index` returned `None` for every character, every glyph in the
    /// content stream came out as `.notdef`, and no text rendered at all — which is
    /// issue #258 ("external fonts + `default-features = false` show no text").
    /// `glyph_widths` and the metrics were empty/hardcoded for the same reason.
    ///
    /// It parses the font properly now. `allsorts` is a non-optional dependency, so it is
    /// available in every build, `text_layout` or not — there was never a reason to guess.
    pub fn from_bytes(
        bytes: &[u8],
        index: usize,
        warnings: &mut Vec<PdfFontParseWarning>,
    ) -> Option<Self> {
        use allsorts::{
            binary::read::ReadScope,
            font_data::FontData,
            tables::{cmap::Cmap, FontTableProvider, HeadTable, HheaTable, HmtxTable, MaxpTable},
            tag,
        };

        let mut warn = |msg: &str| {
            warnings.push(PdfFontParseWarning {
                severity: FontParseSeverity::Warning,
                message: msg.to_string(),
            })
        };

        let font_file = ReadScope::new(bytes).read::<FontData<'_>>().ok()?;
        let provider = font_file.table_provider(index).ok()?;

        let head = provider
            .read_table_data(tag::HEAD)
            .ok()
            .and_then(|d| ReadScope::new(&d).read::<HeadTable>().ok())?;
        let maxp = provider
            .read_table_data(tag::MAXP)
            .ok()
            .and_then(|d| ReadScope::new(&d).read::<MaxpTable>().ok())?;
        let hhea = provider
            .read_table_data(tag::HHEA)
            .ok()
            .and_then(|d| ReadScope::new(&d).read::<HheaTable>().ok())?;

        // Character -> glyph. Without this every glyph id is 0 and the page is blank.
        // (The table data has to outlive the parsed view that borrows it.)
        let mut codepoint_to_glyph = BTreeMap::new();
        let cmap_data = provider.read_table_data(tag::CMAP).ok();
        match cmap_data
            .as_deref()
            .and_then(|d| ReadScope::new(d).read::<Cmap<'_>>().ok())
            .and_then(|cmap| allsorts::font::read_cmap_subtable(&cmap).ok().flatten())
        {
            Some((_encoding, subtable)) => {
                let _ = subtable.mappings_fn(|cp, gid| {
                    codepoint_to_glyph.insert(cp, gid);
                });
            }
            None => warn("font has no usable Unicode cmap subtable; text will not map to glyphs"),
        }

        // Advance widths, in font units. `/W` scales these to 1/1000 em at write time.
        let mut glyph_widths = BTreeMap::new();
        let hmtx_data = provider.read_table_data(tag::HMTX).ok();
        match hmtx_data.as_deref().and_then(|d| {
            ReadScope::new(d)
                .read_dep::<HmtxTable<'_>>((
                    usize::from(maxp.num_glyphs),
                    usize::from(hhea.num_h_metrics),
                ))
                .ok()
        }) {
            Some(hmtx) => {
                for gid in 0..maxp.num_glyphs {
                    if let Ok(advance) = hmtx.horizontal_advance(gid) {
                        glyph_widths.insert(gid, advance);
                    }
                }
            }
            None => warn("font has no hmtx table; glyph advances will be zero"),
        }

        // CFF outlines live in a `CFF ` table; TrueType outlines in `glyf`.
        let font_type = if provider.has_table(tag::CFF) {
            FontType::OpenTypeCFF(())
        } else {
            FontType::TrueType
        };

        // OS/2 usWeightClass drives the FontDescriptor's /StemV estimate. The table is
        // optional (0 means "absent", and /StemV falls back to a constant).
        let os2_data = provider.read_table_data(tag::OS_2).ok();
        let us_weight_class = os2_data
            .as_deref()
            .and_then(|d| {
                ReadScope::new(d)
                    .read_dep::<allsorts::tables::os2::Os2>(d.len())
                    .ok()
            })
            .map(|os2| os2.us_weight_class)
            .unwrap_or(0);

        Some(ParsedFont {
            original_bytes: bytes.to_vec(),
            font_index: index as u32,
            font_name: None,
            codepoint_to_glyph,
            glyph_widths,
            units_per_em: head.units_per_em,
            font_metrics: FontMetrics {
                ascent: hhea.ascender,
                descent: hhea.descender,
            },
            font_type,
            pdf_font_metrics: PdfFontMetricsStub {
                units_per_em: head.units_per_em,
                x_min: head.x_min,
                y_min: head.y_min,
                x_max: head.x_max,
                y_max: head.y_max,
                caret_slope_rise: hhea.caret_slope_rise,
                caret_slope_run: hhea.caret_slope_run,
                us_weight_class,
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

/// How serious a font-parse diagnostic is.
#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontParseSeverity {
    Info,
    Warning,
    Error,
}

/// Stand-in for azul-layout's `FontParseWarning` when `text_layout` is off.
///
/// This used to be `type FontParseWarning = String`, which meant
/// `ParsedFont::from_bytes` really did take a different type with and without the feature
/// (#260): code that read `w.message` compiled with `text_layout` and failed without it.
/// Mirroring the real shape keeps the signature — and the calling code — identical either
/// way.
#[cfg(not(feature = "text_layout"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontParseWarning {
    pub severity: FontParseSeverity,
    pub message: String,
}

#[cfg(not(feature = "text_layout"))]
pub type PdfFontParseWarning = FontParseWarning;

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
    pub glyph_mapping: BTreeMap<u16, (u16, String)>,
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
                        Some((*old_gid, (*new_gid, char.to_string())))
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
pub fn subset_font(font: &ParsedFont, glyph_ids: &BTreeMap<u16, String>) -> Result<SubsetFont, String> {
    use allsorts::{binary::read::ReadScope, font_data::FontData, subset::CmapTarget};

    // Subsetting reads the sfnt tables straight out of the source bytes. `source_bytes`
    // finds them wherever azul kept them (the explicit slot, or the font cache's lazy
    // loca/glyf slot); `ParsedFont::from_bytes` guarantees they're there at all.
    let original_bytes = font
        .source_bytes()
        .ok_or_else(|| "ParsedFont has no source bytes to subset".to_string())?;
    let scope = ReadScope::new(original_bytes.as_slice());
    let font_file = scope.read::<FontData<'_>>().map_err(|e| e.to_string())?;
    let provider = font_file
        .table_provider(font.original_index)
        .map_err(|e| e.to_string())?;

    // Glyph 0 (.notdef) must be the first entry, and there must be no duplicates —
    // allsorts documents both as hard requirements of `subset()`.
    //
    // Omitting it does not fail loudly: allsorts happily renumbers, and the *first used
    // glyph* lands in slot 0. The subset then has no .notdef at all, and its cmap maps a
    // real character to glyph 0 — which every reader treats as "missing glyph". Roboto's
    // "Roboto" subsetted to `R->0, b->1, o->2, t->3`, so the R was drawn as .notdef.
    // (BTreeMap keys are already sorted and unique.)
    let ids: Vec<u16> = std::iter::once(0)
        .chain(glyph_ids.keys().copied().filter(|gid| *gid != 0))
        .collect();

    // Use SubsetProfile::Pdf for PDF embedding and CmapTarget::Unicode for Unicode cmap.
    //
    // (Up to allsorts-azul 0.16.5, a used-glyph set with no cmap-reachable glyph did not
    // just produce an empty subset cmap, it *panicked* — `CmapSubtableFormat4::
    // from_mappings` did `mappings.iter().next().unwrap()` under a "safe as mappings is
    // non-empty" comment. It isn't. 0.17 handles it, and an empty subset cmap is fine for
    // us: rendering goes through Identity-H glyph ids and text extraction through
    // /ToUnicode + /ActualText, neither of which consults the font's own cmap.)
    let bytes = allsorts::subset::subset(
        &provider,
        &ids,
        &allsorts::subset::SubsetProfile::Pdf,
        CmapTarget::Unicode,
    ).map_err(|e| e.to_string())?;

    // Build the old->new glyph id mapping. allsorts renumbers every requested glyph to
    // its POSITION in the `ids` slice: the glyf subsetter builds `old_to_new_id` from
    // `records.iter().enumerate()` (tables/glyf/subset.rs) and the CFF subsetter pushes
    // charstrings in input order (cff/subset.rs), so subset gid i == ids[i] for both
    // outline formats. Component glyphs of composites are *appended after* the requested
    // ids, so the requested positions are stable.
    //
    // This input-order mapping is authoritative. It must NOT be recovered by looking
    // characters up in the subset's cmap (as an earlier revision did): glyphs the shaper
    // substituted have no cmap entry of their own — ligatures (the "fi" in "Configure"),
    // and alternate forms where the font's cmap points a codepoint at a *different*
    // default glyph (Noto CJK maps ASCII digits to a different digit form than the
    // shaper picks for list markers / page numbers). Those glyphs fell out of the
    // cmap-recovered remap, the content stream then emitted gid 0 for them, and every
    // list marker rendered as a .notdef box (#220 F2b) while "Configure" lost its
    // ligature (#220 F5).
    let glyph_mapping: BTreeMap<u16, (u16, String)> = ids
        .iter()
        .enumerate()
        .filter_map(|(idx, &original_gid)| {
            glyph_ids.get(&original_gid).map(|ch| {
                // New GID = position in `ids` (position 0 is .notdef).
                // `ch` is the full extraction text for the glyph — possibly
                // several chars (ligatures like "fi"); ToUnicode keeps all of
                // them (multi-char targets were truncated to the first char
                // before, so copy-pasting "Configure" yielded "Confgure").
                (original_gid, (idx as u16, ch.clone()))
            })
        })
        .collect();

    Ok(SubsetFont {
        bytes,
        glyph_mapping,
    })
}

#[cfg(not(feature = "text_layout"))]
pub fn subset_font(font: &ParsedFont, _glyph_ids: &BTreeMap<u16, String>) -> Result<SubsetFont, String> {
    Ok(SubsetFont {
        // Without text_layout, just return the original font bytes without subsetting
        bytes: font.original_bytes.clone(),
        // Empty mapping - user provides glyph info via Codepoint
        glyph_mapping: BTreeMap::new(),
    })
}

// PDF-specific helper functions for ParsedFont

pub fn generate_cmap_string(_font: &ParsedFont, font_id: &FontId, glyph_ids: &[(u16, String)]) -> String {
    let mappings = glyph_ids
        .iter()
        .map(|(gid, unicode)| {
            (*gid as u32, unicode.chars().map(|c| c as u32).collect())
        })
        .collect();

    let cmap = crate::cmap::ToUnicodeCMap { mappings };
    cmap.to_cmap_string(&font_id.0)
}

/// Repackage one face of a TrueType/OpenType *collection* (`ttcf`) as a standalone
/// sfnt, so it can be embedded as a valid `/FontFile2`/`/FontFile3` program.
///
/// A PDF font program must be a single face: embedding the whole collection produces
/// a stream no conforming reader accepts (and the descendant subtype is decided from
/// the outer magic, which for a collection is neither `OTTO` nor `\0\1\0\0`, so the
/// dictionary would mislabel it too). Returns `None` when `bytes` is not a collection
/// (nothing to do) or the face cannot be extracted.
pub fn extract_collection_face(bytes: &[u8], index: usize) -> Option<Vec<u8>> {
    use allsorts::{
        binary::read::ReadScope, font_data::FontData, subset::whole_font,
        tables::FontTableProvider,
    };

    if bytes.get(..4) != Some(b"ttcf") {
        return None;
    }
    let font_file = ReadScope::new(bytes).read::<FontData<'_>>().ok()?;
    let provider = font_file.table_provider(index).ok()?;
    let tags = provider.table_tags()?;
    whole_font(&provider, &tags).ok()
}

/// Glyph id -> CID map read from the CFF charset of the font program that is about to be
/// embedded, or `None` when the codes in the content stream can stay glyph ids.
///
/// Under `/Encoding /Identity-H` the two-byte codes in the content stream ARE the CIDs.
/// How a viewer turns a CID into a glyph depends on the descendant font:
///
/// - `CIDFontType2` (TrueType `glyf`): via `/CIDToGIDMap`, which we leave at the default
///   `/Identity` — so CID == GID and the codes are glyph ids. `None`.
/// - `CIDFontType0`, name-keyed CFF: the CID is used as the glyph index directly. `None`.
/// - `CIDFontType0`, **CID-keyed** CFF: the viewer maps CID -> GID through the CFF
///   charset (ISO 32000-1, 9.7.4.2). The charset is NOT identity in real fonts —
///   NotoSansJP diverges from glyph 365 on — and the allsorts subsetter preserves the
///   *original* CIDs in the subset charset. Emitting glyph ids here made Acrobat and
///   Preview pick wrong glyphs while PDFium-based viewers (which fall back to
///   CID == GID) looked fine (#280). The content stream, `/W` and `/ToUnicode` must all
///   be keyed by these CIDs instead.
pub fn cff_charset_gid_to_cid_map(font_bytes: &[u8], index: usize) -> Option<BTreeMap<u16, u16>> {
    use allsorts::{
        binary::read::ReadScope, cff::CFF, font_data::FontData, tables::FontTableProvider, tag,
    };

    // Accept both a whole sfnt (extract the `CFF ` table) and a bare `CFF ` table
    // — the latter is what we now embed for a CID-keyed CFF (see
    // `extract_cid_keyed_cff`), and what we read back on parse.
    let cff_data: Vec<u8> = (|| -> Option<Vec<u8>> {
        let font_file = ReadScope::new(font_bytes).read::<FontData<'_>>().ok()?;
        let provider = font_file.table_provider(index).ok()?;
        Some(provider.read_table_data(tag::CFF).ok()?.into_owned())
    })()
    .unwrap_or_else(|| font_bytes.to_vec());
    let cff = ReadScope::new(&cff_data).read::<CFF<'_>>().ok()?;
    let font = cff.fonts.first()?;
    if !font.is_cid_keyed() {
        return None;
    }
    let num_glyphs = font.char_strings_index.len() as u16;
    Some(
        (0..num_glyphs)
            .filter_map(|gid| font.charset.id_for_glyph(gid).map(|cid| (gid, cid)))
            .collect(),
    )
}

/// The bare `CFF ` table of an `OTTO` sfnt, extracted for embedding as
/// `/FontFile3` `/Subtype /CIDFontType0C` — but only when that CFF is CID-keyed.
/// `None` for TrueType faces and for name-keyed CFFs, which keep their current
/// wrappers (code == glyph id is already correct in every viewer there).
///
/// The wrapper decides which resolution semantics FreeType-based viewers apply
/// to the charset-CID codes we emit (see `cff_charset_gid_to_cid_map`). For a
/// whole OTTO sfnt (`/Subtype /OpenType`) FreeType does not flag the face as
/// CID-keyed, so PDFium (Chrome) and poppler use the Identity-H codes directly
/// as glyph indices and never consult the charset — while Acrobat and Preview
/// resolve the same codes *through* the charset. With a non-identity charset no
/// code assignment renders identically in both families: emitting glyph ids
/// broke Acrobat/Preview (#280), emitting charset CIDs breaks Chrome/poppler
/// (codes above the glyph count draw nothing at all; NotoSansJP puts 日 at CID
/// 20616 in a 17,802-glyph font). For a *bare* CID-keyed CFF, FreeType sets
/// FT_FACE_FLAG_CID_KEYED and both families resolve through the charset, so the
/// CIDs render the same everywhere — which is what the pre-rewrite embedding
/// from #215 relied on, and why 0.9.x was correct in Acrobat AND Chrome.
///
/// The returned bytes are the verbatim table `cff_charset_gid_to_cid_map` read
/// the charset from, so the emitted codes stay consistent with the embedded
/// program by construction.
pub fn extract_cid_keyed_cff(font_bytes: &[u8], index: usize) -> Option<Vec<u8>> {
    use allsorts::{
        binary::read::ReadScope, cff::CFF, font_data::FontData, tables::FontTableProvider, tag,
    };

    if !font_bytes.starts_with(b"OTTO") {
        return None;
    }
    let font_file = ReadScope::new(font_bytes).read::<FontData<'_>>().ok()?;
    let provider = font_file.table_provider(index).ok()?;
    let cff_data = provider.read_table_data(tag::CFF).ok()?;
    let cff = ReadScope::new(&cff_data).read::<CFF<'_>>().ok()?;
    if !cff.fonts.first()?.is_cid_keyed() {
        return None;
    }
    Some(cff_data.into_owned())
}

/// Wrap a bare `CFF ` table (as embedded by [`extract_cid_keyed_cff`]) in a
/// minimal `OTTO` sfnt, so it can be handed to `AzulParsedFont::from_bytes` —
/// which requires `head`/`hhea`/`maxp`/`hmtx`, tables a bare CFF byte stream
/// doesn't carry. Used only to reconstruct an in-memory parsed font when
/// *reading back* our own bare-CFF `/FontFile3` embedding (see
/// [`ParsedFont::from_bytes`]); the bytes actually written to a PDF stay
/// bare — wrapping them there would defeat the FreeType CID-keyed detection
/// `extract_cid_keyed_cff` exists for (#280).
///
/// `None` if `bytes` already looks like an sfnt, or doesn't parse as CFF.
#[cfg(feature = "text_layout")]
pub(crate) fn wrap_bare_cff_as_sfnt(bytes: &[u8]) -> Option<Vec<u8>> {
    use allsorts::{binary::read::ReadScope, cff::CFF};

    let looks_like_sfnt = matches!(bytes.get(..4), Some(magic) if magic == b"OTTO"
        || magic == [0, 1, 0, 0]
        || magic == *b"true"
        || magic == *b"typ1"
        || magic == *b"ttcf"
        || magic == *b"wOFF"
        || magic == *b"wOF2");
    if looks_like_sfnt {
        return None;
    }

    let cff = ReadScope::new(bytes).read::<CFF<'_>>().ok()?;
    let font = cff.fonts.first()?;
    let num_glyphs = u16::try_from(font.char_strings_index.len()).ok()?;
    let widths: Vec<u16> = (0..num_glyphs)
        .map(|gid| cff_charstring_width(&cff, font, gid))
        .collect();

    Some(build_minimal_sfnt(bytes, num_glyphs, &widths))
}

/// A glyph's advance width, read directly from its Type2 charstring —
/// `defaultWidthX`/`nominalWidthX` plus the optional leading width delta
/// before the first stack-clearing operator (Adobe TN5177 §16). A bare CFF
/// table has no `hmtx` of its own to read widths from instead; used by
/// [`wrap_bare_cff_as_sfnt`] to synthesize one.
#[cfg(feature = "text_layout")]
fn cff_charstring_width(
    cff: &allsorts::cff::CFF<'_>,
    font: &allsorts::cff::Font<'_>,
    gid: u16,
) -> u16 {
    use allsorts::cff::{CFFVariant, Operator};

    let charstring = font.char_strings_index.read_object(usize::from(gid));

    let (nominal_width, default_width, local_subrs) = match &font.data {
        CFFVariant::CID(cid) => {
            let fd = cid.fd_select.font_dict_index(gid).unwrap_or(0) as usize;
            let pd = cid.private_dicts.get(fd);
            (
                pd.and_then(|d| d.get_i32(Operator::NominalWidthX))
                    .and_then(Result::ok)
                    .unwrap_or(0),
                pd.and_then(|d| d.get_i32(Operator::DefaultWidthX))
                    .and_then(Result::ok)
                    .unwrap_or(0),
                cid.local_subr_indices.get(fd).and_then(|s| s.as_ref()),
            )
        }
        CFFVariant::Type1(t1) => (
            t1.private_dict
                .get_i32(Operator::NominalWidthX)
                .and_then(Result::ok)
                .unwrap_or(0),
            t1.private_dict
                .get_i32(Operator::DefaultWidthX)
                .and_then(Result::ok)
                .unwrap_or(0),
            t1.local_subr_index.as_ref(),
        ),
    };

    let Some(charstring) = charstring else {
        return default_width.max(0) as u16;
    };

    let mut stack: Vec<f64> = Vec::new();
    let width = match cff_charstring_width_delta(
        charstring,
        local_subrs,
        &cff.global_subr_index,
        &mut stack,
        0,
    ) {
        Some(delta) => nominal_width as f64 + delta,
        None => default_width as f64,
    };
    width.round().clamp(0.0, u16::MAX as f64) as u16
}

/// The standard CFF local/global subroutine index bias (Adobe TN5177 §16).
#[cfg(feature = "text_layout")]
fn cff_subr_bias(count: usize) -> i32 {
    if count < 1240 {
        107
    } else if count < 33900 {
        1131
    } else {
        32768
    }
}

/// Scan a Type2 charstring (following `callsubr`/`callgsubr`, depth-limited
/// like allsorts' own interpreter) up to its first stack-clearing operator,
/// returning the leading width delta if the operand count proves one is
/// present. `stack` is threaded through subroutine calls so operands a
/// subroutine leaves behind are visible to its caller, matching how a real
/// Type2 interpreter treats `callsubr`/`callgsubr` as inlining.
#[cfg(feature = "text_layout")]
fn cff_charstring_width_delta(
    charstring: &[u8],
    local_subrs: Option<&allsorts::cff::MaybeOwnedIndex<'_>>,
    global_subrs: &allsorts::cff::MaybeOwnedIndex<'_>,
    stack: &mut Vec<f64>,
    depth: u8,
) -> Option<f64> {
    if depth > 10 {
        return None;
    }
    let mut i = 0usize;
    while i < charstring.len() {
        let b0 = charstring[i];
        match b0 {
            1 | 3 | 18 | 19 | 20 | 23 => {
                // hstem/vstem/hstemhm/hintmask/cntrmask/vstemhm: width present iff
                // the operand count is odd.
                return (stack.len() % 2 == 1).then(|| stack[0]);
            }
            21 => return (stack.len() > 2).then(|| stack[0]), // rmoveto: 2 args
            22 | 4 => return (stack.len() > 1).then(|| stack[0]), // h/vmoveto: 1 arg
            14 => return (stack.len() == 1 || stack.len() == 5).then(|| stack[0]), // endchar
            10 | 29 => {
                // callsubr / callgsubr
                let Some(idx) = stack.pop() else { return None };
                let (subrs, bias) = if b0 == 10 {
                    let subrs = local_subrs?;
                    (subrs, cff_subr_bias(subrs.len()))
                } else {
                    (global_subrs, cff_subr_bias(global_subrs.len()))
                };
                let subr_idx = idx as i32 + bias;
                let sub_bytes = usize::try_from(subr_idx)
                    .ok()
                    .and_then(|si| subrs.read_object(si))?;
                if let Some(w) =
                    cff_charstring_width_delta(sub_bytes, local_subrs, global_subrs, stack, depth + 1)
                {
                    return Some(w);
                }
                i += 1;
            }
            11 => return None, // return: no stack-clearing op seen in this chain
            28 => {
                if i + 3 > charstring.len() {
                    return None;
                }
                let v = i16::from_be_bytes([charstring[i + 1], charstring[i + 2]]);
                stack.push(f64::from(v));
                i += 3;
            }
            32..=246 => {
                stack.push(f64::from(b0) - 139.0);
                i += 1;
            }
            247..=250 => {
                if i + 2 > charstring.len() {
                    return None;
                }
                stack.push((f64::from(b0) - 247.0) * 256.0 + f64::from(charstring[i + 1]) + 108.0);
                i += 2;
            }
            251..=254 => {
                if i + 2 > charstring.len() {
                    return None;
                }
                stack.push(-(f64::from(b0) - 251.0) * 256.0 - f64::from(charstring[i + 1]) - 108.0);
                i += 2;
            }
            255 => {
                if i + 5 > charstring.len() {
                    return None;
                }
                let bits = i32::from_be_bytes([
                    charstring[i + 1],
                    charstring[i + 2],
                    charstring[i + 3],
                    charstring[i + 4],
                ]);
                stack.push(f64::from(bits) / 65536.0);
                i += 5;
            }
            _ => return None,
        }
    }
    None
}

/// Assemble a minimal `OTTO` sfnt around a bare `CFF ` table, synthesizing
/// the `head`/`hhea`/`maxp`/`hmtx` tables a bare CFF doesn't carry. See
/// [`wrap_bare_cff_as_sfnt`].
#[cfg(feature = "text_layout")]
fn build_minimal_sfnt(cff_bytes: &[u8], num_glyphs: u16, widths: &[u16]) -> Vec<u8> {
    const UNITS_PER_EM: u16 = 1000;

    let mut head = Vec::with_capacity(54);
    head.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
    head.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    head.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // fontRevision
    head.extend_from_slice(&0u32.to_be_bytes()); // checkSumAdjustment
    head.extend_from_slice(&0x5F0F_3CF5u32.to_be_bytes()); // magicNumber
    head.extend_from_slice(&0u16.to_be_bytes()); // flags
    head.extend_from_slice(&UNITS_PER_EM.to_be_bytes());
    head.extend_from_slice(&0i64.to_be_bytes()); // created
    head.extend_from_slice(&0i64.to_be_bytes()); // modified
    head.extend_from_slice(&0i16.to_be_bytes()); // xMin
    head.extend_from_slice(&0i16.to_be_bytes()); // yMin
    head.extend_from_slice(&0i16.to_be_bytes()); // xMax
    head.extend_from_slice(&0i16.to_be_bytes()); // yMax
    head.extend_from_slice(&0u16.to_be_bytes()); // macStyle
    head.extend_from_slice(&0u16.to_be_bytes()); // lowestRecPPEM
    head.extend_from_slice(&2i16.to_be_bytes()); // fontDirectionHint
    head.extend_from_slice(&0i16.to_be_bytes()); // indexToLocFormat
    head.extend_from_slice(&0i16.to_be_bytes()); // glyphDataFormat

    let advance_width_max = widths.iter().copied().max().unwrap_or(0);
    let mut hhea = Vec::with_capacity(36);
    hhea.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
    hhea.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    hhea.extend_from_slice(&(UNITS_PER_EM as i16 * 8 / 10).to_be_bytes()); // ascender
    hhea.extend_from_slice(&(-(UNITS_PER_EM as i16) * 2 / 10).to_be_bytes()); // descender
    hhea.extend_from_slice(&0i16.to_be_bytes()); // lineGap
    hhea.extend_from_slice(&advance_width_max.to_be_bytes());
    hhea.extend_from_slice(&0i16.to_be_bytes()); // minLeftSideBearing
    hhea.extend_from_slice(&0i16.to_be_bytes()); // minRightSideBearing
    hhea.extend_from_slice(&0i16.to_be_bytes()); // xMaxExtent
    hhea.extend_from_slice(&1i16.to_be_bytes()); // caretSlopeRise
    hhea.extend_from_slice(&0i16.to_be_bytes()); // caretSlopeRun
    hhea.extend_from_slice(&0i16.to_be_bytes()); // caretOffset
    hhea.extend_from_slice(&0i16.to_be_bytes()); // reserved x4
    hhea.extend_from_slice(&0i16.to_be_bytes());
    hhea.extend_from_slice(&0i16.to_be_bytes());
    hhea.extend_from_slice(&0i16.to_be_bytes());
    hhea.extend_from_slice(&0i16.to_be_bytes()); // metricDataFormat
    hhea.extend_from_slice(&num_glyphs.to_be_bytes()); // numberOfHMetrics

    let mut maxp = Vec::with_capacity(6);
    maxp.extend_from_slice(&0x0000_5000u32.to_be_bytes()); // version 0.5 (CFF)
    maxp.extend_from_slice(&num_glyphs.to_be_bytes());

    let mut hmtx = Vec::with_capacity(widths.len() * 4);
    for &w in widths {
        hmtx.extend_from_slice(&w.to_be_bytes());
        hmtx.extend_from_slice(&0i16.to_be_bytes()); // lsb
    }

    // `allsorts::font::Font::new` (built internally by `AzulParsedFont::from_bytes`)
    // hard-requires a `cmap` with a subtable one of the recognised platform/encoding
    // pairs finds (`find_good_cmap_subtable`) — a format 6 trimmed table under
    // Windows/Unicode-BMP satisfies that. A bare CFF genuinely carries no
    // char->glyph mapping (that's what makes it bare), so this must answer "no
    // mapping" for every codepoint, not just placeholder-map everything to
    // .notdef: an empty format 6 table (`entry_count = 0`) does that — every
    // lookup falls outside its range and `map_glyph` returns `None`, rather than
    // the `Some(0)` a zero-filled format 0 table would return (indistinguishable
    // from a genuine, wrong .notdef mapping to callers like
    // `ParsedFont::lookup_glyph_index`).
    let mut cmap_subtable = Vec::with_capacity(10);
    cmap_subtable.extend_from_slice(&6u16.to_be_bytes()); // format 6
    cmap_subtable.extend_from_slice(&10u16.to_be_bytes()); // length
    cmap_subtable.extend_from_slice(&0u16.to_be_bytes()); // language
    cmap_subtable.extend_from_slice(&0u16.to_be_bytes()); // firstCode
    cmap_subtable.extend_from_slice(&0u16.to_be_bytes()); // entryCount

    let mut cmap = Vec::with_capacity(12 + cmap_subtable.len());
    cmap.extend_from_slice(&0u16.to_be_bytes()); // version
    cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables
    cmap.extend_from_slice(&3u16.to_be_bytes()); // platformID: Windows
    cmap.extend_from_slice(&1u16.to_be_bytes()); // encodingID: Unicode BMP
    cmap.extend_from_slice(&12u32.to_be_bytes()); // subtable offset
    cmap.extend_from_slice(&cmap_subtable);

    build_sfnt(&[
        (*b"CFF ", cff_bytes),
        (*b"cmap", &cmap),
        (*b"head", &head),
        (*b"hhea", &hhea),
        (*b"hmtx", &hmtx),
        (*b"maxp", &maxp),
    ])
}

/// Assemble an `OTTO` sfnt from `(tag, data)` tables, already in ascending-tag
/// order (required by the sfnt table directory). Computes each table's
/// checksum per the standard sfnt algorithm (sum of big-endian `u32` words,
/// zero-padded to a 4-byte boundary).
#[cfg(feature = "text_layout")]
fn build_sfnt(tables: &[([u8; 4], &[u8])]) -> Vec<u8> {
    fn checksum(data: &[u8]) -> u32 {
        let mut sum: u32 = 0;
        for chunk in data.chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            sum = sum.wrapping_add(u32::from_be_bytes(word));
        }
        sum
    }
    fn padded_len(len: usize) -> usize {
        (len + 3) & !3
    }

    let num_tables = tables.len() as u16;
    let mut search_range_pow2 = 1u16;
    let mut entry_selector = 0u16;
    while search_range_pow2 * 2 <= num_tables {
        search_range_pow2 *= 2;
        entry_selector += 1;
    }
    let search_range = search_range_pow2 * 16;
    let range_shift = num_tables * 16 - search_range;

    let mut offset = 12 + 16 * tables.len();
    let mut directory = Vec::with_capacity(16 * tables.len());
    let mut data_section = Vec::new();
    for (tag, data) in tables {
        directory.extend_from_slice(tag);
        directory.extend_from_slice(&checksum(data).to_be_bytes());
        directory.extend_from_slice(&(offset as u32).to_be_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_be_bytes());

        data_section.extend_from_slice(data);
        data_section.resize(data_section.len() + (padded_len(data.len()) - data.len()), 0);
        offset += padded_len(data.len());
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(b"OTTO");
    out.extend_from_slice(&num_tables.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    out.extend_from_slice(&directory);
    out.extend_from_slice(&data_section);
    out
}

#[cfg(feature = "text_layout")]
fn get_glyph_width(font: &ParsedFont, gid: u16) -> Option<u16> {
    // `glyph_records_decoded` was replaced by the lazy `get_or_decode_glyph`.
    font.get_or_decode_glyph(gid).map(|g| g.horz_advance)
}

#[cfg(feature = "text_layout")]
pub fn get_normalized_widths_ttf(font: &ParsedFont, glyph_ids: &[(u16, String)]) -> Vec<lopdf::Object> {
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

/// Build the `/W` array for entries of `(content-stream code, gid to fetch the width of)`.
///
/// The code is the CID under Identity-H: equal to the gid for TrueType and name-keyed CFF
/// fonts, but mapped through the CFF charset for CID-keyed CFF fonts (see
/// [`cff_charset_gid_to_cid_map`]). `entries` must be sorted ascending by code — the
/// run-length groups (`c [w1 w2 ...]`) depend on it.
#[cfg(feature = "text_layout")]
pub fn get_normalized_widths_codes(
    font: &ParsedFont,
    entries: &[(u16, u16)],
) -> Vec<lopdf::Object> {
    let percentage_font_scaling = 1000.0 / (font.pdf_font_metrics.units_per_em as f32);

    let mut widths_list = Vec::new();
    let mut current_low_code = 0u16;
    let mut current_high_code = 0u16;
    let mut current_width_vec: Vec<i64> = Vec::new();

    for &(code, gid) in entries {
        let glyph_width = get_glyph_width(font, gid)
            .map(|w| (w as f32 * percentage_font_scaling) as i64)
            .unwrap_or(0);

        if current_width_vec.is_empty() {
            current_low_code = code;
            current_high_code = code;
            current_width_vec.push(glyph_width);
        } else if code == current_high_code + 1 {
            current_high_code = code;
            current_width_vec.push(glyph_width);
        } else {
            widths_list.push(lopdf::Object::Integer(current_low_code as i64));
            widths_list.push(lopdf::Object::Array(
                current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
            ));
            current_low_code = code;
            current_high_code = code;
            current_width_vec = vec![glyph_width];
        }
    }

    if !current_width_vec.is_empty() {
        widths_list.push(lopdf::Object::Integer(current_low_code as i64));
        widths_list.push(lopdf::Object::Array(
            current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
        ));
    }

    widths_list
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

    // Not a unit test: this REGENERATES `defaultfonts/*.subset.ttf` + the `FONTS`
    // mapping table (the "subsetting example"). Ignored so CI doesn't rewrite
    // committed assets / fail; run manually with `--ignored` to regenerate.
    #[test]
    #[ignore = "regenerates bundled subset fonts + FONTS table; run manually with --ignored"]
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
            let (subset_bytes, glyph_mapping) = font.subset(&glyph_ids, azul_layout::CmapTarget::Unicode).unwrap();
            let glyph_mapping = glyph_mapping
                .into_iter()
                .map(|(k, (g, c))| (k, (g, c.to_string())))
                .collect();
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
                // glyph_mapping values are Strings now (ligatures carry several
                // chars); the generated FONTS table wants one char per builtin
                // glyph, which is always the case for the WIN_1252 charmap.
                let ch = char.chars().next().unwrap_or('\u{FFFD}');
                target_map.push(format!(
                    "    ({}, {old_gid}, {new_gid}, '{c}'),",
                    name.get_num(),
                    c = if ch == '\'' {
                        "\\'".to_string()
                    } else if ch == '\\' {
                        "\\\\".to_string()
                    } else {
                        ch.to_string()
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

