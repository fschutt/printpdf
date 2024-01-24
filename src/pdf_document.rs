//! A `PDFDocument` represents the whole content of the file

use crate::utils::random_character_string_32;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::rc::Rc;

use crate::OffsetDateTime;
use lopdf;

use crate::indices::*;
use crate::{
    BuiltinFont, DirectFontRef, Error, ExternalFont, Font, FontData, FontList, IccProfileList, IndirectFontRef, Mm,
    PdfConformance, PdfMetadata, PdfPage, PdfPageReference,
};

/// PDF document
#[derive(Debug, Clone)]
pub struct PdfDocument {
    /// Pages of the document
    pub(super) pages: Vec<PdfPage>,
    /// Fonts used in this document
    pub fonts: FontList,
    /// ICC profiles used in the document
    pub(super) _icc_profiles: IccProfileList,
    /// Inner PDF document
    pub(super) inner_doc: lopdf::Document,
    /// Document ID. Must be changed if the document is loaded / parsed from a file
    pub document_id: String,
    /// Metadata for this document
    pub metadata: PdfMetadata,
    /// The bookmarks in the document. A HashMap<Page Number, Bookmark Name>
    pub bookmarks: HashMap<usize, String>,
}

/// Marker struct for a document. Used to make the API a bit nicer.
/// It simply calls `PdfDocument` functions.
pub struct PdfDocumentReference {
    /// A wrapper for a document, so actions from outside this library
    /// are restricted to functions inside this crate (only functions in `lopdf`
    /// can directly manipulate the document)
    pub(crate) document: Rc<RefCell<PdfDocument>>,
}

impl PdfDocument {
    /// Creates a new PDF document
    #[allow(clippy::new_ret_no_self)]
    #[inline]
    pub fn new<S1, S2>(
        document_title: S1,
        initial_page_width: Mm,
        initial_page_height: Mm,
        initial_layer_name: S2,
    ) -> (PdfDocumentReference, PdfPageIndex, PdfLayerIndex)
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let doc = Self {
            pages: Vec::new(),
            document_id: random_character_string_32(),
            fonts: FontList::new(),
            _icc_profiles: IccProfileList::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            metadata: PdfMetadata::new(document_title, 1, false, PdfConformance::default()),
            bookmarks: HashMap::new(),
        };

        let doc_ref = Rc::new(RefCell::new(doc));

        let (initial_page, layer_index) = PdfPage::new(initial_page_width, initial_page_height, initial_layer_name, 0);

        {
            doc_ref.borrow_mut().pages.push(initial_page);
        }

        (PdfDocumentReference { document: doc_ref }, PdfPageIndex(0), layer_index)
    }

    pub fn empty<S: Into<String>>(document_title: S) -> PdfDocumentReference {
        let doc = Self {
            pages: Vec::new(),
            document_id: random_character_string_32(),
            fonts: FontList::new(),
            _icc_profiles: IccProfileList::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            metadata: PdfMetadata::new(document_title, 1, false, PdfConformance::X3_2002_PDF_1_3),
            bookmarks: HashMap::new(),
        };

        let doc_ref = Rc::new(RefCell::new(doc));
        PdfDocumentReference { document: doc_ref }
    }
}

macro_rules! implement_adding_fonts {
    ($self:expr, $font_name:expr, $font:expr) => {{
        let font_ref;

        let possible_ref = {
            let doc = $self.document.borrow();
            font_ref = IndirectFontRef::new($font_name);
            match doc.fonts.get_font(&font_ref) {
                Some(f) => Some(f.clone()),
                None => None,
            }
        };

        if possible_ref.is_some() {
            Ok(font_ref)
        } else {
            let mut doc = $self.document.borrow_mut();
            let direct_ref = DirectFontRef {
                inner_obj: doc.inner_doc.new_object_id(),
                data: $font,
            };

            doc.fonts.add_font(font_ref.clone(), direct_ref);
            Ok(font_ref)
        }
    }};
}

impl PdfDocumentReference {
    // ----- BUILDER FUNCTIONS

    /// Changes the title on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_title<S>(self, new_title: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.document_title = new_title.into();
        self
    }

    /// Changes the author metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_author<S>(self, author: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.author = author.into();
        self
    }

    /// Changes the creator metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_creator<S>(self, creator: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.creator = creator.into();
        self
    }

    /// Changes the producer/publisher metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_producer<S>(self, producer: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.producer = producer.into();
        self
    }

    /// Changes the keywords metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_keywords<S>(self, keywords: Vec<S>) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.keywords = keywords.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Changes the subject metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_subject<S>(self, subject: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.subject = subject.into();
        self
    }

    /// Changes the subject metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_identifier<S>(self, identifier: S) -> Self
    where
        S: Into<String>,
    {
        self.document.borrow_mut().metadata.identifier = identifier.into();
        self
    }

    /// Set the trapping of the document
    #[inline]
    pub fn with_trapping(self, trapping: bool) -> Self {
        self.document.borrow_mut().metadata.trapping = trapping;
        self
    }

    /// Sets the document ID (for comparing two PDF documents for equality)
    #[inline]
    pub fn with_document_id(self, id: String) -> Self {
        self.document.borrow_mut().metadata.xmp_metadata.document_id = id;
        self
    }

    /// Set the version of the document
    #[inline]
    pub fn with_document_version(self, version: u32) -> Self {
        self.document.borrow_mut().metadata.document_version = version;
        self
    }

    /// Changes the conformance of this document. It is recommended to call
    /// `check_for_errors()` after changing it.
    #[inline]
    pub fn with_conformance(self, conformance: PdfConformance) -> Self {
        self.document.borrow_mut().metadata.conformance = conformance;
        self
    }

    /// Sets the creation date on the document.
    ///
    /// Per default, the creation date is set to the current time.
    #[inline]
    pub fn with_creation_date(self, creation_date: OffsetDateTime) -> Self {
        self.document.borrow_mut().metadata.creation_date = creation_date;
        self
    }

    /// Sets the metadata date on the document.
    ///
    /// By default, the metadata date is set to the current time.
    #[inline]
    pub fn with_metadata_date(self, metadata_date: OffsetDateTime) -> Self {
        self.document.borrow_mut().metadata.metadata_date = metadata_date;
        self
    }

    /// Sets the modification date on the document. Intended to be used when
    /// reading documents that already have a modification date.
    #[inline]
    pub fn with_mod_date(self, mod_date: OffsetDateTime) -> Self {
        self.document.borrow_mut().metadata.modification_date = mod_date;
        self
    }

    // ----- ADD FUNCTIONS

    /// Create a new pdf page and returns the index of the page
    #[inline]
    pub fn add_page<S>(&self, x_mm: Mm, y_mm: Mm, inital_layer_name: S) -> (PdfPageIndex, PdfLayerIndex)
    where
        S: Into<String>,
    {
        let mut doc = self.document.borrow_mut();
        let (pdf_page, pdf_layer_index) = PdfPage::new(x_mm, y_mm, inital_layer_name, doc.pages.len());
        doc.pages.push(pdf_page);
        let page_index = PdfPageIndex(doc.pages.len() - 1);
        (page_index, pdf_layer_index)
    }
    /// Create a new pdf page and returns the index of the page.
    /// If the page already has a bookmark, overwrites it.
    #[inline]
    pub fn add_bookmark<S>(&self, name: S, page: PdfPageIndex)
    where
        S: Into<String>,
    {
        let mut doc = self.document.borrow_mut();
        doc.bookmarks.insert(page.0, name.into());
    }
    /// Add a font from a font stream
    pub fn add_external_font<R>(&self, font_stream: R) -> ::std::result::Result<IndirectFontRef, Error>
    where
        R: ::std::io::Read,
    {
        let last_font_index = {
            let doc = self.document.borrow();
            doc.fonts.len()
        };
        let external_font = ExternalFont::new(font_stream, last_font_index)?;
        let external_font_name = external_font.face_name.clone();
        let font = Font::ExternalFont(external_font);
        implement_adding_fonts!(self, external_font_name, font)
    }

    /// Add a font from a font stream and set the whether or not to allow subsetting the font
    #[cfg(feature = "font_subsetting")]
    pub fn add_external_font_with_subsetting<R>(
        &self,
        font_stream: R,
        allow_subsetting: bool,
    ) -> ::std::result::Result<IndirectFontRef, Error>
    where
        R: ::std::io::Read,
    {
        let font = self.add_external_font(font_stream);
        if let Ok(font) = &font {
            match self.document.borrow().fonts.get_font(font).unwrap().data {
                Font::ExternalFont(ex_font) => {
                    *ex_font.allow_subsetting.borrow_mut() = allow_subsetting;
                }
                Font::BuiltinFont(_) => unreachable!(),
            }
        }
        font
    }

    /// Add a font from a custom font backend
    pub fn add_external_font_data<F>(&self, bytes: Vec<u8>, data: F) -> Result<IndirectFontRef, Error>
    where
        F: FontData + 'static,
    {
        let last_font_index = {
            let doc = self.document.borrow();
            doc.fonts.len()
        };
        let external_font = ExternalFont::with_font_data(bytes, last_font_index, Box::new(data));
        let external_font_name = external_font.face_name.clone();
        let font = Font::ExternalFont(external_font);
        implement_adding_fonts!(self, external_font_name, font)
    }

    /// Add a font from a custom font backend and set the whether or not to allow subsetting the
    /// font
    #[cfg(feature = "font_subsetting")]
    pub fn add_external_font_data_with_subsetting<F>(
        &self,
        bytes: Vec<u8>,
        data: F,
        allow_subsetting: bool,
    ) -> Result<IndirectFontRef, Error>
    where
        F: FontData + 'static,
    {
        let font = self.add_external_font_data(bytes, data);
        if let Ok(font) = &font {
            match self.document.borrow().fonts.get_font(font).unwrap().data {
                Font::ExternalFont(ex_font) => {
                    *ex_font.allow_subsetting.borrow_mut() = allow_subsetting;
                }
                Font::BuiltinFont(_) => unreachable!(),
            }
        }
        font
    }

    /// Add a built-in font to the document
    ///
    /// Built-in fonts can only be used to print characters that are supported by the
    /// [Windows-1252][] encoding.  All other characters will be ignored.
    ///
    /// [Windows-1252]: https://en.wikipedia.org/wiki/Windows-1252
    pub fn add_builtin_font(&self, builtin_font: BuiltinFont) -> ::std::result::Result<IndirectFontRef, Error> {
        let builtin_font_name: &'static str = builtin_font.into();
        implement_adding_fonts!(self, builtin_font_name, Font::BuiltinFont(builtin_font))
    }

    // ----- GET FUNCTIONS

    /// Returns the page (for inserting content)
    #[inline]

    pub fn get_page(&self, page: PdfPageIndex) -> PdfPageReference {
        let _ = &self.document.borrow_mut().pages[page.0];
        PdfPageReference {
            document: Rc::downgrade(&self.document),
            page,
        }
    }

    /// Returns a direct reference (object ID) to the font from an
    /// indirect reference (postscript name)
    #[inline]
    pub fn get_font(&self, font: &IndirectFontRef) -> Option<DirectFontRef> {
        let doc = self.document.borrow();
        doc.fonts.get_font(font)
    }

    /// Drops the PDFDocument, returning the inner `lopdf::Document`.
    /// # Safety
    /// Document may be only half-written, use only in extreme cases
    #[inline]
    pub unsafe fn get_inner(self) -> lopdf::Document {
        let doc = Rc::try_unwrap(self.document).unwrap().into_inner();
        doc.inner_doc
    }

    // --- MISC FUNCTIONS

    /// Checks for invalid settings in the document
    pub fn check_for_errors(&self) -> ::std::result::Result<(), Error> {
        // TODO
        #[cfg(feature = "logging")]
        {
            warn!("Checking PDFs for errors is currently not supported!");
        }

        Ok(())
    }

    /// Tries to match the document to the given conformance.
    /// Errors only on an unrecoverable error.
    pub fn repair_errors(&self, _conformance: PdfConformance) -> ::std::result::Result<(), Error> {
        // TODO
        #[cfg(feature = "logging")]
        {
            warn!("Reparing PDFs is currently not supported!");
        }

        Ok(())
    }

    /// Save PDF document to bytes
    #[allow(unused_qualifications)]
    pub fn save_to_bytes(self) -> Result<Vec<u8>, Error> {
        use lopdf::Object::*;
        use lopdf::StringFormat::Literal;
        use lopdf::{Dictionary as LoDictionary, Object as LoObject};
        use std::mem;

        // todo: remove unwrap, handle error
        let mut doc = Rc::try_unwrap(self.document).unwrap().into_inner();
        let pages_id = doc.inner_doc.new_object_id();
        let bookmarks_id = doc.inner_doc.new_object_id();
        let mut bookmarks_list = LoDictionary::from_iter(vec![
            ("Type", "Outlines".into()),
            ("Count", Integer(doc.bookmarks.len() as i64)),
            /* First and Last will be filled in once they are created from the pages */
        ]);

        // extra pdf infos
        let (xmp_metadata, document_info, icc_profile) = doc.metadata.clone().into_obj();

        let xmp_metadata_id = match xmp_metadata {
            Some(metadata) => Some(doc.inner_doc.add_object(metadata)),
            None => None,
        };

        let document_info_id = doc.inner_doc.add_object(document_info);

        // add catalog
        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = "FOGRA39";

        let mut output_intents = LoDictionary::from_iter(vec![
            ("S", Name("GTS_PDFX".into())),
            ("OutputCondition", String(icc_profile_descr.into(), Literal)),
            ("Type", Name("OutputIntent".into())),
            ("OutputConditionIdentifier", String(icc_profile_short.into(), Literal)),
            ("RegistryName", String("http://www.color.org".into(), Literal)),
            ("Info", String(icc_profile_str.into(), Literal)),
        ]);

        let mut catalog = LoDictionary::from_iter(vec![
            ("Type", "Catalog".into()),
            ("PageLayout", "OneColumn".into()),
            (
                "PageMode",
                if !doc.bookmarks.is_empty() {
                    "UseOutlines"
                } else {
                    "UseNone"
                }
                .into(),
            ),
            ("Outlines", Reference(bookmarks_id)),
            ("Pages", Reference(pages_id)),
        ]);

        if let Some(profile) = icc_profile {
            let icc_profile: lopdf::Stream = profile.into();
            let icc_profile_id = doc.inner_doc.add_object(Stream(icc_profile));
            output_intents.set("DestinationOutputProfile", Reference(icc_profile_id));
            catalog.set("OutputIntents", Array(vec![Dictionary(output_intents)]));
        }

        if let Some(metadata_id) = xmp_metadata_id {
            catalog.set("Metadata", Reference(metadata_id));
        }

        let mut pages = LoDictionary::from_iter(vec![
            ("Type", "Pages".into()),
            ("Count", Integer(doc.pages.len() as i64)),
            /* Kids and Resources missing */
        ]);

        // add all pages with contents
        let mut page_ids = Vec::<LoObject>::new();

        // ----- OCG CONTENT

        // page index + page names to add the OCG to the /Catalog
        let page_layer_names: Vec<(usize, Vec<::std::string::String>)> = doc
            .pages
            .iter()
            .map(|page| (page.index, page.layers.iter().map(|layer| layer.name.clone()).collect()))
            .collect();

        // add optional content groups (layers) to the /Catalog
        let usage_ocg_dict = LoDictionary::from_iter(vec![
            ("Type", Name("OCG".into())),
            (
                "CreatorInfo",
                Dictionary(LoDictionary::from_iter(vec![
                    ("Creator", String("Adobe Illustrator 14.0".into(), Literal)),
                    ("Subtype", Name("Artwork".into())),
                ])),
            ),
        ]);

        let usage_ocg_dict_ref = doc.inner_doc.add_object(Dictionary(usage_ocg_dict));

        let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);

        let intent_arr_ref = doc.inner_doc.add_object(intent_arr);

        // page index, layer index, reference to OCG dictionary
        let ocg_list: Vec<(usize, Vec<(usize, lopdf::Object)>)> = page_layer_names
            .into_iter()
            .map(|(page_idx, layer_names)| {
                (
                    page_idx,
                    layer_names
                        .into_iter()
                        .enumerate()
                        .map(|(layer_idx, layer_name)| {
                            (
                                layer_idx,
                                Reference(doc.inner_doc.add_object(Dictionary(LoDictionary::from_iter(vec![
                                    ("Type", Name("OCG".into())),
                                    ("Name", String(layer_name.into(), Literal)),
                                    ("Intent", Reference(intent_arr_ref)),
                                    ("Usage", Reference(usage_ocg_dict_ref)),
                                ])))),
                            )
                        })
                        .collect(),
                )
            })
            .collect();

        let flattened_ocg_list: Vec<lopdf::Object> = ocg_list
            .iter()
            .flat_map(|(_, layers)| layers.iter().map(|(_, obj)| obj.clone()))
            .collect();

        catalog.set(
            "OCProperties",
            Dictionary(LoDictionary::from_iter(vec![
                ("OCGs", Array(flattened_ocg_list.clone())),
                // optional content configuration dictionary, page 376
                (
                    "D",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Order", Array(flattened_ocg_list.clone())),
                        // "radio button groups"
                        ("RBGroups", Array(vec![])),
                        // initially visible OCG
                        ("ON", Array(flattened_ocg_list)),
                    ])),
                ),
            ])),
        );

        // ----- END OCG CONTENT (on document level)

        // ----- PAGE CONTENT

        // add fonts (shared resources)
        let mut font_dict_id = None;

        // add all fonts / other resources shared in the whole document
        let fonts_dict: lopdf::Dictionary = doc.fonts.into_with_document(&mut doc.inner_doc, &mut doc.pages);

        if !fonts_dict.is_empty() {
            font_dict_id = Some(doc.inner_doc.add_object(Dictionary(fonts_dict)));
        }

        let mut page_id_to_obj: HashMap<usize, (u32, u16)> = HashMap::new();

        for (idx, page) in doc.pages.into_iter().enumerate() {
            let annotation_ids = page
                .resources
                .link_annotations
                .clone()
                .into_iter()
                .map(|(_, annotation)| doc.inner_doc.add_object(annotation))
                .collect::<Vec<_>>();

            let mut p = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("Rotate", Integer(0)),
                (
                    "MediaBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                (
                    "TrimBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                (
                    "CropBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                (
                    "Annots",
                    annotation_ids
                        .iter()
                        .map(|id| Reference(*id))
                        .collect::<Vec<LoObject>>()
                        .into(),
                ),
                ("Parent", Reference(pages_id)),
            ]);

            if let Some(extension) = &page.extend_with {
                for (key, value) in extension.iter() {
                    p.set(key.to_vec(), value.clone());
                }
            }

            // this will collect the resources needed for rendering this page
            let layers_temp = ocg_list.iter().find(|e| e.0 == idx).unwrap();
            let (mut resources_page, layer_streams) =
                page.collect_resources_and_streams(&mut doc.inner_doc, &layers_temp.1);

            if let Some(f) = font_dict_id {
                resources_page.set("Font", Reference(f));
            }

            if !resources_page.is_empty() {
                let resources_page_id = doc.inner_doc.add_object(Dictionary(resources_page));
                p.set("Resources", Reference(resources_page_id));
            }

            // merge all streams of the individual layers into one big stream
            let mut layer_streams_merged_vec = Vec::<u8>::new();
            for mut stream in layer_streams {
                layer_streams_merged_vec.append(&mut stream.content);
            }

            let merged_layer_stream = lopdf::Stream::new(lopdf::Dictionary::new(), layer_streams_merged_vec);
            let page_content_id = doc.inner_doc.add_object(merged_layer_stream);

            p.set("Contents", Reference(page_content_id));
            let page_obj = doc.inner_doc.add_object(p);
            if doc.bookmarks.contains_key(&idx) {
                page_id_to_obj.insert(idx, page_obj);
            }
            page_ids.push(Reference(page_obj));
        }

        if !doc.bookmarks.is_empty() {
            let len = doc.bookmarks.len();
            if len == 1 {
                let page_index = doc.bookmarks.iter().next().unwrap().0.to_owned();
                let title = doc.bookmarks.iter().next().unwrap().1.to_owned();
                let obj_ref = doc.inner_doc.add_object(Dictionary(LoDictionary::from_iter(vec![
                    ("Parent", Reference(bookmarks_id)),
                    ("Title", String(title.into(), Literal)),
                    (
                        "Dest",
                        Array(vec![
                            Reference(page_id_to_obj.get(&page_index).unwrap().to_owned()),
                            "XYZ".into(),
                            Null,
                            Null,
                            Null,
                        ]),
                    ),
                ])));
                bookmarks_list.set("First", Reference(obj_ref));
                bookmarks_list.set("Last", Reference(obj_ref));
            } else {
                let mut sorted_bmarks: Vec<(&usize, &std::string::String)> = doc.bookmarks.iter().collect();
                sorted_bmarks.sort();
                for (i, (page_index, b_name)) in sorted_bmarks.iter().enumerate() {
                    let dest = (
                        "Dest",
                        Array(vec![
                            Reference(page_id_to_obj.get(page_index).unwrap().to_owned()),
                            "XYZ".into(),
                            Null,
                            Null,
                            Null,
                        ]),
                    );
                    doc.inner_doc.add_object(Dictionary(LoDictionary::from_iter(if i == 0 {
                        bookmarks_list.set("First", Reference((doc.inner_doc.max_id + 1, 0)));
                        vec![
                            ("Parent", Reference(bookmarks_id)),
                            ("Title", String(b_name.to_owned().to_owned().into(), Literal)),
                            ("Next", Reference((doc.inner_doc.max_id + 2, 0))),
                            dest,
                        ]
                    } else if i == len - 1 {
                        bookmarks_list.set("Last", Reference((doc.inner_doc.max_id + 1, 0)));
                        vec![
                            ("Parent", Reference(bookmarks_id)),
                            ("Title", String(b_name.to_owned().to_owned().into(), Literal)),
                            ("Prev", Reference((doc.inner_doc.max_id, 0))),
                            dest,
                        ]
                    } else {
                        vec![
                            ("Parent", Reference(bookmarks_id)),
                            ("Title", String(b_name.to_owned().to_owned().into(), Literal)),
                            ("Prev", Reference((doc.inner_doc.max_id, 0))),
                            ("Next", Reference((doc.inner_doc.max_id + 2, 0))),
                            dest,
                        ]
                    })));
                }
            }
        }

        pages.set::<_, LoObject>("Kids".to_string(), page_ids.into());

        // ----- END PAGE CONTENT

        doc.inner_doc.objects.insert(pages_id, Dictionary(pages));
        doc.inner_doc.objects.insert(bookmarks_id, Dictionary(bookmarks_list));

        // save inner document
        let catalog_id = doc.inner_doc.add_object(catalog);
        let instance_id = random_character_string_32();

        doc.inner_doc.trailer.set("Root", Reference(catalog_id));
        doc.inner_doc.trailer.set("Info", Reference(document_info_id));
        doc.inner_doc.trailer.set(
            "ID",
            Array(vec![
                String(doc.document_id.as_bytes().to_vec(), Literal),
                String(instance_id.as_bytes().to_vec(), Literal),
            ]),
        );

        // does nothing in debug mode, optimized in release mode
        Self::optimize(&mut doc.inner_doc);
        let mut bytes = Vec::new();
        let mut writer = BufWriter::new(&mut bytes);
        doc.inner_doc.save_to(&mut writer)?;
        mem::drop(writer);

        Ok(bytes)
    }

    /// Save PDF Document, writing the contents to the target
    pub fn save<W: Write>(self, target: &mut BufWriter<W>) -> Result<(), Error> {
        Ok(target.write_all(&self.save_to_bytes()?)?)
    }

    #[cfg(any(debug_assertions, feature = "less-optimization"))]
    #[inline]
    fn optimize(_: &mut lopdf::Document) {}

    #[cfg(all(not(debug_assertions), not(feature = "less-optimization")))]
    #[inline]
    fn optimize(doc: &mut lopdf::Document) {
        doc.prune_objects();
        doc.delete_zero_length_streams();
        doc.compress();
    }
}
