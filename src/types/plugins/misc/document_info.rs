//! Info dictionary of a PDF document

extern crate lopdf;
extern crate chrono;

use *;

/// "Info" dictionary of a PDF document. 
/// Actual data is contained in DocumentMetadata, to keep it in sync with the XmpMetadata 
/// (if the timestamps / settings are not in sync, Preflight will complain)
#[derive(Debug)]
pub struct DocumentInfo {
    // DocumentInfo is older than XmpMetadata
    // The following is a list of things available to the DocumentInfo dictionary.
    // These keys don't have to be set:

    /*
    /Author ( Craig J. Hogan ) 
    /Subject ( doi:10.1038/445037a ) 
    /Keywords ( cosmology infrared protogalaxy starlight ) 
    /Identifier ( doi:10.1038/445037a ) 
    /Creator ( … ) 
    /Producer ( … )
    */

    // The more modern approach is to put them into the XmpMetadata struct:
    // This struct is merely a wrapper around those types that HAVE to be in a PDF/X-conform
    // document.
    
    /*
    <rdf:Description rdf:about=“” xmlns:dc=“http://purl.org/dc/elements/1.1/"> 
            <dc:creator>Craig J. Hogan</dc:creator>
            <dc:title>Cosmology: Ripples of early starlight</dc:title>
            <dc:identifier>doi:10.1038/445037a</dc:identifier>
            <dc:source>Nature 445, 37 (2007)</dc:source>
            <dc:date>2007-01-04</dc:date>
            <dc:format>application/pdf</dc:format>
            <dc:publisher>Nature Publishing Group</dc:publisher>
            <dc:language>en<dc:language>
            <dc:rights>© 2007 Nature Publishing Group</dc:rights> 
    </rdf:Description>   
    <rdf:Description rdf:about=“” xmlns:prism=“http://prismstandard.org/namespaces/1.2/basic/"> 
        <prism:publicationName>Nature</prism:publicationName>
        <prism:issn>0028-0836</prism:issn>
        <prism:eIssn>1476-4679</prism:eIssn>
        <prism:publicationDate>2007-01-04</prism:publicationDate>
        <prism:copyright>© 2007 Nature Publishing Group</prism:copyright>
        <prism:rightsAgent>permissions@nature.com</prism:rightsAgent>
        <prism:volume>445</prism:volume> <prism:number>7123</prism:number>
        <prism:startingPage>37</prism:startingPage>
        <prism:endingPage>37</prism:endingPage>
        <prism:section>News and Views</prism:section> 
    </rdf:Description>      
    */
}

impl DocumentInfo {

    /// Create a new doucment info dictionary from a document
    pub fn new()
    -> Self
    { 
        Self { }
    }

    /// This functions is similar to the IntoPdfObject trait method,
    /// but takes additional arguments in order to delay the setting
    pub(in types) fn into_obj<S>(self, 
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