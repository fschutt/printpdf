use std::collections::BTreeMap;

use crate::IccProfileType;
use crate::PdfDocument;
use crate::PdfDocumentInfo;
use crate::color::IccProfile;
use lopdf::Dictionary as LoDictionary;
use lopdf::Object::*;
use lopdf::StringFormat::{Hexadecimal, Literal};
use lopdf::Stream as LoStream;

pub struct SaveOptions {
    pub optimize: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self { optimize: !(std::cfg!(debug_assertions)) }
    }
}

pub fn serialize_pdf_into_bytes(pdf: &PdfDocument, opts: &SaveOptions) -> Vec<u8> {

    let mut doc = lopdf::Document::with_version("1.3");
    let pages_id = doc.new_object_id();

    let mut catalog = LoDictionary::from_iter(vec![
        ("Type", "Catalog".into()),
        ("PageLayout", "OneColumn".into()),
        ("PageMode", "UseNone".into()),
        ("Pages", Reference(pages_id)),
    ]);

    if pdf.metadata.info.conformance.must_have_icc_profile() {

        /// Default ICC profile, necessary if `PdfMetadata::must_have_icc_profile()` return true
        const ICC_PROFILE_ECI_V2: &[u8] = include_bytes!("../assets/CoatedFOGRA39.icc");

        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
            12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
            offset paper, 115 g/m2), screen ruling 60/cm";     
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = String("FOGRA39".into(), Literal);
        let registry = String("http://www.color.org".into(), Literal);
        let icc = IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), IccProfileType::Cmyk)
            .with_alternate_profile(false)
            .with_range(true);
        let icc_profile_id = doc.add_object(Stream(icc_to_stream(&icc)));
        let output_intents = LoDictionary::from_iter(vec![
            ("S", Name("GTS_PDFX".into())),
            ("OutputCondition", String(icc_profile_descr.into(), Literal)),
            ("Type", Name("OutputIntent".into())),
            ("OutputConditionIdentifier", icc_profile_short),
            ("RegistryName", registry),
            ("Info", String(icc_profile_str.into(), Literal)),
            ("DestinationOutputProfile", Reference(icc_profile_id)),
        ]);
        catalog.set("OutputIntents", Array(vec![Dictionary(output_intents)]));
    }

    if pdf.metadata.info.conformance.must_have_xmp_metadata() {
        let xmp_obj = Stream(LoStream::new(
            LoDictionary::from_iter(vec![
                ("Type", "Metadata".into()), 
                ("Subtype", "XML".into())
            ]),
            pdf.metadata.xmp_metadata_string().as_bytes().to_vec(),
        ));
        let metadata_id = doc.add_object(xmp_obj);
        catalog.set("Metadata", Reference(metadata_id));
    }

    // Pre-allocated IDs of the pages
    let page_ids = pdf.pages.iter()
        .map(|_| doc.new_object_id())
        .collect::<Vec<_>>();

    // Add layers
    if !pdf.resources.layers.map.is_empty() {

        let layer_ids = pdf.resources.layers.map.iter().map(|(id, s)| {

            let usage_ocg_dict = LoDictionary::from_iter(vec![
                ("Type", Name("OCG".into())),
                (
                    "CreatorInfo",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Creator", String(s.creator.clone().into(), Literal)),
                        ("Subtype", Name(s.usage.to_string().into())),
                    ])),
                ),
            ]);
    
            let usage_ocg_dict_ref = doc.add_object(Dictionary(usage_ocg_dict));
            let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);
            let intent_arr_ref = doc.add_object(intent_arr);
    
            let pdf_id = doc.add_object(Dictionary(
                LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    ("Name", String(s.name.to_string().into(), Literal)), // TODO: non-ASCII layer names!
                    ("Intent", Reference(intent_arr_ref)),
                    ("Usage", Reference(usage_ocg_dict_ref)),
                ]),
            ));
    
            (id.clone(), pdf_id)
        }).collect::<BTreeMap<_, _>>();
    
        let flattened_ocg_list = layer_ids
            .values()
            .map(|s| Reference(s.clone()))
            .collect::<Vec<_>>();
    
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
    }

    // Add bookmarks
    if !pdf.bookmarks.map.is_empty() {
        
        let bookmarks_id = doc.new_object_id();
        let mut bookmarks_sorted = pdf.bookmarks.map.iter().collect::<Vec<_>>();
        bookmarks_sorted.sort_by(|(_, v), (_, v2)| {
            (v.page, &v.name).cmp(&(v2.page, &v2.name))
        });
        let bookmarks_sorted = bookmarks_sorted.into_iter().filter_map(|(k, v)| {
            let page_obj_id = page_ids.get(v.page).cloned()?;
            Some((k, &v.name, page_obj_id))
        }).collect::<Vec<_>>();

        let bookmark_ids = bookmarks_sorted.iter().map(|(id, name, page_id)| {
            let newid = doc.new_object_id();
            (id, name, page_id, newid)
        }).collect::<Vec<_>>();
        
        let first = bookmark_ids.first().map(|s| s.3).unwrap();
        let last = bookmark_ids.last().map(|s| s.3).unwrap();
        for (i, (_id, name, pageid, self_id)) in bookmark_ids.iter().enumerate() {
            let prev = if i == 0 { None } else { bookmark_ids.get(i - 1).map(|s| s.3.clone()) };
            let next = bookmark_ids.get(i + 1).map(|s| s.3.clone());
            let dest = Array(vec![
                Reference((*pageid).clone()),
                "XYZ".into(),
                Null,
                Null,
                Null,
            ]);
            let mut dict = LoDictionary::from_iter(vec![
                ("Parent", Reference(bookmarks_id)),
                ("Title", String(name.to_string().into(), Literal)),
                ("Dest", dest),
            ]);
            if let Some(prev) = prev {
                dict.set("Prev", Reference(prev));
            }
            if let Some(next) = next {
                dict.set("Next", Reference(next));
            }
            doc.set_object(*self_id, dict);
        }

        let bookmarks_list = LoDictionary::from_iter(vec![
            ("Type", "Outlines".into()),
            ("Count", Integer(pdf.bookmarks.map.len() as i64)),
            ("First", Reference(first)),
            ("Last", Reference(last)),
        ]);

        doc.set_object(bookmarks_id, bookmarks_list);
        catalog.set("Outlines", Reference(bookmarks_id));
        catalog.set("PageMode", String("UseOutlines".into(), Literal));
    }

    // Add fonts and other resources
    /*
    pdf.resources.extgstates
    pdf.resources.fonts

    let xobjects_dict: lopdf::Dictionary = pdf.resources.xobjects.into_with_document(doc);
    let graphics_state_dict: lopdf::Dictionary = pdf.resources.extgstates.into();
    let annotations_dict: lopdf::Dictionary = link_annot_to_dict(pdf.resources.links);

    if !xobjects_dict.is_empty() {
        dict.set("XObject", lopdf::Object::Dictionary(xobjects_dict));
    }

    if !graphics_state_dict.is_empty() {
        dict.set("ExtGState", lopdf::Object::Dictionary(graphics_state_dict));
    }

    if !annotations_dict.is_empty() {
        dict.set("Annots", lopdf::Object::Dictionary(annotations_dict))
    }
    */

    /*
    // add all pages with contents
    let mut page_ids = Vec::<LoObject>::new();

    // add fonts (shared resources)
    let mut font_dict_id = None;

    // add all fonts / other resources shared in the whole document
    let fonts_dict: lopdf::Dictionary = doc
        .fonts
        .into_with_document(&mut doc.inner_doc, &mut doc.pages);

    if !fonts_dict.is_empty() {
        font_dict_id = Some(doc.inner_doc.add_object(Dictionary(fonts_dict)));
    }
    */

    //-- END

    let pages = LoDictionary::from_iter(vec![
        ("Type", "Pages".into()),
        ("Count", Integer(page_ids.len() as i64)),
        ("Kids", Array(page_ids.iter().map(|q| Reference(q.clone())).collect::<Vec<_>>())),
    ]);

    doc.objects.insert(pages_id, Dictionary(pages));

    let catalog_id = doc.add_object(catalog);
    let instance_id = crate::utils::random_character_string_32();
    let document_id = crate::utils::random_character_string_32();

    let document_info_id = doc.add_object(Dictionary(docinfo_to_dict(&pdf.metadata.info)));

    doc.trailer.set("Root", Reference(catalog_id));
    doc.trailer.set("Info", Reference(document_info_id));
    doc.trailer.set(
        "ID",
        Array(vec![
            String(document_id.as_bytes().to_vec(), Literal),
            String(instance_id.as_bytes().to_vec(), Literal),
        ]),
    );

    if opts.optimize {
        doc.compress();
    }

    let mut bytes = Vec::new();
    let mut writer = std::io::BufWriter::new(&mut bytes);
    let _ = doc.save_to(&mut writer);
    std::mem::drop(writer);

    bytes
}


fn docinfo_to_dict(m: &PdfDocumentInfo) -> LoDictionary {

    let trapping = if m.trapped { "True" } else { "False" };
    let gts_pdfx_version = m.conformance.get_identifier_string();

    let info_mod_date = crate::utils::to_pdf_time_stamp_metadata(&m.modification_date);
    let info_create_date = crate::utils::to_pdf_time_stamp_metadata(&m.creation_date);

    let creation_date = String(info_create_date.into_bytes(), Literal);
    let title = String(m.document_title.to_string().as_bytes().to_vec(), Literal);
    let identifier = String(m.identifier.as_bytes().to_vec(), Literal);
    let keywords = String(m.keywords.join(",").as_bytes().to_vec(), Literal);

    LoDictionary::from_iter(vec![
        ("Trapped", trapping.into()),
        ("CreationDate", creation_date),
        ("ModDate", String(info_mod_date.into_bytes(), Literal)),
        ("GTS_PDFXVersion", String(gts_pdfx_version.into(), Literal)),
        ("Title", title),
        ("Author", String(m.author.as_bytes().to_vec(), Literal)),
        ("Creator", String(m.creator.as_bytes().to_vec(), Literal)),
        ("Producer", String(m.producer.as_bytes().to_vec(), Literal)),
        ("Subject", String(m.subject.as_bytes().to_vec(), Literal)),
        ("Identifier", identifier),
        ("Keywords", keywords),
    ])
}

fn icc_to_stream(val: &IccProfile) -> LoStream {
    use lopdf::Object::*;
    use lopdf::{Dictionary as LoDictionary, Stream as LoStream};

    let (num_icc_fields, alternate) = match val.icc_type {
        IccProfileType::Cmyk => (4, "DeviceCMYK"),
        IccProfileType::Rgb => (3, "DeviceRGB"),
        IccProfileType::Greyscale => (1, "DeviceGray"),
    };

    let mut stream_dict = LoDictionary::from_iter(vec![
        ("N", Integer(num_icc_fields)),
        ("Length", Integer(val.icc.len() as i64)),
    ]);

    if val.has_alternate {
        stream_dict.set("Alternate", Name(alternate.into()));
    }

    if val.has_range {
        stream_dict.set(
            "Range",
            Array(vec![
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
                Real(0.0),
                Real(1.0),
            ]),
        );
    }

    LoStream::new(stream_dict, val.icc.clone())
}