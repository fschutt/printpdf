//! Stub plugin for XMP Metadata streams, to be expanded later

use crate::OffsetDateTime;
use lopdf;
use std::string;

use crate::utils::random_character_string_32;
use crate::PdfMetadata;

/// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
/// Right now it just fills out the necessary fields
#[derive(Debug, Clone)]
pub struct XmpMetadata {
    /// Document ID
    pub document_id: String,
    /// Web-viewable or "default" or to be left empty. Usually "default".
    pub rendition_class: Option<String>,
    /// Document version
    pub document_version: u32,
}

impl XmpMetadata {
    /// Creates a new XmpMetadata object
    pub fn new(rendition_class: Option<String>, document_version: u32) -> Self {
        let document_id: String = random_character_string_32();
        Self {
            document_id,
            rendition_class,
            document_version,
        }
    }

    /// Consumes the XmpMetadata and turns it into a PDF Object.
    /// This is similar to the
    pub(crate) fn into_obj(self, m: &PdfMetadata) -> lopdf::Object {
        use lopdf::Object::*;
        use lopdf::{Dictionary as LoDictionary, Stream as LoStream};

        // Shared between XmpMetadata and DocumentInfo
        let trapping = if m.trapping { "True" } else { "False" };

        // let xmp_instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let instance_id = random_character_string_32();
        let create_date = to_pdf_xmp_date(&m.creation_date);
        let modification_date = to_pdf_xmp_date(&m.modification_date);
        let metadata_date = to_pdf_xmp_date(&m.metadata_date);

        let pdf_x_version = m.conformance.get_identifier_string();
        let document_version = self.document_version.to_string();
        let document_id = self.document_id.to_string();

        let rendition_class = match self.rendition_class {
            Some(class) => class,
            None => string::String::new(),
        };

        let xmp_metadata = format!(
            include_str!("../assets/catalog_xmp_metadata.txt"),
            create = create_date,
            modify = modification_date,
            mdate = metadata_date,
            title = m.document_title,
            id = document_id,
            instance = instance_id,
            class = rendition_class,
            version = document_version,
            pdfx = pdf_x_version,
            trapping = trapping,
            creator = m.creator,
            subject = m.subject,
            keywords = m.keywords.join(","),
            identifier = m.identifier,
            producer = m.producer
        );

        Stream(LoStream::new(
            LoDictionary::from_iter(vec![("Type", "Metadata".into()), ("Subtype", "XML".into())]),
            xmp_metadata.as_bytes().to_vec(),
        ))
    }
}

// D:2018-09-19T10:05:05+00'00'
fn to_pdf_xmp_date(date: &OffsetDateTime) -> String {
    // Since the time is in UTC, we know that the time zone
    // difference to UTC is 0 min, 0 sec, hence the 00'00
    format!(
        "D:{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00'00'",
        date.year(),
        date.month(),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
    )
}
