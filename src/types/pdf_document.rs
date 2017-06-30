//! A `PDFDocument` represents the whole content of the file

extern crate lopdf;
extern crate chrono;
extern crate rand;

use *;
use indices::*;
use std::io::{Write, Seek};
use rand::Rng;
use std::sync::{Arc, Mutex};

/// PDF document
#[derive(Debug)]
pub struct PdfDocument {
    /// Pages of the document
    pub(super) pages: Vec<PdfPage>,
    /// PDF contents as references.
    /// As soon as data gets added to the inner_doc, a reference gets pushed into here
    pub(super) contents: Vec<lopdf::Object>,
    /// Inner PDF document
    pub(super) inner_doc: lopdf::Document,
    /// Document ID. Must be changed if the document is loaded / parsed from a file
    pub document_id: std::string::String,
    /// Metadata for this document
    pub metadata: PdfMetadata,
}

/// Marker struct for a document. Used to make the API a bit nicer.
/// It simply calls PdfDocument:: ... functions.
pub struct PdfDocumentReference {
    pub document: Arc<Mutex<PdfDocument>>,
}

impl PdfDocument {

    /// Creates a new PDF document
    #[inline]
    pub fn new<S>(document_title: S, initial_page_width_mm: f64, initial_page_height_mm: f64, 
                  initial_layer_name: S)
    -> (PdfDocumentReference, PdfPageIndex, PdfLayerIndex) where S: Into<String>
    {
        let doc = Self {
            pages: Vec::new(),
            document_id: rand::thread_rng().gen_ascii_chars().take(32).collect(),
            contents: Vec::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            metadata: PdfMetadata::new(document_title, 1, false, PdfConformance::X3_2002_PDF_1_3)
        };

        let doc_ref = Arc::new(Mutex::new(doc));

        let (initial_page, layer_index) = PdfPage::new(
            initial_page_width_mm, 
            initial_page_height_mm, 
            initial_layer_name,
            0);

        { doc_ref.lock().unwrap().pages.push(initial_page); }

        (PdfDocumentReference { document: doc_ref }, PdfPageIndex(0), layer_index)
    }

}

impl PdfDocumentReference {

    // ----- BUILDER FUNCTIONS

    /// Changes the title on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_title<S>(self, new_title: S)
    -> () where S: Into<String>
    {
        self.document.lock().unwrap().metadata.document_title = new_title.into();
    }

    /// Set the trapping of the document
    #[inline]
    pub fn with_trapping(self, trapping: bool)
    -> Self 
    {
        self.document.lock().unwrap().metadata.trapping = trapping;
        self
    }

    /// Sets the document ID (for comparing two PDF documents for equality)
    #[inline]
    pub fn with_document_id(self, id: String)
    -> Self
    {
        self.document.lock().unwrap().metadata.xmp_metadata.document_id = id;
        self
    }

    /// Set the version of the document
    #[inline]
    pub fn with_document_version(self, version: u32)
    -> Self 
    {
        self.document.lock().unwrap().metadata.document_version = version;
        self
    }

    /// Changes the conformance of this document. It is recommended to call 
    /// `check_for_errors()` after changing it.
    #[inline]
    pub fn with_conformance(self, conformance: PdfConformance)
    -> Self
    {
        self.document.lock().unwrap().metadata.conformance = conformance;
        self
    }

    /// Sets the modification date on the document. Intended to be used when
    /// reading documents that already have a modification date.
    #[inline]
    pub fn with_mod_date(self, mod_date: chrono::DateTime<chrono::Local>)
    -> Self
    {
        self.document.lock().unwrap().metadata.modification_date = mod_date;
        self
    }

    // ----- ADD FUNCTIONS

    /// Create a new pdf page and returns the index of the page
    #[inline]
    pub fn add_page<S>(&self, x_mm: f64, y_mm: f64, inital_layer_name: S)
    -> (PdfPageIndex, PdfLayerIndex) where S: Into<String>
    {
        let mut doc = self.document.lock().unwrap();
        let (pdf_page, pdf_layer_index) = PdfPage::new(x_mm, y_mm, inital_layer_name, doc.pages.len());
        doc.pages.push(pdf_page);
        let page_index = PdfPageIndex(self.document.lock().unwrap().pages.len() - 1);
        (page_index, pdf_layer_index)
    }

    /// Add arbitrary Pdf Objects. These are tracked by reference and get 
    /// instantiated / referenced when the document is saved.
    #[inline]
    pub fn add_arbitrary_content<C>(&self, content: Box<C>, only_use_first_item: bool)
    -> PdfContentIndex where C: 'static + IntoPdfObject
    {
        let mut doc = self.document.lock().unwrap();
        let obj_id = if only_use_first_item {
            doc.inner_doc.add_object(content.into_obj()[0].to_owned())
        } else {
            doc.inner_doc.add_object(content.into_obj())
        };

        doc.contents.push(lopdf::Object::Reference(obj_id));
        PdfContentIndex(doc.contents.len() - 1)
    }

    /// Add a font from a font stream
    #[inline]
    pub fn add_font<R>(&self, font_stream: R)
    -> ::std::result::Result<FontIndex, Error> where R: ::std::io::Read
    {
        let font = Font::new(font_stream)?;
        let index = self.add_arbitrary_content(Box::new(font), true);

        // let doc = self.document.lock().unwrap();
        // let font_ref = doc.contents.get(index.0).unwrap();
        // let font = doc.inner_doc.get_object(font_ref);

        // let font_stream_id = &self.doc.add_object(font_stream);
        // font_descriptor_vec.push(("FontFile3".into(), Reference(*font_stream_id)));

        // Create dictionaries and add to DOM
        // let font_descriptor_id = &self.doc.add_object(LoDictionary::from_iter(font_descriptor_vec));
        // desc_fonts.set("FontDescriptor".to_string(), Reference(*font_descriptor_id));

        // Embed character ids
        // let cid_to_unicode_map_stream_id = &self.doc.add_object(Stream(cid_to_unicode_map_stream));
        // font_vec.push(("ToUnicode".into(), Reference(*cid_to_unicode_map_stream_id)));
        // let char_to_cid_map_stream_id = &self.doc.add_object(Stream(char_to_cid_map_stream));
        // font_vec.push(("Encoding".into(), Name("Identity-H".into())));

        // let desc_fonts_id = &self.doc.add_object(Array(vec![Dictionary(desc_fonts)]));
        // font_vec.push(("DescendantFonts".into(), Reference(*desc_fonts_id)));

        // let font = LoDictionary::from_iter(font_vec);
        // &self.fonts.insert(face_name.clone(), font);

        Ok(FontIndex(index))
    }

    /// Add SVG content to the document
    #[inline]
    pub fn add_svg<R>(&self, svg_data: R)
    -> ::std::result::Result<SvgIndex, Error>
    where R: ::std::io::Read
    {
        let svg_obj = Svg::new(svg_data)?;
        let index = self.add_arbitrary_content(Box::new(svg_obj), false);
        Ok(SvgIndex(index))
    }

    // ----- GET FUNCTIONS

    /// Returns the page (for inserting content)
    #[inline]
    pub fn get_page(&self, page: PdfPageIndex)
    -> PdfPageReference
    {
        self.document.lock().unwrap().pages.get(page.0).unwrap();
        PdfPageReference { document: Arc::downgrade(&self.document).clone(), page }
    }

    /// Drops the PDFDocument, returning the inner `lopdf::Document`. 
    /// Document may be only half-written
    #[inline]
    pub unsafe fn get_inner(self)
    -> (lopdf::Document, Vec<lopdf::Object>)
    {
        let doc = Arc::try_unwrap(self.document).unwrap().into_inner().unwrap();
        (doc.inner_doc, doc.contents)
    }

    // --- MISC FUNCTIONS

    /// Checks for invalid settings in the document
    pub fn check_for_errors(&self) 
    -> ::std::result::Result<(), Error>
    {
        // todo
        warn!("Checking PDFs for errors is currently not supported!");
        Ok(())
    }

    /// Tries to match the document to the given conformance.
    /// Errors only on an unrecoverable error.
    pub fn repair_errors(&self, conformance: PdfConformance)
    -> ::std::result::Result<(), Error>
    {
        //todo
        warn!("Reparing PDFs is currently not supported!");
        Ok(())
    }

    /// Save PDF Document, writing the contents to the target
    pub fn save<W: Write + Seek>(self, target: &mut W)
    -> ::std::result::Result<(), Error>
    {
        use lopdf::{Dictionary as LoDictionary, 
                    Object as LoObject};
        use lopdf::Object::*;
        use std::iter::FromIterator;

        // todo: remove unwrap, handle error
        let mut doc = Arc::try_unwrap(self.document).unwrap().into_inner().unwrap();
        let pages_id = doc.inner_doc.new_object_id();

        // extra pdf infos
        let (xmp_metadata, document_info, icc_profile) = doc.metadata.clone().into_obj();
        let xmp_metadata_id = doc.inner_doc.add_object(xmp_metadata);
        let document_info_id = doc.inner_doc.add_object(document_info);
            
        // add catalog 
        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = "FOGRA39";

        use lopdf::StringFormat::Literal as Literal;
        let mut output_intents = LoDictionary::from_iter(vec![
                          ("S", Name("GTS_PDFX".into())),
                          ("OutputCondition", String(icc_profile_descr.into(), Literal)),
                          ("Type", Name("OutputIntent".into())),
                          ("OutputConditionIdentifier", String(icc_profile_short.into(), Literal)),
                          ("RegistryName", String("http://www.color.org".into(), Literal)),
                          ("Info", String(icc_profile_str.into(), Literal)), 
                          ]);

        if let Some(profile) = icc_profile { 
            use traits::IntoPdfObject;
            let vec_icc_profiles = Box::new(profile).into_obj();
            let icc_profile_id = doc.inner_doc.add_object(vec_icc_profiles[0].to_owned());
            output_intents.set("DestinationOutputProfile", Reference(icc_profile_id));
        }

        let catalog = LoDictionary::from_iter(vec![
                      ("Type", "Catalog".into()),
                      ("PageLayout", "OneColumn".into()),
                      ("PageMode", "Use0".into()),
                      ("Pages", Reference(pages_id)),
                      ("Metadata", Reference(xmp_metadata_id) ),
                      ("OutputIntents", Array(vec![Dictionary(output_intents)])),
                    ]);

        let mut pages = LoDictionary::from_iter(vec![
                      ("Type", "Pages".into()),
                      ("Count", Integer(doc.pages.len() as i64)),
                      /* Kids and Resources missing */
                      ]);

        // add all pages with contents
        let mut page_ids = Vec::<LoObject>::new();

        for page in doc.pages.into_iter() {
            
            let mut p = LoDictionary::from_iter(vec![
                      ("Type", "Page".into()),
                      ("Rotate", Integer(0)),
                      ("MediaBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("TrimBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("CropBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("Parent", Reference(pages_id)) ]);

            // this will collect the resources needed for rendering this page
            let (resources_page, layer_streams) = page.collect_resources_and_streams(&doc.contents);

            if !(resources_page.len() == 0) {
                let resources_page_id = doc.inner_doc.add_object(lopdf::Object::Dictionary(resources_page));
                p.set("Resources", Reference(resources_page_id));
            }

            // merge layer streams
            let mut layer_streams_merged_vec = Vec::<u8>::new();

            // merge all streams of the individual layers into one big stream
            for mut stream in layer_streams {

                // todo: write begin of pdf layer

                // todo: check if pdf is allowed to have layers
                // if metadata.conformance.is_layering_allowed() { }

                layer_streams_merged_vec.append(&mut stream.content);
                // todo: write end of pdf layer
            }

            let merged_layer_stream = lopdf::Stream::new(lopdf::Dictionary::new(), layer_streams_merged_vec);
            let page_content_id = doc.inner_doc.add_object(merged_layer_stream);
            p.set("Contents", Reference(page_content_id));

            page_ids.push(Reference(doc.inner_doc.add_object(p)))
        }

        pages.set::<_, LoObject>("Kids".to_string(), page_ids.into());
        doc.inner_doc.objects.insert(pages_id, Dictionary(pages));

        // save inner document
        let catalog_id = doc.inner_doc.add_object(catalog);
        let instance_id: std::string::String = rand::thread_rng().gen_ascii_chars().take(32).collect();

        doc.inner_doc.trailer.set("Root", Reference(catalog_id));
        doc.inner_doc.trailer.set("Info", Reference(document_info_id));
        doc.inner_doc.trailer.set("ID", Array(vec![
                                            String(doc.document_id.as_bytes().to_vec(), Literal), 
                                            String(instance_id.as_bytes().to_vec(), Literal)
                                        ]));

        // doc.inner_doc.prune_objects();
        // doc.inner_doc.delete_zero_length_streams();
        // doc.inner_doc.compress();
        doc.inner_doc.save_to(target).unwrap();

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