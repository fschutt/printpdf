//! Info dictionary of a PDF document

extern crate lopdf;
extern crate chrono;

use *;

/// "Info" dictionary of a PDF document. 
/// Actual data is contained in DocumentMetadata, to keep it in sync with the XmpMetadata 
/// (if the timestamps / settings are not in sync, Preflight will complain)
#[derive(Debug)]
pub struct DocumentInfo { }

impl DocumentInfo {

    /// Create a new doucment info dictionary from a document
    pub fn new()
    -> Self
    { 
        Self { }
    }

    /// This functions is similar to the IntoPdfObject trait method,
    /// but takes additional arguments in order to delay the setting
    pub(api::types) fn into_obj<S>(self, 
                                   document_title: S, 
                                   trapping: bool, 
                                   conformance: PdfConformance,
                                   creation_date: chrono::DateTime<chrono::Local>,
                                   modification_date: chrono::DateTime<chrono::Local>)
    -> lopdf::Object where S: Into<String>
    {
        use lopdf::Dictionary as LoDictionary;
        use lopdf::Object::*;
        use lopdf::StringFormat::Literal;
        use std::iter::FromIterator;
        use std::string::String;

        let trapping = match trapping { true => "True", false => "False" };
        let gts_pdfx_version = conformance.get_identifier_string();

        // mod_date timestamp format: D:20170505150224+02'00'
        let time_zone = modification_date.format("%z").to_string();
        let mod_date = modification_date.format("D:%Y%m%d%H%M%S");
        let info_mod_date = format!("{}+{}'{}'", mod_date, 
                                    time_zone.chars().take(2).collect::<String>(), 
                                    time_zone.chars().rev().take(2).collect::<String>());

        let time_zone = creation_date.format("%z").to_string();
        let creation_date = creation_date.format("D:%Y%m%d%H%M%S");
        let info_create_date = format!("{}+{}'{}'", creation_date, 
                                    time_zone.chars().take(2).collect::<String>(), 
                                    time_zone.chars().rev().take(2).collect::<String>());

        Dictionary(LoDictionary::from_iter(vec![
            ("Trapped", trapping.into()),
            ("CreationDate", String(info_create_date.into_bytes(), Literal)),
            ("ModDate", String(info_mod_date.into_bytes(), Literal)),
            ("GTS_PDFXVersion", String(gts_pdfx_version.into(), Literal)),
            ("Title", String(document_title.into().as_bytes().to_vec(), Literal))
        ]))
    }
}