//! Info dictionary of a PDF document

extern crate lopdf;
extern crate chrono;
use api::traits::IntoPdfObject;

pub struct DocumentInfo {
    pub trapped: bool,
    pub creation_date: chrono::DateTime,
    pub mod_date: chrono::DateTime,
    pub gts_pdfx_version: String,
    pub document_title: String,
}

impl DocumentInfo {

    /// Create a new doucment info dictionary from a document
    pub fn new(document: &PDFDocument)
    -> Self
    {
        Self {
            trapped: document.trapped,
            creation_date: document.creation_date,
            mod_date: chrono::Local::now(),
            gts_pdfx_version: document.conformity.get_identifer_string(),
            document_title: document.title,
        }
    }
}

impl IntoPdfObject for DocumentInfo {
    fn into_obj(Box<Self>)
    -> lopdf::Object
    {
        use lopdf::Dictionary as LoDictionary;
        use lopdf::Object::*;

        let trapped = match self.trapped { true => "True", false => "False" };

        // mod_date timestamp format: D:20170505150224+02'00'
        let time_zone = self.mod_date.format("%z").to_string();
        let mod_date = self.mod_date.format("D:%Y%m%d%H%M%S");
        let info_mod_date = format!("{}+{}'{}'", mod_date, 
                                    time_zone.chars().take(2).collect(), 
                                    time_zone.chars().rev().take(2).collect());

        let time_zone = self.dreation_date.format("%z").to_string();
        let creation_date = current_time.format("D:%Y%m%d%H%M%S");
        let info_create_date = format!("{}+{}'{}'", creation_date, 
                                    time_zone.chars().take(2).collect(), 
                                    time_zone.chars().rev().take(2).collect());

        Dictionary(LoDictionary::from_iter(vec![
            ("Trapped", trapped.into()),
            ("CreationDate", String(info_create_date.into_bytes(), StringFormat::Literal)),
            ("ModDate", String(info_mod_date.into_bytes(), StringFormat::Literal)),
            ("GTS_PDFXVersion", String(self.gts_pdfx_version.into(), StringFormat::Literal)),
            ("Title", String(self.document_title.clone().into(), StringFormat::Literal))
        ]))
    }
}