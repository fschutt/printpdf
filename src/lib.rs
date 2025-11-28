//! `printpdf` PDF library, second API iteration version

use std::collections::BTreeMap;

use serde_derive::{Deserialize, Serialize};

/// Bookmarks, page and link annotation handling
pub mod annotation;
pub use annotation::*;
/// /ToUnicode map serialization / parsing
pub mod cmap;
pub use cmap::*;
/// Text encoding and decoding functions
pub mod text;
pub use text::*;
/// WASM API functions
pub mod wasm;
pub use wasm::*;
/// PDF conformance / PDF standards handling and validation
pub mod conformance;
pub use conformance::*;
/// Transformation and text matrices
pub mod matrix;
pub use matrix::*;
/// Typed PDF units (Pt, Mm, Px, etc.)
pub mod units;
pub use units::*;
/// Date parsing and serializiation
pub mod date;
pub use date::*;
/// Font and codepoint handling
pub mod font;
pub use font::*;
/// Text shaping, to position text manually
pub mod shape;
pub use shape::*;
/// Point / line / polygon handling
pub mod graphics;
pub use graphics::*;
/// Page operations
pub mod ops;
pub use ops::*;
/// Color handling
pub mod color;
pub use color::*;
/// XObject handling
pub mod xobject;
pub use xobject::*;
/// Image types (always available)
pub mod image_types;
pub use image_types::*;
/// SVG handling
#[cfg(feature = "svg")]
pub mod svg;
#[cfg(feature = "svg")]
pub use svg::*;
/// Image decoding (requires 'images' feature)
#[cfg(feature = "images")]
pub mod image;
#[cfg(feature = "images")]
pub use image::*;
/// HTML handling (using azul solver3 and DisplayList)
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "html")]
pub use html::*;

/// Public API for text shaping using azul text3
#[cfg(feature = "text_layout")]
pub mod text_shaping {
    pub use azul_layout::text3::cache::{FontManager, UnifiedLayout, ParsedFontTrait, LoadedFonts};
    pub use azul_layout::text3::glyphs::{get_glyph_runs_pdf, PdfGlyphRun, PdfPositionedGlyph};
    pub use azul_css::props::basic::ColorU;
    pub use crate::shape::{layout_to_ops, layout_to_ops_with_offset};
    #[cfg(feature = "html")]
    pub use crate::html::bridge::render_unified_layout_public;
}
/// HTML component definitions
#[cfg(feature = "html")]
pub mod components;
#[cfg(feature = "html")]
pub use components::*;
/// Utility functions (random strings, numbers, timestamp formatting)
pub mod utils;
use utils::*;
/// Core utils for writing PDF
pub mod serialize;
pub use serialize::*;
/// Core utils for parsing PDF
pub mod deserialize;
pub use deserialize::*;
/// Rendering PDF to SVG (always available)
pub(crate) mod render;
pub use render::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GeneratePdfOptions {
    /// Whether to embed fonts in the PDF (default: true)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_embedding: Option<bool>,
    /// Page width in mm, default 210.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_width: Option<f32>,
    /// Page height in mm, default 297.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_height: Option<f32>,
    /// Top margin in mm, default 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_top: Option<f32>,
    /// Right margin in mm, default 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_right: Option<f32>,
    /// Bottom margin in mm, default 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_bottom: Option<f32>,
    /// Left margin in mm, default 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_left: Option<f32>,
    /// Settings for automatic image optimization when saving PDF files
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_optimization: Option<ImageOptimizationOptions>,
    /// Show page numbers in footer ("Page X of Y" format)
    /// NOTE: Full CSS @page rule parsing is not yet implemented.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_page_numbers: Option<bool>,
    /// Custom header text (appears on all pages, or skipped on first if skip_first_page is set)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_text: Option<String>,
    /// Custom footer text (in addition to or instead of page numbers)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_text: Option<String>,
    /// Skip header/footer on the first page
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_first_page: Option<bool>,
}

impl Default for GeneratePdfOptions {
    fn default() -> Self {
        Self {
            font_embedding: Some(true),
            page_width: Some(210.0),
            page_height: Some(297.0),
            margin_top: None,
            margin_right: None,
            margin_bottom: None,
            margin_left: None,
            image_optimization: None,
            show_page_numbers: None,
            header_text: None,
            footer_text: None,
            skip_first_page: None,
        }
    }
}

impl GeneratePdfOptions {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Base64 is necessary because there are a lot of JS issues surrounding
/// `ArrayBuffer` / `Uint8Buffer` / `ByteArray` type mismatches, so a simple
/// `atob` / `btoa` fixes that at the cost of slight performance decrease.
///
/// Note: this enum is untagged, so from JS you can pass in either the base64 bytes
/// or the bytearray and it'll work.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Base64OrRaw {
    /// Base64 string, usually tagged with
    B64(String),
    /// Raw bytes
    Raw(Vec<u8>),
}

impl Default for Base64OrRaw {
    fn default() -> Self {
        Base64OrRaw::Raw(Vec::new())
    }
}

impl Base64OrRaw {
    // Decodes the bytes if base64 and also gets rid of the "data:...;base64," prefix
    pub fn decode_bytes(&self) -> Result<Vec<u8>, String> {
        use base64::Engine;
        match self {
            Base64OrRaw::B64(r) => base64::prelude::BASE64_STANDARD
                .decode(get_base64_substr(r))
                .map_err(|e| e.to_string()),
            Base64OrRaw::Raw(r) => Ok(r.clone()),
        }
    }
}

fn get_base64_substr(input: &str) -> &str {
    // Check if the input starts with "data:" and contains a comma.
    if input.starts_with("data:") {
        if let Some(comma_index) = input.find(',') {
            // Optionally, verify that the metadata contains "base64"
            let metadata = &input[..comma_index];
            if metadata.contains("base64") {
                // Return the portion after the comma
                &input[comma_index + 1..]
            } else {
                // If not marked as base64, assume the whole string is encoded
                input
            }
        } else {
            // No comma found; fall back to using the entire string
            input
        }
    } else {
        // Not a data URL, so use the string as-is
        input
    }
}

/// Internal ID for page annotations
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageAnnotId(pub String);

impl PageAnnotId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Internal ID for XObjects
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct XObjectId(pub String);

impl XObjectId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Internal ID for Fonts
#[derive(Debug, PartialEq, Hash, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontId(pub String);

impl FontId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Internal ID for Layers
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerInternalId(pub String);

impl LayerInternalId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Internal ID for extended graphic states
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExtendedGraphicsStateId(pub String);

impl ExtendedGraphicsStateId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Internal ID for ICC profiles
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IccProfileId(pub String);

impl IccProfileId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// Parsed PDF document
#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfDocument {
    /// Metadata about the document (author, info, XMP metadata, etc.)
    pub metadata: PdfMetadata,
    /// Resources shared between pages, such as fonts, XObjects, images, forms, ICC profiles, etc.
    pub resources: PdfResources,
    /// Document-level bookmarks (used for the outline)
    pub bookmarks: PageAnnotMap,
    /// Page contents
    pub pages: Vec<PdfPage>,
}

impl PdfDocument {

    pub fn new(name: &str) -> Self {
        Self {
            metadata: PdfMetadata {
                info: PdfDocumentInfo {
                    document_title: name.to_string(),
                    ..Default::default()
                },
                xmp: None,
            },
            resources: PdfResources::default(),
            bookmarks: PageAnnotMap::default(),
            pages: Vec::new(),
        }
    }

    /// Parses a PDF
    pub fn parse(
        bytes: &[u8],
        opts: &PdfParseOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<Self, String> {
        self::deserialize::parse_pdf_from_bytes(bytes, opts, warnings)
    }

    pub async fn parse_async(
        bytes: &[u8],
        opts: &PdfParseOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<Self, String> {
        self::deserialize::parse_pdf_from_bytes_async(bytes, opts, warnings).await
    }

    pub fn add_graphics_state(&mut self, gs: ExtendedGraphicsState) -> ExtendedGraphicsStateId {
        let id = ExtendedGraphicsStateId::new();
        self.resources.extgstates.map.insert(id.clone(), gs);
        id
    }

    pub fn add_layer(&mut self, layer: &Layer) -> LayerInternalId {
        let id = LayerInternalId::new();
        self.resources.layers.map.insert(id.clone(), layer.clone());
        id
    }

    /// Add a font from a parsed font.
    pub fn add_font(&mut self, font: &ParsedFont) -> FontId {
        let id = FontId::new();
        let pdf_font = crate::font::PdfFont::new(font.clone());
        self.resources.fonts.map.insert(id.clone(), pdf_font);
        id
    }

    /// Extract text from all pages in the document
    /// This method properly handles both TextItem::Text and TextItem::GlyphIds
    pub fn extract_text(&self) -> Vec<Vec<String>> {
        self.pages
            .iter()
            .map(|page| page.extract_text(&self.resources))
            .collect()
    }

    /// Adds an image to the internal resources
    pub fn add_image(&mut self, image: &RawImage) -> XObjectId {
        let id = XObjectId::new();
        self.resources
            .xobjects
            .map
            .insert(id.clone(), XObject::Image(image.clone()));
        id
    }

    /// Adds an external XObject stream (usually SVG or other stream) to the PDF resources
    /// so that it can be later be invoked with `UseXobject { id }`
    pub fn add_xobject(&mut self, parsed_svg: &ExternalXObject) -> XObjectId {
        let id = XObjectId::new();
        self.resources
            .xobjects
            .map
            .insert(id.clone(), XObject::External(parsed_svg.clone()));
        id
    }

    /// Adds a new page-level bookmark on page `$page`, returning the bookmarks internal ID
    pub fn add_bookmark(&mut self, name: &str, page: usize) -> PageAnnotId {
        let id = PageAnnotId::new();
        self.bookmarks.map.insert(
            id.clone(),
            PageAnnotation {
                name: name.to_string(),
                page,
            },
        );
        id
    }

    /// Renders HTML to pages (new implementation using azul solver3)
    #[cfg(feature = "html")]
    pub fn from_html(
        html: &str,
        images: &BTreeMap<String, Base64OrRaw>,
        fonts: &BTreeMap<String, Base64OrRaw>,
        options: &GeneratePdfOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<Self, String> {
        use crate::html::{XmlRenderOptions, PageMargins};
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let mut pdf = Self::new("PDF Document");

        // Convert to XmlRenderOptions
        let mut xml_options = XmlRenderOptions::default();
        xml_options.page_width = Mm(options.page_width.unwrap_or(210.0));
        xml_options.page_height = Mm(options.page_height.unwrap_or(297.0));
        
        // Apply page margins if configured
        xml_options.margins = PageMargins {
            top: Mm(options.margin_top.unwrap_or(0.0)),
            right: Mm(options.margin_right.unwrap_or(0.0)),
            bottom: Mm(options.margin_bottom.unwrap_or(0.0)),
            left: Mm(options.margin_left.unwrap_or(0.0)),
        };
        
        // Apply header/footer configuration
        // NOTE: Full CSS @page rule parsing is not yet implemented.
        // These options provide programmatic control over page decoration.
        xml_options.show_page_numbers = options.show_page_numbers.unwrap_or(false);
        xml_options.header_text = options.header_text.clone();
        xml_options.footer_text = options.footer_text.clone();
        xml_options.skip_first_page = options.skip_first_page.unwrap_or(false);
        
        // Convert images and fonts
        for (key, img) in images {
            let bytes = match img {
                Base64OrRaw::Raw(b) => b.clone(),
                Base64OrRaw::B64(s) => STANDARD.decode(s).map_err(|e| format!("Base64 decode error: {}", e))?,
            };
            xml_options.images.insert(key.clone(), bytes);
        }
        
        for (key, font) in fonts {
            let bytes = match font {
                Base64OrRaw::Raw(b) => b.clone(),
                Base64OrRaw::B64(s) => STANDARD.decode(s).map_err(|e| format!("Base64 decode error: {}", e))?,
            };
            xml_options.fonts.insert(key.clone(), bytes);
        }

        // Render XML to pages
        match crate::html::xml_to_pdf_pages(html, &xml_options) {
            Ok((pages, font_data)) => {
                // Register fonts from font_data in pdf.resources.fonts
                for (font_hash, parsed_font) in font_data.iter() {
                    // The font ID matches what the bridge generated
                    let font_id = FontId(format!("F{}", font_hash.font_hash));
                    
                    let pdf_font = crate::font::PdfFont::new(parsed_font.clone());
                    pdf.resources.fonts.map.insert(font_id, pdf_font);
                }
                
                pdf.pages.extend(pages);
                Ok(pdf)
            }
            Err(errs) => {
                warnings.extend(errs);
                Ok(pdf)
            }
        }
    }

    /// Renders a PDF Page into an SVG String. Returns `None` on an invalid page number
    /// (note: 1-indexed, so the first PDF page is "page 1", not "page 0").
    pub fn page_to_svg(
        &self,
        page: usize,
        opts: &PdfToSvgOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Option<String> {
        Some(
            self.pages
                .get(page.saturating_sub(1))?
                .to_svg(&self.resources, opts, warnings),
        )
    }

    pub async fn page_to_svg_async(
        &self,
        page: usize,
        opts: &PdfToSvgOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Option<String> {
        Some(
            self.pages
                .get(page.saturating_sub(1))?
                .to_svg_async(&self.resources, opts, warnings)
                .await,
        )
    }

    /// Replaces `document.pages` with the new pages
    pub fn with_pages(&mut self, pages: Vec<PdfPage>) -> &mut Self {
        let mut pages = pages;
        self.pages.append(&mut pages);
        self
    }
    /// Serializes the PDF document and writes it to a `writer`
    pub fn save_writer<W: std::io::Write>(
        &self,
        w: &mut W,
        opts: &PdfSaveOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) {
        self::serialize::serialize_pdf(self, opts, w, warnings);
    }
    /// Serializes the PDF document to bytes
    pub fn save(&self, opts: &PdfSaveOptions, warnings: &mut Vec<PdfWarnMsg>) -> Vec<u8> {
        self::serialize::serialize_pdf_into_bytes(self, opts, warnings)
    }

    /// Returns the lopdf Document structure for advanced manipulation
    pub fn to_lopdf_document(
        &self,
        opts: &PdfSaveOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> lopdf::Document {
        self::serialize::to_lopdf_doc(self, opts, warnings)
    }

    pub async fn save_async(
        &self,
        opts: &PdfSaveOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Vec<u8> {
        self::serialize::serialize_pdf_into_bytes(self, opts, warnings)
    }
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfResources {
    /// Fonts found in the PDF file, indexed by the sha256 of their contents
    #[serde(default)]
    pub fonts: PdfFontMap,
    /// XObjects (forms, images, embedded PDF contents, etc.)
    #[serde(default)]
    pub xobjects: XObjectMap,
    /// Map of explicit extended graphics states
    #[serde(default)]
    pub extgstates: ExtendedGraphicsStateMap,
    /// Map of optional content groups
    #[serde(default)]
    pub layers: PdfLayerMap,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PdfLayerMap {
    pub map: BTreeMap<LayerInternalId, Layer>,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PdfFontMap {
    pub map: BTreeMap<FontId, crate::font::PdfFont>,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct XObjectMap {
    pub map: BTreeMap<XObjectId, XObject>,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageAnnotMap {
    pub map: BTreeMap<PageAnnotId, PageAnnotation>,
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExtendedGraphicsStateMap {
    pub map: BTreeMap<ExtendedGraphicsStateId, ExtendedGraphicsState>,
}

/// This is a wrapper in order to keep shared data between the documents XMP metadata and
/// the "Info" dictionary in sync
#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfMetadata {
    /// Document information
    #[serde(default)]
    pub info: PdfDocumentInfo,
    /// XMP Metadata. Is ignored on save if the PDF conformance does not allow XMP
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xmp: Option<XmpMetadata>,
}

impl PdfMetadata {
    /// Consumes the XmpMetadata and turns it into a PDF Object.
    /// This is similar to the
    pub(crate) fn xmp_metadata_string(&self) -> String {
        // Shared between XmpMetadata and DocumentInfo
        let trapping = if self.info.trapped { "True" } else { "False" };

        // let xmp_instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let instance_id = random_character_string_32();
        let create_date = to_pdf_xmp_date(&self.info.creation_date);
        let modification_date = to_pdf_xmp_date(&self.info.modification_date);
        let metadata_date = to_pdf_xmp_date(&self.info.metadata_date);

        let pdf_x_version = self.info.conformance.get_identifier_string();
        let document_version = self.info.version.to_string();
        let document_id = self.info.identifier.to_string();

        let rendition_class = match self.xmp.as_ref().and_then(|s| s.rendition_class.clone()) {
            Some(class) => class,
            None => "".to_string(),
        };

        format!(
            include_str!("./res/catalog_xmp_metadata.txt"),
            create = create_date,
            modify = modification_date,
            mdate = metadata_date,
            title = self.info.document_title,
            id = document_id,
            instance = instance_id,
            class = rendition_class,
            version = document_version,
            pdfx = pdf_x_version,
            trapping = trapping,
            creator = self.info.creator,
            subject = self.info.subject,
            keywords = self.info.keywords.join(","),
            identifier = self.info.identifier,
            producer = self.info.producer
        )
    }
}

/// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
/// Right now it just fills out the necessary fields
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmpMetadata {
    /// Web-viewable or "default" or to be left empty. Usually "default".
    #[serde(default)]
    pub rendition_class: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfDocumentInfo {
    /// Is the document trapped?
    pub trapped: bool,
    /// PDF document version
    pub version: u32,
    /// Creation date of the document
    pub creation_date: OffsetDateTime,
    /// Modification date of the document
    pub modification_date: OffsetDateTime,
    /// Creation date of the metadata
    pub metadata_date: OffsetDateTime,
    /// PDF Standard
    pub conformance: PdfConformance,
    /// PDF document title
    pub document_title: String,
    /// PDF document author
    pub author: String,
    /// The creator of the document
    pub creator: String,
    /// The producer of the document
    pub producer: String,
    /// Keywords associated with the document
    pub keywords: Vec<String>,
    /// The subject of the document
    pub subject: String,
    /// Identifier associated with the document
    pub identifier: String,
}

impl Default for PdfDocumentInfo {
    fn default() -> Self {
        Self {
            trapped: false,
            version: 1,
            creation_date: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            modification_date: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            metadata_date: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            conformance: PdfConformance::default(),
            document_title: String::new(),
            author: String::new(),
            creator: String::new(),
            producer: String::new(),
            keywords: Vec::new(),
            subject: String::new(),
            identifier: String::new(),
        }
    }
}
