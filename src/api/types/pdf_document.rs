//! A `PDFDocument` represents the whole content of the file

extern crate lopdf;

use *;
use std::io::{Write, Seek};

/// PDF document
#[derive(Debug)]
pub struct PdfDocument {
    // Pages of the document
    pages: Vec<PdfPage>,
    /// PDF document title
    title: String,
    /// PDF contents (subject to change)
    contents: Vec<Box<IntoPdfObject>>,
    /// Inner PDF document
    inner_doc: lopdf::Document,
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
        },
        PdfPageIndex(0),
        PdfLayerIndex(0))
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

    /// ## Miscellaneous functions

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

        // add root, catalog & pages
        let pages_id = self.inner_doc.new_object_id();
        let dest_output_profile = self.inner_doc.new_object_id();

        let create_date = "2017-05-05T15:02:24+02:00";
        let modify_date = "2017-05-05T15:02:24+02:00";
        let metadata_date = "2017-05-05T15:02:24+02:00";
        let document_title = self.title.clone();
        let document_id = "6b23e74f-ab86-435e-b5b0-2ffc876ba5a2";
        let instance_id = "2898d852-f86f-4479-955b-804d81046b19";
        let document_version = "1";
        let rendition_class = "default";
        let pdf_x_version = "PDF/X-3:2002";
        let trapped = "False";

        // extra pdf infos required for pdf/x-3
        let info = Dictionary(LoDictionary::from_iter(vec![
            ("Trapped", "False".into()),
            ("CreationDate", String("D:20170505150224+02'00'".into(), StringFormat::Literal)),
            ("ModDate", String("D:20170505150224+02'00'".into(), StringFormat::Literal)),
            ("GTX_PDFXVersion", String(pdf_x_version.into(), StringFormat::Literal)),
            ("Title", String(document_title.clone().into(), StringFormat::Literal))
        ]));

        let info_id = self.inner_doc.add_object(info);
        
        // overprint key as name
        // document id - random hash in trailer, for checking if a PDF document has been modified
        // document creation date (CreationDate)
        // date of last change (ModDate)
        // document title (Title)
        // output intent (OutputIntent)
        // PDF/X conformance key (GTX_PDFXVersion - "PDF/X-3:2002")

        // note: standard rgb is not allowed
        // note: lzw compression is prohibited

        // let random_str: String = ::std::rand::thread_rng().gen_ascii_chars().take(len).collect();

        // xmp metadata

        
        let xmp_metadata = format!(include_str!("../../templates/catalog_xmp_metadata.txt"),
                           create_date, modify_date, metadata_date, document_title, document_id, 
                           instance_id, rendition_class, document_version, pdf_x_version, trapped);

        let stream = Stream(LoStream::new(LoDictionary::from_iter(vec![
                          ("Type", "Metadata".into()),
                          ("Subtype", "XML".into()), ]),
                          xmp_metadata.as_bytes().to_vec() ));

        let catalog = LoDictionary::from_iter(vec![
                      ("Type", "Catalog".into()),
                      ("PageLayout", "OneColumn".into()),
                      ("PageMode", "Use0".into()),
                      ("Pages", Reference(pages_id)),
                      ("Metadata", Reference(self.inner_doc.add_object(stream)) ),
                      ("OutputIntents", Array(vec![Dictionary(LoDictionary::from_iter(vec![
                          ("S", Name("GTS_PDFX".into())),
                          ("OutputCondition", String("Commercial and special offset print acccording  \
                                               to ISO 12647-2:2004 / Amd 1, paper type 1 or 2  \
                                               (matte or gloss-coated offset paper, 115 g/m2), \
                                               screen ruling 60/cm".into(), StringFormat::Literal)),
                          ("Type", Name("OutputIntent".into())),
                          ("OutputConditionIdentifier", String("FOGRA39".into(), StringFormat::Literal)),
                          ("RegistryName", String("www.color.org".into(), StringFormat::Literal)),
                          ("DestinationOutputProfile", Reference(dest_output_profile)),
                          ("Info", String("Coated FOGRA39 (ISO 12647-2:2004)".into(), StringFormat::Literal)), 
                          ])),
                      ])),

                    ]);

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