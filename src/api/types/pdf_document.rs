//! A `PDFDocument` represents the whole content of the file

extern crate lopdf;
extern crate chrono;
extern crate rand;

use *;
use api::types::indices::*;
use std::io::{Write, Seek};

/// PDF document
#[derive(Debug)]
pub struct PdfDocument {
    /// Pages of the document
    pages: Vec<PdfPage>,
    /// PDF contents (subject to change)
    contents: Vec<Box<IntoPdfObject>>,
    /// Inner PDF document
    inner_doc: lopdf::Document,
    /// PDF document title
    pub title: String,
    /// Is the document trapped? [Read More](https://www.adobe.com/studio/print/pdf/trapping.pdf)
    pub trapping: bool,
    /// Document version
    pub document_version: u32,
    /// PDF conformance currently set for this document
    pub conformance: PdfConformance,
    /// XMP Metadata. Is ignored on save if the PDF conformance does not allow XMP
    pub xmp_metadata: Option<XmpMetadata>,
    /// Document ID, used for comparing two documents for equality
    pub document_id: String,
    /// Instance ID, changed when the document is saved
    pub instance_id: Option<String>,
    /// Target color profile
    pub target_icc_profile: Option<IccProfile>,
}

impl<'a> PdfDocument {

    /// Creates a new PDF document
    #[inline]
    pub fn new<S>(initial_page: PdfPage, title: S)
    -> (Self, PdfPageIndex, PdfLayerIndex) where S: Into<String>
    {
        (Self {
            pages: vec![initial_page],
            title: title.into(),
            contents: Vec::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            trapping: false,
            document_version: 1,
            conformance: PdfConformance::X3_2003_PDF_1_4,
            xmp_metadata: None,
            document_id: "6b23e74f-ab86-435e-b5b0-2ffc876ba5a2".into(), // todo!
            instance_id: None,
            target_icc_profile: None,
        },
        PdfPageIndex(0),
        PdfLayerIndex(0))
    }

    /// Checks for invalid settings in the document
    pub fn check_for_errors(&mut self) 
    -> ::std::result::Result<(), Error>
    {
        // todo
        Ok(())
    }

    /// Tries to match the document to the given conformance.
    /// Errors only on an unrecoverable error.
    pub fn repair_errors(&mut self, conformance: PdfConformance)
    -> ::std::result::Result<(), Error>
    {
        //todo
        Ok(())
    }

    // ----- BUILDER FUNCTIONS

    /// Set the trapping of the document
    #[inline]
    pub fn with_trapping(mut self, trapping: bool)
    -> Self 
    {
        self.trapping = trapping;
        self
    }

    /// Sets the document ID (for comparing two PDF documents for equality)
    #[inline]
    pub fn with_document_id(mut self, id: String)
    -> Self
    {
        self.document_id = id;
        self
    }

    /// Set the version of the document
    #[inline]
    pub fn with_document_version(mut self, version: u32)
    -> Self 
    {
        self.document_version = version;
        self
    }

    /// Changes the conformance of this document. It is recommended to call `check_for_errors()`
    /// after changing it.
    #[inline]
    pub fn with_conformance(mut self, conformance: PdfConformance)
    -> Self
    {
        self.conformance = conformance;
        self
    }

    // ----- ADD FUNCTIONS

    /// Create a new pdf page and returns the index of the page
    #[inline]
    pub fn add_page(&mut self, x_mm: f64, y_mm: f64, inital_layer: PdfLayer)
    -> (PdfPageIndex, PdfLayerIndex)
    {
        self.pages.push(PdfPage::new(x_mm, y_mm, inital_layer));
        (PdfPageIndex(self.pages.len() - 1), PdfLayerIndex(0))
    }

    /// Add arbitrary Pdf Objects. These are tracked by reference and get 
    /// instantiated / referenced when the document is saved.
    #[inline]
    pub fn add_arbitrary_content<C>(&mut self, content: Box<C>)
    -> PdfContentIndex where C: 'static + IntoPdfObject
    {
        self.contents.push(content);
        PdfContentIndex(self.contents.len() - 1)
    }

    /// Add a font from a font stream
    #[inline]
    pub fn add_font<R>(&mut self, font_stream: R)
    -> ::std::result::Result<FontIndex, Error> where R: ::std::io::Read
    {
        use api::types::plugins::graphics::two_dimensional::Font;
        let font = Font::new(font_stream)?;
        let index = self.add_arbitrary_content(Box::new(font));
        Ok(FontIndex(index))
    }

    /// Add SVG content to the document
    #[inline]
    pub fn add_svg<R>(&mut self,
                      svg_data: R)
    -> ::std::result::Result<SvgIndex, Error> 
    where R: ::std::io::Read
    {
        // todo
        unimplemented!()
    }

    // ----- GET FUNCTIONS

    /// Returns the page (for inserting content)
    #[inline]
    pub fn get_page_mut(&mut self, page: PdfPageIndex)
    -> &mut PdfPage
    {
        self.pages.get_mut(page.0).unwrap()
    }

    /// Drops the PDFDocument, returning the inner `lopdf::Document`. 
    /// Document may be only half-written
    #[inline]
    pub unsafe fn get_inner(self)
    -> (lopdf::Document, Vec<Box<IntoPdfObject>>)
    {
        (self.inner_doc, self.contents)
    }


    /// Save PDF Document, writing the contents to the target
    pub fn save<W: Write + Seek>(mut self, target: &mut W)
    -> ::std::result::Result<(), Error>
    {
        use lopdf::{Dictionary as LoDictionary, 
                    Object as LoObject, 
                    Stream as LoStream};
        use lopdf::Object::*;
        use lopdf::StringFormat;
        use std::iter::FromIterator;
        use PdfConformance::*;
        use api::traits::IntoPdfObject;
        use rand::Rng;

        let pages_id = self.inner_doc.new_object_id();
        let info_id = self.inner_doc.new_object_id();

        let current_time = chrono::Local::now();
        let xmp_create_date = current_time.to_rfc3339();
        let xmp_modify_date = xmp_create_date.clone();
        let xmp_document_metadata_date = xmp_create_date.clone(); /* preliminary */
        
        // info_mod timestamp: D:20170505150224+02'00'
        let info_create_date = current_time.format("D:");
        let time_zone = current_time.format("%z").to_string();
        let mod_date = current_time.format("D:%Y%m%d%H%M%S");
        let info_mod_date = format!("{}+{}'{}'", mod_date, 
                                    time_zone.chars().take(2).collect(), 
                                    time_zone.chars().rev().take(2).collect());

        let xmp_document_title = self.title.clone();
        let xmp_document_id = self.document_id.clone();
        // let xmp_instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let xmp_instance_id = rand::thread_rng().gen_ascii_chars().take(32).collect();

        let rendition_class = match self.xmp_metadata {
            Some(meta) => Some(Box::new(meta).into_obj()),
            None => match self.conformance.must_have_xmp_metadata() {
                        true => None, /* todo: error on certain conformance levels */
                        false => None,
                    }
        };

        let document_version = self.document_version.to_string();
        let pdf_x_version = self.conformance.get_identifier_string();
        let trapped = match self.trapping { true => "True", false => "False" };

        // extra pdf infos required for pdf/x-3
        let info = LoDictionary::from_iter(vec![
            ("Trapped", trapped.into()),
            ("CreationDate", String(info_mod_date.into_bytes(), StringFormat::Literal)),
            ("ModDate", String(info_mod_date.into_bytes(), StringFormat::Literal)),
            ("GTS_PDFXVersion", String(pdf_x_version.into(), StringFormat::Literal)),
            ("Title", String(xmp_document_title.clone().into(), StringFormat::Literal))
        ]);

        self.inner_doc.objects.insert(info_id, Dictionary(info));

        // add icc profile
        let icc_profile = match self.icc_profile {

            Some(icc) => { let stream_obj = Box::new(icc).into_obj(); 
                           Some(Reference(self.inner_doc.add_object(stream_obj))) },

            None =>      match self.conformance.must_have_icc_profile() {
                             true => Some(IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), 
                                          IccProfileType::Cmyk)),
                             false => None,
                         }
        };

        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = "FOGRA39";

        // xmp metadata
        let catalog = LoDictionary::from_iter(vec![
                      ("Type", "Catalog".into()),
                      ("PageLayout", "OneColumn".into()),
                      ("PageMode", "Use0".into()),
                      ("Pages", Reference(pages_id)),
                      ("Metadata", Reference(self.inner_doc.add_object(stream)) ),
                      ("OutputIntents", Array(vec![Dictionary(LoDictionary::from_iter(vec![
                          ("S", Name("GTS_PDFX".into())),
                          ("OutputCondition", String(icc_profile_descr.into(), StringFormat::Literal)),
                          ("Type", Name("OutputIntent".into())),
                          ("OutputConditionIdentifier", String(icc_profile_short.into(), StringFormat::Literal)),
                          ("RegistryName", String("www.color.org".into(), StringFormat::Literal)),
                          ("Info", String(icc_profile_str.into(), StringFormat::Literal)), 
                          ])),
                      ])),
                    ]);

        if let Some(i) = icc_profile { ("DestinationOutputProfile", Reference(i))}

        let mut pages = LoDictionary::from_iter(vec![
                      ("Type", "Pages".into()),
                      ("Count", Integer(self.pages.len() as i64)),
                      /* Kids and Resources missing */
                      ]);

        // add all contents, save references
        // todo

        // add pages
        let mut page_ids = Vec::<LoObject>::new();

        for page in self.pages.into_iter() {
            
            let p = LoDictionary::from_iter(vec![
                      ("Type", "Page".into()),
                      ("Rotate", Integer(0)),
                      ("MediaBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("TrimBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("CropBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("Parent", Reference(pages_id)) ]);

            // add page content references
            // todo

            page_ids.push(Reference(self.inner_doc.add_object(p)))
        }

        pages.set::<_, LoObject>("Kids".to_string(), page_ids.into());
        self.inner_doc.objects.insert(pages_id, Dictionary(pages));

        // save inner document
        let catalog_id = self.inner_doc.add_object(catalog);
        
        self.inner_doc.trailer.set("Root", Reference(catalog_id));
        self.inner_doc.trailer.set("Info", Reference(info_id));
        
        self.inner_doc.prune_objects();
        self.inner_doc.delete_zero_length_streams();
        self.inner_doc.compress();
        self.inner_doc.save_to(target).unwrap();

        Ok(())
    }
}

/*
impl std::convert::From<lopdf::Doument> for PdfDocument
{
    fn from(doc: lopdf::Doument) -> Self
    {
        
    }
}
*/