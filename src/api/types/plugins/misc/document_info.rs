//! Info dictionary of a PDF document

extern crate lopdf;
extern crate chrono;

use *;
use api::traits::IntoPdfObject;

#[derive(Debug)]
pub struct DocumentInfo {
    /// Is the document trapped?
    pub trapping: bool,
    /// PDF document title
    pub document_title: String,
    /// PDF document version
    pub document_version: u32,
    /// PDF Standard
    pub conformance: PdfConformance,
    /// Creation date of the document
    pub creation_date: chrono::DateTime<chrono::Local>,
    /// Modification date of the document
    pub modify_date: chrono::DateTime<chrono::Local>,
}

impl DocumentInfo {

    /// Create a new doucment info dictionary from a document
    pub fn new<S>(title: S, document_version: u32, trapping: bool, conformance: PdfConformance)
    -> Self where S: Into<String>
    { 
        let current_time = chrono::Local::now();
        Self {
            document_title: title.into(),
            trapping: trapping,
            document_version: document_version,
            creation_date: current_time,
            modify_date: current_time,
            conformance: conformance,
        }
    }
}

impl IntoPdfObject for DocumentInfo {

    fn into_obj(self: Box<Self>)
    -> lopdf::Object
    {
        use lopdf::Dictionary as LoDictionary;
        use lopdf::Object::*;
        use lopdf::StringFormat::Literal;
        use std::iter::FromIterator;
        use std::string::String;

        let trapping = match self.trapping { true => "True", false => "False" };

        let gts_pdfx_version = self.conformance.get_identifier_string();

        // mod_date timestamp format: D:20170505150224+02'00'
        let time_zone = self.modify_date.format("%z").to_string();
        let mod_date = self.modify_date.format("D:%Y%m%d%H%M%S");
        let info_mod_date = format!("{}+{}'{}'", mod_date, 
                                    time_zone.chars().take(2).collect::<String>(), 
                                    time_zone.chars().rev().take(2).collect::<String>());

        let time_zone = self.creation_date.format("%z").to_string();
        let creation_date = self.creation_date.format("D:%Y%m%d%H%M%S");
        let info_create_date = format!("{}+{}'{}'", creation_date, 
                                    time_zone.chars().take(2).collect::<String>(), 
                                    time_zone.chars().rev().take(2).collect::<String>());

        Dictionary(LoDictionary::from_iter(vec![
            ("Trapped", trapping.into()),
            ("CreationDate", String(info_create_date.into_bytes(), Literal)),
            ("ModDate", String(info_mod_date.into_bytes(), Literal)),
            ("GTS_PDFXVersion", String(gts_pdfx_version.into(), Literal)),
            ("Title", String(self.document_title.clone().into(), Literal))
        ]))
    }
}