//! Stub plugin for XMP Metadata streams, to be expanded later

use *;

/// Initial struct for Xmp metatdata. This should be expanded later for XML handling, etc.
/// Right now it just fills out the necessary fields
#[derive(Debug, Clone)]
pub struct XmpMetadata {
    /// "default" or to be left empty. Usually "default".
    rendition_class: Option<String>,
}

impl XmpMetadata {

    /// Creates a new XmpMetadata object
    pub fn new(document: &PdfDocument)
    -> Self
    {
        Self {
            rendition_class: Some("default"),
        }
    }
}

impl IntoPdfObject for XmpMetadata {
    fn into_obj(self: Box<Self>)
    -> lopdf::Object
    {
        use lopdf::Object::*;
        use lopdf::StringFormat;

        let xmp_metadata = format!(include_str!("../../../../templates/catalog_xmp_metadata.txt"),
                           create_date, modify_date, metadata_date, document_title, document_id, 
                           instance_id, rendition_class, document_version, pdf_x_version, trapped);

        let stream = Stream(LoStream::new(LoDictionary::from_iter(vec![
                          ("Type", "Metadata".into()),
                          ("Subtype", "XML".into()), ]),
                          xmp_metadata.as_bytes().to_vec() ));
        
        match self.rendition_class {
            Some(r) => String(r.to_string().into_bytes(), StringFormat::Literal),
            None => String("".to_string().into_bytes(), StringFormat::Literal),
        }
    }
}