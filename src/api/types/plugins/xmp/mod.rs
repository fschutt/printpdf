//! Stub plugin for XMP Metadata streams, to be expanded later

extern crate chrono;
extern crate lopdf;

use *;
use api::traits::IntoPdfObject;

/// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
/// Right now it just fills out the necessary fields
#[derive(Debug, Clone)]
pub struct XmpMetadata {    
    /// Creation date of the document
    pub creation_date: chrono::DateTime<chrono::Local>,
    /// Mocificaion date of the document. Currently the same as the creation_date
    pub modify_date: chrono::DateTime<chrono::Local>,
    /// Creation date of the metadata
    pub metadata_date: chrono::DateTime<chrono::Local>,
    /// Title of the document
    pub document_title: String,
    /// Document ID
    pub document_id: String,
    /// Web-viewable or "default" or to be left empty. Usually "default".
    pub rendition_class: Option<String>,
    /// PDF Standard that applies to this document
    pub conformance: PdfConformance,
    /// Is the document trapped? [Read More](https://www.adobe.com/studio/print/pdf/trapping.pdf)
    pub trapping: bool,
    /// Document version
    pub document_version: u32,
}

impl XmpMetadata {

    /// Creates a new XmpMetadata object
    pub fn new<S>(title: S, document_version: u32, trapping: bool, conformance: PdfConformance)
    -> Self where S: Into<String>
    {
        let current_time = chrono::Local::now();

        Self {
            creation_date: current_time.clone(),
            modify_date: current_time.clone(),
            metadata_date: current_time,
            document_title: title.into(),
            document_id: "6b23e74f-ab86-435e-b5b0-2ffc876ba5a2".into(), // todo!
            rendition_class: Some("default".into()),
            conformance: conformance,
            trapping: trapping,
            document_version: document_version,
        }
    }
}

impl IntoPdfObject for XmpMetadata {
    fn into_obj(self: Box<Self>)
    -> lopdf::Object
    {
        use lopdf::{Stream as LoStream, Dictionary as LoDictionary};
        use lopdf::Object::*;
        use rand::Rng;
        use std::iter::FromIterator;

        // Shared between XmpMetadata and DocumentInfo
        let trapping = match self.trapping { true => "True", false => "False" };

        // let xmp_instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let instance_id: std::string::String = rand::thread_rng().gen_ascii_chars().take(32).collect();
        let create_date = self.creation_date.to_rfc3339();
        let modify_date = self.modify_date.to_rfc3339();
        let metadata_date = self.metadata_date.to_rfc3339(); /* preliminary */

        let pdf_x_version = self.conformance.get_identifier_string();
        let document_version = self.document_version.to_string();
        let document_title = self.document_title.clone();
        let document_id = self.document_id.to_string();
        let rendition_class = match self.rendition_class {
            Some(class) => class,
            None => "".to_string(),
        };

        let xmp_metadata = format!(include_str!("../../../../templates/catalog_xmp_metadata.txt"),
                           create_date, modify_date, metadata_date, document_title, document_id, 
                           instance_id, rendition_class, document_version, pdf_x_version, trapping);

        Stream(LoStream::new(LoDictionary::from_iter(vec![
            ("Type", "Metadata".into()),
            ("Subtype", "XML".into()), ]),
            xmp_metadata.as_bytes().to_vec() ))
    }
}