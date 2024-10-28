//! `printpdf` PDF library, second API iteration version

use std::collections::BTreeMap;

/// Link / bookmark annotation handling
pub mod annotation;
pub use annotation::*;
/// PDF standard handling
pub mod conformance;
pub use conformance::*;
/// Transformation and text matrices
pub mod matrix;
pub use matrix::*;
/// Units (Pt, Mm, Px, etc.)
pub mod units;
use pdf_writer::writers::ExtGraphicsState;
use serialize::SaveOptions;
pub use units::*;
/// Date handling (stubs for platforms that don't support access to time clocks, such as wasm32-unknown)
pub mod date;
pub use date::*;
/// Font and codepoint handling
pub mod font;
pub use font::*;
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
/// SVG handling
pub mod svg;
pub use svg::*;
/// Constants and library includes
pub(crate) mod constants;
/// Utility functions (random strings, numbers, timestamp formatting)
pub(crate) mod utils;
use utils::*;
/// Writing PDF
pub(crate) mod serialize;
/// Parsing PDF
pub(crate) mod deserialize;

/// Internal ID for page annotations
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct PageAnnotId(pub String);

impl PageAnnotId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for link annotations
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct LinkAnnotId(pub String);

impl LinkAnnotId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for XObjects
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct XObjectId(pub String);

impl XObjectId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for Fonts
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct FontId(pub String);

impl FontId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for Layers
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct LayerInternalId(pub String);

impl LayerInternalId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for extended graphic states
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct ExtendedGraphicsStateId(pub String);

impl ExtendedGraphicsStateId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Internal ID for ICC profiles
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord)]
pub struct IccProfileId(pub String);

impl IccProfileId {
    pub fn new() -> Self { Self(crate::utils::random_character_string_32()) }
}

/// Parsed PDF document
#[derive(Debug, PartialEq, Clone)]
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
                    .. Default::default()
                }, xmp: None 
            },
            resources: PdfResources::default(),
            bookmarks: PageAnnotMap::default(),
            pages: Vec::new(),
        }
    }

    pub fn add_graphics_state(&mut self, gs: ExtendedGraphicsState) -> ExtendedGraphicsStateId {
        let id = ExtendedGraphicsStateId::new();
        self.resources.extgstates.map.insert(id.clone(), gs);
        id
    }

    pub fn add_layer(&mut self, name: &str, creator: &str, intent: LayerIntent, usage: LayerSubtype) -> LayerInternalId {
        let id = LayerInternalId::new();
        self.resources.layers.map.insert(id.clone(), Layer {
            name: name.to_string(),
            creator: creator.to_string(),
            intent,
            usage,
        });
        id
    }

    pub fn add_font(&mut self, font: &ParsedFont) -> FontId {
        let id = FontId::new();
        self.resources.fonts.map.insert(id.clone(), font.clone());
        id
    }

    /// Adds an external XObject stream (usually SVG or other stream) to the PDF resources
    /// so that it can be later be invoked with `UseXObject { id }`
    pub fn add_xobject(&mut self, parsed_svg: &ExternalXObject) -> XObjectId {
        let id = XObjectId::new();
        self.resources.xobjects.map.insert(id.clone(), XObject::External(parsed_svg.clone()));
        id
    }

    // Adds a link (hyperlink or self-referential link) to the document resources, returning the links internal ID
    pub fn add_link(&mut self, link: LinkAnnotation) -> LinkAnnotId {
        let id = LinkAnnotId::new();
        self.resources.links.map.insert(id.clone(), link);
        id
    }

    /// Adds a new page-level bookmark on page `$page`, returning the bookmarks internal ID
    pub fn add_bookmark(&mut self, name: &str, page: usize) -> PageAnnotId {
        let id = PageAnnotId::new();
        self.bookmarks.map.insert(id.clone(), PageAnnotation {
            name: name.to_string(),
            page,
        });
        id
    }

    /// Replaces `document.pages` with the new pages
    pub fn with_pages(&mut self, pages: Vec<PdfPage>) -> &mut Self {
        self.pages = pages;
        self
    }

    /// Serializes the PDF document to bytes
    pub fn save_to_bytes(&self) -> Vec<u8> {
        self::serialize::serialize_pdf_into_bytes(self, &SaveOptions::default())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PdfResources {
    /// Fonts found in the PDF file, indexed by the sha256 of their contents
    pub fonts: PdfFontMap,
    /// XObjects (forms, images, embedded PDF contents, etc.)
    pub xobjects: XObjectMap,
    /// Annotations for links between rects on pages
    pub links: LinkAnnotMap,
    /// Map of explicit extended graphics states
    pub extgstates: ExtendedGraphicsStateMap,
    /// Map of optional content groups
    pub layers: PdfLayerMap,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct PdfLayerMap {
    pub map: BTreeMap<LayerInternalId, Layer>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct PdfFontMap {
    pub map: BTreeMap<FontId, ParsedFont>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct ParsedIccProfile {

}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct XObjectMap {
    pub map: BTreeMap<XObjectId, XObject>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct PageAnnotMap {
    pub map: BTreeMap<PageAnnotId, PageAnnotation>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct LinkAnnotMap {
    pub map: BTreeMap<LinkAnnotId, LinkAnnotation>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct ExtendedGraphicsStateMap {
    pub map: BTreeMap<ExtendedGraphicsStateId, ExtendedGraphicsState>,  
}

/// This is a wrapper in order to keep shared data between the documents XMP metadata and
/// the "Info" dictionary in sync
#[derive(Debug, PartialEq, Clone)]
pub struct PdfMetadata {
    /// Document information
    pub info: PdfDocumentInfo,
    /// XMP Metadata. Is ignored on save if the PDF conformance does not allow XMP
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
            include_str!("../assets/catalog_xmp_metadata.txt"),
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
#[derive(Debug, PartialEq, Clone)]
pub struct XmpMetadata {
    /// Web-viewable or "default" or to be left empty. Usually "default".
    pub rendition_class: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
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
            creation_date: OffsetDateTime::UNIX_EPOCH,
            modification_date: OffsetDateTime::UNIX_EPOCH,
            metadata_date: OffsetDateTime::UNIX_EPOCH,
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
