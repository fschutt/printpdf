//! `printpdf` PDF library, second API iteration version

use std::collections::BTreeMap;

use annotation::{LinkAnnotation, PageAnnotation};
use conformance::PdfConformance;
use font::ParsedFont;
use ops::PdfPage;
use time::OffsetDateTime;
use utils::{random_character_string_32, to_pdf_xmp_date};
use xobject::XObject;

/// Default ICC profile, necessary if `PdfMetadata::must_have_icc_profile()` return true
pub const ICC_PROFILE_ECI_V2: &[u8] = include_bytes!("../assets/CoatedFOGRA39.icc");

/// Link / bookmark annotation handling
pub mod annotation;
/// PDF standard handling
pub mod conformance;
/// Transformation and text matrices
pub mod matrix;
/// Units (Pt, Mm, Px, etc.)
pub mod units;
/// Date handling (stubs for platforms that don't support access to time clocks, such as wasm32-unknown)
pub mod date;
/// Font and codepoint handling
pub mod font;
/// Point / line / polygon handling
pub mod graphics;
/// Page operations
pub mod ops;
/// Color handling
pub mod color;
/// XObject handling
pub mod xobject;
/// Constants and library includes
pub(crate) mod constants;
/// Utility functions (random strings, numbers, timestamp formatting)
pub(crate) mod utils;

/// Internal ID for page annotations
#[derive(Debug, PartialEq, Clone)]
pub struct PageAnnotId(pub String);

/// Internal ID for link annotations
#[derive(Debug, PartialEq, Clone)]
pub struct LinkAnnotId(pub String);

/// Internal ID for XObjects
#[derive(Debug, PartialEq, Clone)]
pub struct XObjectId(pub String);

/// Internal ID for Fonts
#[derive(Debug, PartialEq, Clone)]
pub struct FontId(pub String);

/// Internal ID for Layers
#[derive(Debug, PartialEq, Clone)]
pub struct LayerInternalId(pub String);

/// Internal ID for extended graphic states
#[derive(Debug, PartialEq, Clone)]
pub struct ExtendedGraphicsStateId(pub String);

/// Internal ID for ICC profiles
#[derive(Debug, PartialEq, Clone)]
pub struct IccProfileId(pub String);

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

#[derive(Debug, PartialEq, Clone)]
pub struct PdfResources {
    /// Fonts found in the PDF file, indexed by the sha256 of their contents
    pub fonts: PdfFontMap,
    /// ICC profiles in this document, indexed by the sha256 of their contents
    pub icc: IccProfileMap,
    /// XObjects (forms, images, embedded PDF contents, etc.)
    pub xobjects: XObjectMap,
    /// Annotations for links between rects on pages
    pub links: LinkAnnotMap,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct PdfFontMap {
    pub map: BTreeMap<FontId, ParsedFont>,  
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct IccProfileMap {
    pub map: BTreeMap<IccProfileId, ParsedIccProfile>,  
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
    pub(crate) fn xmp_metadata_string(self) -> String {

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
