//! Stub plugin for XMP Metadata streams, to be expanded later

extern crate chrono;
extern crate lopdf;

use *;
use rand::Rng;

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
    pub fn new(rendition_class: Option<String>, document_version: u32)
    -> Self
    {
        let document_id: String = rand::thread_rng().gen_ascii_chars().take(32).collect();
        Self {
            document_id: document_id,
            rendition_class: rendition_class,
            document_version: document_version,
        }
    }

    /// Consumes the XmpMetadata and turns it into a PDF Object.
    /// This is similar to the
    pub(in types) fn into_obj<S>(self,
                           conformance: PdfConformance,
                           trapping: bool,
                           creation_date: chrono::DateTime<chrono::Local>,
                           modification_date: chrono::DateTime<chrono::Local>,
                           metadata_date: chrono::DateTime<chrono::Local>,
                           document_title: S)
    -> lopdf::Object where S: Into<String> + std::fmt::Display
    {
        use lopdf::{Stream as LoStream, Dictionary as LoDictionary};
        use lopdf::Object::*;
        use std::iter::FromIterator;

        // Shared between XmpMetadata and DocumentInfo
        let trapping = if trapping { "True" } else { "False" };

        // let xmp_instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let instance_id: std::string::String = rand::thread_rng().gen_ascii_chars().take(32).collect();
        let create_date = to_pdf_xmp_date(creation_date);
        let modification_date = to_pdf_xmp_date(modification_date);
        let metadata_date = to_pdf_xmp_date(metadata_date);

        let pdf_x_version = conformance.get_identifier_string();
        let document_version = self.document_version.to_string();
        let document_id = self.document_id.to_string();

        let rendition_class = match self.rendition_class {
            Some(class) => class,
            None => "".to_string(),
        };

        let xmp_metadata = format!(include_str!("../../../templates/catalog_xmp_metadata.txt"),
                           create_date, modification_date, metadata_date, document_title, document_id,
                           instance_id, rendition_class, document_version, pdf_x_version, trapping);

        Stream(LoStream::new(LoDictionary::from_iter(vec![
            ("Type", "Metadata".into()),
            ("Subtype", "XML".into()), ]),
            xmp_metadata.as_bytes().to_vec() ))
    }
}


fn to_pdf_xmp_date(date: chrono::DateTime<chrono::Local>)
-> String
{
    // 2017-05-16T16:00:05+02:00
    let time_zone = date.format("%z").to_string();
    let mod_date = date.format("D:%Y-%m-%dT%H:%M:%S");
    format!("{}{}'{}'", mod_date,
                        time_zone.chars().take(3).collect::<String>(),
                        time_zone.chars().rev().take(2).collect::<String>())
}
