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
    /// XMP Metadata. Is ignored on save if the PDF conformance does not allow XMP
    pub xmp_metadata: XmpMetadata,
    /// PDF Info dictionary. Contains metadata for this document
    pub document_info: DocumentInfo,
    /// Target color profile
    pub target_icc_profile: Option<IccProfile>,
}

impl<'a> PdfDocument {

    /// Creates a new PDF document
    #[inline]
    pub fn new<S>(initial_page: PdfPage, title: S)
    -> (Self, PdfPageIndex, PdfLayerIndex) where S: Into<String> + Clone
    {
        (Self {
            pages: vec![initial_page],
            contents: Vec::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            xmp_metadata: XmpMetadata::new(title.clone(), 1, false, PdfConformance::X3_2003_PDF_1_4),
            document_info: DocumentInfo::new(title.clone(), 1, false, PdfConformance::X3_2003_PDF_1_4),
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
        self.xmp_metadata.trapping = trapping;
        self
    }

    /// Sets the document ID (for comparing two PDF documents for equality)
    #[inline]
    pub fn with_document_id(mut self, id: String)
    -> Self
    {
        self.xmp_metadata.document_id = id;
        self
    }

    /// Set the version of the document
    #[inline]
    pub fn with_document_version(mut self, version: u32)
    -> Self 
    {
        self.xmp_metadata.document_version = version;
        self
    }

    /// Changes the conformance of this document. It is recommended to call 
    /// `check_for_errors()` after changing it.
    #[inline]
    pub fn with_conformance(mut self, conformance: PdfConformance)
    -> Self
    {
        self.xmp_metadata.conformance = conformance;
        self
    }

    /// Sets the modification date on the document. Intended to be used when
    /// reading documents that already have a modification date.
    #[inline]
    pub fn with_mod_date(mut self, mod_date: chrono::DateTime<chrono::Local>)
    -> Self
    {
        self.document_info.modify_date = mod_date.clone();
        self.xmp_metadata.modify_date = mod_date;
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

    // --- MISC FUNCTIONS

    /// Changes the title on both the document info dictionary as well as the metadata
    #[inline]
    pub fn set_title<S>(mut self, new_title: S)
    -> () where S: Into<String> + Clone
    {
        self.xmp_metadata.document_title = new_title.clone().into();
        self.document_info.document_title = new_title.clone().into();
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

        // add icc profile if necessary
        let icc_profile = {
            if self.xmp_metadata.conformance.must_have_icc_profile() {
                match self.target_icc_profile {
                    Some(icc) => Some(icc),
                    None =>      Some(IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), 
                                      IccProfileType::Cmyk)),
                }
            } else {
                None
            }
        };

        // extra pdf infos
        let xmp_metadata = Box::new(self.xmp_metadata).into_obj();
        let xmp_metadata_id = self.inner_doc.add_object(xmp_metadata);
        let document_info = Box::new(self.document_info).into_obj();
        let document_info_id = self.inner_doc.add_object(document_info);
            
        // add catalog 
        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = "FOGRA39";

        use lopdf::StringFormat::Literal as Literal;
        let mut catalog = LoDictionary::from_iter(vec![
                      ("Type", "Catalog".into()),
                      ("PageLayout", "OneColumn".into()),
                      ("PageMode", "Use0".into()),
                      ("Pages", Reference(pages_id)),
                      ("Metadata", Reference(xmp_metadata_id) ),
                      ("OutputIntents", Array(vec![Dictionary(LoDictionary::from_iter(vec![
                          ("S", Name("GTS_PDFX".into())),
                          ("OutputCondition", String(icc_profile_descr.into(), Literal)),
                          ("Type", Name("OutputIntent".into())),
                          ("OutputConditionIdentifier", String(icc_profile_short.into(), Literal)),
                          ("RegistryName", String("www.color.org".into(), Literal)),
                          ("Info", String(icc_profile_str.into(), Literal)), 
                          ])),
                      ])),
                    ]);

        // this may have to go onto the OutputIntents dictionary
        if let Some(profile) = icc_profile { 
            catalog.set("DestinationOutputProfile", Reference(self.inner_doc.add_object(Box::new(profile).into_obj())));
        }

        let mut pages = LoDictionary::from_iter(vec![
                      ("Type", "Pages".into()),
                      ("Count", Integer(self.pages.len() as i64)),
                      /* Kids and Resources missing */
                      ]);

        // add all pages with contents
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

            // add page content (todo)

            page_ids.push(Reference(self.inner_doc.add_object(p)))
        }

        pages.set::<_, LoObject>("Kids".to_string(), page_ids.into());
        self.inner_doc.objects.insert(pages_id, Dictionary(pages));

        // save inner document
        let catalog_id = self.inner_doc.add_object(catalog);
        
        self.inner_doc.trailer.set("Root", Reference(catalog_id));
        self.inner_doc.trailer.set("Info", Reference(document_info_id));

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