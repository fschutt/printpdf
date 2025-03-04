use std::collections::{BTreeMap, BTreeSet};

use lopdf::{
    Dictionary as LoDictionary,
    Object::{Array, Dictionary, Integer, Name, Null, Real, Reference, Stream, String as LoString},
    Stream as LoStream,
    StringFormat::{Hexadecimal, Literal},
    content::Operation as LoOp,
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    Actions, BuiltinFont, Color, ColorArray, Destination, FontId, IccProfileType,
    ImageOptimizationOptions, Line, LinkAnnotation, Op, PaintMode, ParsedFont, PdfDocument,
    PdfDocumentInfo, PdfPage, PdfResources, PdfWarnMsg, Polygon, TextItem, XObject, XObjectId,
    color::IccProfile, font::SubsetFont,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename = "camelCase")]
pub struct PdfSaveOptions {
    /// If set to true (default), compresses streams and
    /// prunes unreferenced PDF objects. Set to false for debugging
    #[serde(default = "default_optimize")]
    pub optimize: bool,
    /// Whether to include the entire font or to subset it.
    /// Default is set to true because some CJK fonts can be massive.
    #[serde(default = "default_subset_fonts")]
    pub subset_fonts: bool,
    /// Whether to ignore unknown operations. If set to true
    /// (default), will skip any unknown PDF operations when serializing the file.
    #[serde(default = "default_secure")]
    pub secure: bool,
    /// Image optimization options
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_optimization: Option<ImageOptimizationOptions>,
}

const fn default_optimize() -> bool {
    true
}
const fn default_subset_fonts() -> bool {
    true
}
const fn default_secure() -> bool {
    true
}

impl Default for PdfSaveOptions {
    fn default() -> Self {
        Self {
            optimize: default_optimize(),
            subset_fonts: default_subset_fonts(),
            secure: default_secure(),
            image_optimization: Some(ImageOptimizationOptions::default()),
        }
    }
}

// Initializes the image resources and the document
//
// Note: this function may become async later on!
pub fn init_doc_and_resources(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
) -> (lopdf::Document, lopdf::Dictionary) {
    let mut doc = lopdf::Document::with_version("1.3");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;

    let mut global_xobject_dict = LoDictionary::new();
    for (k, v) in pdf.resources.xobjects.map.iter() {
        global_xobject_dict.set(
            k.0.clone(),
            crate::xobject::add_xobject_to_document(v, &mut doc, opts.image_optimization.as_ref()),
        );
    }

    (doc, global_xobject_dict)
}

pub fn serialize_pdf_into_bytes(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let (mut doc, global_xobject_dict) = init_doc_and_resources(pdf, opts);
    let pages_id = doc.new_object_id();
    let mut catalog = LoDictionary::from_iter(vec![
        ("Type", "Catalog".into()),
        ("PageLayout", "OneColumn".into()),
        ("PageMode", "UseNone".into()),
        ("Pages", Reference(pages_id)),
    ]);

    // (Optional): Add OutputIntents to catalog
    if pdf.metadata.info.conformance.must_have_icc_profile() {
        /// Default ICC profile, necessary if `PdfMetadata::must_have_icc_profile()` return true
        const ICC_PROFILE_ECI_V2: &[u8] = include_bytes!("./res/CoatedFOGRA39.icc");
        const ICC_PROFILE_LICENSE: &str = include_str!("./res/CoatedFOGRA39.icc.LICENSE.txt");

        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = LoString("FOGRA39".into(), Literal);
        let registry = LoString("http://www.color.org".into(), Literal);
        let icc = IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), IccProfileType::Cmyk)
            .with_alternate_profile(false)
            .with_range(true);
        let icc_profile_id = doc.add_object(Stream(icc_to_stream(&icc)));
        let output_intents = LoDictionary::from_iter(vec![
            ("S", Name("GTS_PDFX".into())),
            (
                "OutputCondition",
                LoString(icc_profile_descr.into(), Literal),
            ),
            ("License", LoString(ICC_PROFILE_LICENSE.into(), Literal)),
            ("Type", Name("OutputIntent".into())),
            ("OutputConditionIdentifier", icc_profile_short),
            ("RegistryName", registry),
            ("Info", LoString(icc_profile_str.into(), Literal)),
            ("DestinationOutputProfile", Reference(icc_profile_id)),
        ]);
        catalog.set("OutputIntents", Array(vec![Dictionary(output_intents)]));
    }

    // (Optional): Add XMP Metadata to catalog
    if pdf.metadata.info.conformance.must_have_xmp_metadata() {
        let xmp_obj = Stream(LoStream::new(
            LoDictionary::from_iter(vec![("Type", "Metadata".into()), ("Subtype", "XML".into())]),
            pdf.metadata.xmp_metadata_string().as_bytes().to_vec(),
        ));
        let metadata_id = doc.add_object(xmp_obj);
        catalog.set("Metadata", Reference(metadata_id));
    }

    // (Optional): Add "OCProperties" (layers) to catalog
    // Build a mapping from each layer's internal id to a single OCG object ID.
    let layer_ids = if !pdf.resources.layers.map.is_empty() {
        let map = pdf
            .resources
            .layers
            .map
            .iter()
            .map(|(id, layer)| {
                let usage_ocg_dict = LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    (
                        "CreatorInfo",
                        Dictionary(LoDictionary::from_iter(vec![
                            ("Creator", LoString(layer.creator.clone().into(), Literal)),
                            ("Subtype", Name(layer.usage.to_string().into())),
                        ])),
                    ),
                ]);
                let usage_ocg_dict_ref = doc.add_object(Dictionary(usage_ocg_dict));
                let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);
                let intent_arr_ref = doc.add_object(intent_arr);
                let pdf_id = doc.add_object(Dictionary(LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    ("Name", LoString(layer.name.to_string().into(), Literal)),
                    ("Intent", Reference(intent_arr_ref)),
                    ("Usage", Reference(usage_ocg_dict_ref)),
                ])));
                (id.clone(), pdf_id)
            })
            .collect::<BTreeMap<_, _>>();
        let flattened_ocg_list = map.values().map(|s| Reference(*s)).collect::<Vec<_>>();
        catalog.set(
            "OCProperties",
            Dictionary(LoDictionary::from_iter(vec![
                ("OCGs", Array(flattened_ocg_list.clone())),
                (
                    "D",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Order", Array(flattened_ocg_list.clone())),
                        ("RBGroups", Array(vec![])),
                        ("ON", Array(flattened_ocg_list)),
                    ])),
                ),
            ])),
        );
        Some(map)
    } else {
        None
    };

    // Build fonts dictionary
    let mut global_font_dict = LoDictionary::new();
    let prepared_fonts = prepare_fonts(&pdf.resources, &pdf.pages, warnings);
    for (font_id, prepared) in prepared_fonts.iter() {
        let font_dict = add_font_to_pdf(&mut doc, font_id, prepared);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(font_id.0.clone(), Reference(font_dict_id));
    }

    for internal_font in get_used_internal_fonts(&pdf.pages) {
        let font_dict = builtin_font_to_dict(&internal_font);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(internal_font.get_pdf_id(), Reference(font_dict_id));
    }
    let global_font_dict_id = doc.add_object(global_font_dict);

    let global_xobject_dict_id = doc.add_object(global_xobject_dict);

    let mut global_extgstate_dict = LoDictionary::new();
    for (k, v) in pdf.resources.extgstates.map.iter() {
        global_extgstate_dict.set(k.0.clone(), crate::graphics::extgstate_to_dict(v));
    }
    let global_extgstate_dict_id = doc.add_object(global_extgstate_dict);

    let page_ids_reserved = pdf
        .pages
        .iter()
        .map(|_| doc.new_object_id())
        .collect::<Vec<_>>();

    // Render pages
    let page_ids = pdf
        .pages
        .iter()
        .zip(page_ids_reserved.iter())
        .map(|(page, page_id)| {
            let mut page_resources = LoDictionary::new();

            // Instead of re-creating new OCG dictionaries here,
            // re-use the objects from the global layer_ids mapping.
            if let Some(ref layer_ids) = layer_ids {
                let page_layers = page
                    .ops
                    .iter()
                    .filter_map(|op| {
                        if let Op::BeginLayer { layer_id } = op {
                            layer_ids
                                .get(layer_id)
                                .map(|ocg_obj_id| (layer_id.0.clone(), *ocg_obj_id))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if !page_layers.is_empty() {
                    page_resources.set(
                        "Properties",
                        LoDictionary::from_iter(
                            page_layers
                                .iter()
                                .map(|(name, ocg_obj_id)| (name.as_str(), Reference(*ocg_obj_id))),
                        ),
                    );
                }
            }

            // Gather annotations
            let links = page
                .ops
                .iter()
                .filter_map(|l| match l {
                    Op::LinkAnnotation { link } => Some(link.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();

            page_resources.set(
                "Annots",
                Array(
                    links
                        .iter()
                        .map(|l| Dictionary(link_annotation_to_dict(l, &page_ids_reserved)))
                        .collect(),
                ),
            );

            page_resources.set("Font", Reference(global_font_dict_id));
            page_resources.set("XObject", Reference(global_xobject_dict_id));
            page_resources.set("ExtGState", Reference(global_extgstate_dict_id));

            let layer_stream = translate_operations(
                &page.ops,
                &prepared_fonts,
                &pdf.resources.xobjects.map,
                opts.secure,
                warnings,
            ); // Vec<u8>
            let merged_layer_stream =
                LoStream::new(LoDictionary::new(), layer_stream).with_compression(false);

            let page_obj = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("MediaBox", page.get_media_box()),
                ("TrimBox", page.get_trim_box()),
                ("CropBox", page.get_crop_box()),
                ("Parent", Reference(pages_id)),
                ("Resources", Reference(doc.add_object(page_resources))),
                ("Contents", Reference(doc.add_object(merged_layer_stream))),
            ]);

            doc.set_object(*page_id, page_obj);

            *page_id
        })
        .collect::<Vec<_>>();

    // Now that the page objs are rendered, resolve which bookmarks reference which page objs
    if !pdf.bookmarks.map.is_empty() {
        let bookmarks_id = doc.new_object_id();
        let mut bookmarks_sorted = pdf.bookmarks.map.iter().collect::<Vec<_>>();
        bookmarks_sorted.sort_by(|(_, v), (_, v2)| (v.page, &v.name).cmp(&(v2.page, &v2.name)));
        let bookmarks_sorted = bookmarks_sorted
            .into_iter()
            .filter_map(|(k, v)| {
                let page_obj_id = page_ids.get(v.page).cloned()?;
                Some((k, &v.name, page_obj_id))
            })
            .collect::<Vec<_>>();

        let bookmark_ids = bookmarks_sorted
            .iter()
            .map(|(id, name, page_id)| {
                let newid = doc.new_object_id();
                (id, name, page_id, newid)
            })
            .collect::<Vec<_>>();

        let first = bookmark_ids.first().map(|s| s.3).unwrap();
        let last = bookmark_ids.last().map(|s| s.3).unwrap();
        for (i, (_id, name, pageid, self_id)) in bookmark_ids.iter().enumerate() {
            let prev = if i == 0 {
                None
            } else {
                bookmark_ids.get(i - 1).map(|s| s.3)
            };
            let next = bookmark_ids.get(i + 1).map(|s| s.3);
            let dest = Array(vec![Reference(*(*pageid)), "XYZ".into(), Null, Null, Null]);
            let mut dict = LoDictionary::from_iter(vec![
                ("Parent", Reference(bookmarks_id)),
                ("Title", LoString(name.to_string().into(), Literal)),
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
        catalog.set("PageMode", LoString("UseOutlines".into(), Literal));
    }

    doc.set_object(
        pages_id,
        LoDictionary::from_iter(vec![
            ("Type", "Pages".into()),
            ("Count", Integer(page_ids.len() as i64)),
            (
                "Kids",
                Array(page_ids.iter().map(|q| Reference(*q)).collect::<Vec<_>>()),
            ),
        ]),
    );

    let catalog_id = doc.add_object(catalog);
    let document_info_id = doc.add_object(Dictionary(docinfo_to_dict(&pdf.metadata.info)));
    let instance_id = crate::utils::random_character_string_32();
    let document_id = crate::utils::random_character_string_32();

    doc.trailer.set("Root", Reference(catalog_id));
    doc.trailer.set("Info", Reference(document_info_id));
    doc.trailer.set(
        "ID",
        Array(vec![
            LoString(document_id.as_bytes().to_vec(), Literal),
            LoString(instance_id.as_bytes().to_vec(), Literal),
        ]),
    );

    if opts.optimize {
        // doc.compress();
    }

    let mut bytes = Vec::new();
    let mut writer = std::io::BufWriter::new(&mut bytes);
    let _ = doc.save_to(&mut writer);
    std::mem::drop(writer);

    bytes
}

fn get_used_internal_fonts(pages: &[PdfPage]) -> BTreeSet<BuiltinFont> {
    pages
        .iter()
        .flat_map(|p| {
            p.ops.iter().filter_map(|op| match op {
                Op::WriteTextBuiltinFont { font, .. } => Some(*font),
                _ => None,
            })
        })
        .collect()
}

fn builtin_font_to_dict(font: &BuiltinFont) -> LoDictionary {
    LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("Type1".into())),
        ("BaseFont", Name(font.get_id().into())),
        ("Encoding", Name("WinAnsiEncoding".into())),
    ])
}

pub(crate) fn translate_operations(
    ops: &[Op],
    fonts: &BTreeMap<FontId, PreparedFont>,
    xobjects: &BTreeMap<XObjectId, XObject>,
    secure: bool,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let mut content = Vec::new();

    for op in ops {
        match op {
            Op::SetRenderingIntent { intent } => {
                content.push(LoOp::new("ri", vec![Name(intent.get_id().into())]));
            }
            Op::SetColorSpaceFill { id } => {
                content.push(LoOp::new("cs", vec![Name(id.clone().into())]));
            }
            Op::SetColorSpaceStroke { id } => {
                content.push(LoOp::new("CS", vec![Name(id.clone().into())]));
            }
            Op::SetHorizontalScaling { percent } => {
                content.push(LoOp::new("Tz", vec![Real(*percent)]));
            }
            Op::AddLineBreak => {
                content.push(LoOp::new("T*", vec![]));
            }
            Op::Marker { id } => {
                content.push(LoOp::new("MP", vec![Name(id.clone().into())]));
            }
            Op::BeginLayer { layer_id } => {
                content.push(LoOp::new(
                    "BDC",
                    vec![Name("OC".into()), Name(layer_id.0.clone().into())],
                ));
            }
            Op::EndLayer { .. } => {
                content.push(LoOp::new("EMC", vec![]));
            }
            Op::SaveGraphicsState => {
                content.push(LoOp::new("q", vec![]));
            }
            Op::RestoreGraphicsState => {
                content.push(LoOp::new("Q", vec![]));
            }
            Op::LoadGraphicsState { gs } => {
                content.push(LoOp::new("gs", vec![Name(gs.0.as_bytes().to_vec())]));
            }
            Op::StartTextSection => {
                content.push(LoOp::new("BT", vec![]));
            }
            Op::EndTextSection => {
                content.push(LoOp::new("ET", vec![]));
            }
            Op::WriteText { items, font } => {
                if let Some(prepared_font) = fonts.get(font) {
                    // Convert TextItems to PDF objects for the TJ operator
                    let mut tj_array = Vec::new();

                    for item in items {
                        match item {
                            TextItem::Text(text) => {
                                // Get glyph IDs for the text
                                let glyph_ids = text
                                    .chars()
                                    .filter_map(|s| {
                                        prepared_font.original.lookup_glyph_index(s as u32)
                                    })
                                    .collect::<Vec<_>>();

                                let bytes = glyph_ids
                                    .iter()
                                    .flat_map(|x| vec![(x >> 8) as u8, (x & 255) as u8])
                                    .collect::<Vec<u8>>();

                                tj_array.push(LoString(bytes, Hexadecimal));
                            }
                            TextItem::Offset(offset) => {
                                tj_array.push(Integer(*offset as i64));
                            }
                        }
                    }

                    // Only use TJ if we have multiple items or kerning offsets
                    if items.len() > 1 || items.iter().any(|i| matches!(i, TextItem::Offset(_))) {
                        content.push(LoOp::new("TJ", vec![Array(tj_array)]));
                    } else if let Some(TextItem::Text(_)) = items.first() {
                        // If it's a single text item with no kerning, use simpler Tj
                        content.push(LoOp::new("TJ", vec![Array(tj_array)]));
                    }
                }
            }

            Op::WriteTextBuiltinFont { items, font } => {
                // Convert TextItems to PDF objects for the TJ operator
                let mut tj_array = Vec::new();

                for item in items {
                    match item {
                        TextItem::Text(text) => {
                            let bytes = lopdf::Document::encode_text(
                                &lopdf::Encoding::SimpleEncoding(b"WinAnsiEncoding"),
                                text,
                            );
                            tj_array.push(LoString(bytes, Hexadecimal));
                        }
                        TextItem::Offset(offset) => {
                            tj_array.push(Integer(*offset as i64));
                        }
                    }
                }

                // Only use TJ if we have multiple items or kerning offsets
                if items.len() > 1 || items.iter().any(|i| matches!(i, TextItem::Offset(_))) {
                    content.push(LoOp::new("TJ", vec![Array(tj_array)]));
                } else if let Some(TextItem::Text(_)) = items.first() {
                    // If it's a single text item with no kerning, use simpler Tj
                    content.push(LoOp::new("TJ", vec![Array(tj_array)]));
                }
            }
            Op::WriteCodepoints { font, cp } => {
                if let Some(prepared_font) = fonts.get(font) {
                    let subset_codepoints = cp
                        .iter()
                        .filter_map(|(gid, ch)| {
                            prepared_font
                                .subset_font
                                .glyph_mapping
                                .get(gid)
                                .map(|c| (c.0, *ch))
                        })
                        .collect::<Vec<_>>();

                    let bytes = subset_codepoints
                        .into_iter()
                        .flat_map(|(x, _)| {
                            let [b0, b1] = x.to_be_bytes();
                            std::iter::once(b0).chain(std::iter::once(b1))
                        })
                        .collect::<Vec<u8>>();

                    content.push(LoOp::new("Tj", vec![LoString(bytes, Hexadecimal)]));
                }
            }
            Op::WriteCodepointsWithKerning { font, cpk } => {
                if let Some(font) = fonts.get(font) {
                    let subset_codepoints = cpk
                        .iter()
                        .filter_map(|(kern, gid, ch)| {
                            font.subset_font
                                .glyph_mapping
                                .get(gid)
                                .map(|c| (*kern, c.0, *ch))
                        })
                        .collect::<Vec<_>>();

                    let mut list = Vec::new();

                    for (pos, codepoint, _) in subset_codepoints.iter() {
                        if *pos != 0 {
                            list.push(Integer(*pos));
                        }
                        let bytes = codepoint.to_be_bytes().to_vec();
                        list.push(LoString(bytes, Hexadecimal));
                    }

                    content.push(LoOp::new("TJ", vec![Array(list)]));
                }
            }
            Op::SetLineHeight { lh } => {
                content.push(LoOp::new("TL", vec![Real(lh.0)]));
            }
            Op::SetWordSpacing { pt } => {
                content.push(LoOp::new("Tw", vec![Real(pt.0)]));
            }
            Op::SetFontSize { size, font } => {
                content.push(LoOp::new(
                    "Tf",
                    vec![font.0.clone().into(), (size.0).into()],
                ));
            }
            Op::SetFontSizeBuiltinFont { size, font } => {
                content.push(LoOp::new("Tf", vec![font.get_id().into(), (size.0).into()]));
            }
            Op::SetTextCursor { pos } => {
                content.push(LoOp::new("Td", vec![pos.x.0.into(), pos.y.0.into()]));
            }
            Op::SetFillColor { col } => {

                let ci = match &col {
                    Color::Rgb(_) => "rg",
                    Color::Cmyk(_) | Color::SpotColor(_) => "k",
                    Color::Greyscale(_) => "g",
                };

                if col.is_out_of_range() {
                    warnings.push(PdfWarnMsg::error(
                        0,
                        0,
                        format!(
                            "PDF color {col:?} is out of range, must be normalized to 0.0 - 1.0"
                        ),
                    ));
                }
                let cvec = col.into_vec().into_iter().map(Real).collect();
                content.push(LoOp::new(ci, cvec));
            }
            Op::SetOutlineColor { col } => {
                let ci = match &col {
                    Color::Rgb(_) => "RG",
                    Color::Cmyk(_) | Color::SpotColor(_) => "K",
                    Color::Greyscale(_) => "G",
                };
                if col.is_out_of_range() {
                    warnings.push(PdfWarnMsg::error(
                        0,
                        0,
                        format!(
                            "PDF color {col:?} is out of range, must be normalized to 0.0 - 1.0"
                        ),
                    ));
                }
                let cvec = col.into_vec().into_iter().map(Real).collect();
                content.push(LoOp::new(ci, cvec));
            }
            Op::SetOutlineThickness { pt } => {
                content.push(LoOp::new("w", vec![Real(pt.0)]));
            }
            Op::SetLineDashPattern { dash } => {
                let dash_array_ints = dash.as_array().into_iter().map(Integer).collect();
                content.push(LoOp::new(
                    "d",
                    vec![Array(dash_array_ints), Integer(dash.offset)],
                ));
            }
            Op::SetLineJoinStyle { join } => {
                content.push(LoOp::new("j", vec![Integer(join.id())]));
            }
            Op::SetMiterLimit { limit } => {
                content.push(LoOp::new("M", vec![Real(limit.0)]));
            }
            Op::SetLineCapStyle { cap } => {
                content.push(LoOp::new("J", vec![Integer(cap.id())]));
            }
            Op::SetTextRenderingMode { mode } => {
                content.push(LoOp::new("Tr", vec![Integer(mode.id())]));
            }
            Op::SetCharacterSpacing { multiplier } => {
                content.push(LoOp::new("Tc", vec![Real(*multiplier)]));
            }
            Op::SetLineOffset { multiplier } => {
                content.push(LoOp::new("Ts", vec![Real(*multiplier)]));
            }
            Op::DrawLine { line } => {
                content.append(&mut line_to_stream_ops(line));
            }
            Op::DrawPolygon { polygon } => {
                content.append(&mut polygon_to_stream_ops(polygon));
            }
            Op::SetTransformationMatrix { matrix } => {
                content.push(LoOp::new(
                    "cm",
                    matrix.as_array().iter().copied().map(Real).collect(),
                ));
            }
            Op::SetTextMatrix { matrix } => {
                content.push(LoOp::new(
                    "Tm",
                    matrix.as_array().iter().copied().map(Real).collect(),
                ));
            }
            Op::LinkAnnotation { link } => {
                // TODO!
            }
            Op::UseXobject { id, transform } => {
                use crate::matrix::CurTransMat;
                let mut t = CurTransMat::Identity;
                for q in
                    transform.get_ctms(xobjects.get(id).and_then(|xobj| xobj.get_width_height()))
                {
                    t = CurTransMat::Raw(CurTransMat::combine_matrix(t.as_array(), q.as_array()));
                }

                content.push(LoOp::new("q", vec![]));
                content.push(LoOp::new(
                    "cm",
                    t.as_array().into_iter().map(Real).collect(),
                ));
                content.push(LoOp::new("Do", vec![Name(id.0.as_bytes().to_vec())]));
                content.push(LoOp::new("Q", vec![]));
            }
            Op::BeginInlineImage => {
                content.push(LoOp::new("BI", vec![]));
            }
            Op::BeginInlineImageData => {
                content.push(LoOp::new("ID", vec![]));
            }
            Op::EndInlineImage => {
                content.push(LoOp::new("EI", vec![]));
            }
            Op::BeginMarkedContent { tag } => {
                content.push(LoOp::new("BMC", vec![Name(tag.clone().into())]));
            }
            Op::BeginMarkedContentWithProperties { tag, properties } => {
                let props = Array(properties.iter().map(|item| item.to_lopdf()).collect());
                content.push(LoOp::new("BDC", vec![Name(tag.clone().into()), props]));
            }
            Op::DefineMarkedContentPoint { tag, properties } => {
                let props = Array(properties.iter().map(|item| item.to_lopdf()).collect());
                content.push(LoOp::new("DP", vec![Name(tag.clone().into()), props]));
            }
            Op::EndMarkedContent => {
                content.push(LoOp::new("EMC", vec![]));
            }
            Op::BeginCompatibilitySection => {
                content.push(LoOp::new("BX", vec![]));
            }
            Op::EndCompatibilitySection => {
                content.push(LoOp::new("EX", vec![]));
            }
            Op::MoveToNextLineShowText { text } => {
                content.push(LoOp::new(
                    "'",
                    vec![LoString(text.as_bytes().to_vec(), Hexadecimal)],
                ));
            }
            Op::SetSpacingMoveAndShowText {
                word_spacing,
                char_spacing,
                text,
            } => {
                content.push(LoOp::new(
                    "\"",
                    vec![
                        Real(*word_spacing),
                        Real(*char_spacing),
                        LoString(text.as_bytes().to_vec(), Hexadecimal),
                    ],
                ));
            }
            Op::MoveTextCursorAndSetLeading { tx, ty } => {
                content.push(LoOp::new("TD", vec![Real(*tx), Real(*ty)]));
            }
            Op::Unknown { key, value } => {
                // Skip unknown operators for security reasons.
                if !secure {
                    content.push(LoOp::new(
                        key.as_str(),
                        value.iter().map(|s| s.to_lopdf()).collect(),
                    ));
                }
            }
        }
    }

    lopdf::content::Content {
        operations: content,
    }
    .encode()
    .unwrap_or_default()
}

pub(crate) struct PreparedFont {
    original: ParsedFont,
    pub(crate) subset_font: SubsetFont,
    cid_to_unicode_map: String,
    vertical_writing: bool, // default: false
    ascent: i64,
    descent: i64,
    max_height: i64,
    total_width: i64,
    // encode widths / heights so that they fit into what PDF expects
    // see page 439 in the PDF 1.7 reference
    // basically widths_list will contain objects like this:
    // 20 [21, 99, 34, 25]
    // which means that the character with the GID 20 has a width of 21 units
    // and the character with the GID 21 has a width of 99 units
    widths_list: Vec<lopdf::Object>,
}

const DEFAULT_CHARACTER_WIDTH: i64 = 1000;

fn line_to_stream_ops(line: &Line) -> Vec<LoOp> {
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to point
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Cubic bezier with three control points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// Stroke path
    pub const OP_PATH_PAINT_STROKE: &str = "S";
    /// Close path
    pub const OP_PATH_CLOSE: &str = "h";

    let mut operations = Vec::new();
    let points = &line.points;
    
    if points.is_empty() {
        return operations;
    }

    // Start with a move to the first point
    operations.push(LoOp::new(
        OP_PATH_CONST_MOVE_TO,
        vec![points[0].p.x.into(), points[0].p.y.into()],
    ));

    // Process remaining points
    let mut i = 1;
    while i < points.len() {
        let current = &points[i];
        
        if current.bezier {
            // Current point is a bezier handle
            // For a cubic bezier, we need two control points and an end point
            if i + 2 < points.len() {
                let control1 = current;
                let control2 = &points[i + 1];
                let end_point = &points[i + 2];
                
                // Check if second control point is also a bezier handle
                if control2.bezier {
                    // Two bezier handles followed by an end point
                    operations.push(LoOp::new(
                        OP_PATH_CONST_4BEZIER,
                        vec![
                            control1.p.x.into(), control1.p.y.into(),
                            control2.p.x.into(), control2.p.y.into(),
                            end_point.p.x.into(), end_point.p.y.into(),
                        ],
                    ));
                    i += 3; // Skip past the control points and end point
                } else {
                    // Only one bezier handle - treat as a line to be safe
                    operations.push(LoOp::new(
                        OP_PATH_CONST_LINE_TO,
                        vec![current.p.x.into(), current.p.y.into()],
                    ));
                    i += 1;
                }
            } else {
                // Not enough points left for a bezier curve
                operations.push(LoOp::new(
                    OP_PATH_CONST_LINE_TO,
                    vec![current.p.x.into(), current.p.y.into()],
                ));
                i += 1;
            }
        } else {
            // Regular point - draw a straight line
            operations.push(LoOp::new(
                OP_PATH_CONST_LINE_TO,
                vec![current.p.x.into(), current.p.y.into()],
            ));
            i += 1;
        }
    }

    // Add final operations
    if line.is_closed {
        // Close the path before stroking
        operations.push(LoOp::new(OP_PATH_CLOSE, vec![]));
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE, vec![]));
    } else {
        // Just stroke without closing
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE, vec![]));
    }

    operations
}

fn polygon_to_stream_ops(poly: &Polygon) -> Vec<LoOp> {
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to point
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Cubic bezier with three control points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// End path without filling or stroking
    pub const OP_PATH_PAINT_END: &str = "n";

    let mut operations = Vec::new();

    if poly.rings.is_empty() {
        return operations;
    }

    for ring in &poly.rings {
        let points = &ring.points;
        
        if points.is_empty() {
            continue;
        }

        // Start with a move to the first point
        operations.push(LoOp::new(
            OP_PATH_CONST_MOVE_TO,
            vec![points[0].p.x.into(), points[0].p.y.into()],
        ));

        // Process remaining points
        let mut i = 1;
        while i < points.len() {
            let current = &points[i];
            
            if current.bezier {
                // Current point is a bezier handle
                // For a cubic bezier, we need two control points and an end point
                if i + 2 < points.len() {
                    let control1 = current;
                    let control2 = &points[i + 1];
                    let end_point = &points[i + 2];
                    
                    // Check if second control point is also a bezier handle
                    if control2.bezier {
                        // Two bezier handles followed by an end point
                        operations.push(LoOp::new(
                            OP_PATH_CONST_4BEZIER,
                            vec![
                                control1.p.x.into(), control1.p.y.into(),
                                control2.p.x.into(), control2.p.y.into(),
                                end_point.p.x.into(), end_point.p.y.into(),
                            ],
                        ));
                        i += 3; // Skip past the control points and end point
                    } else {
                        // Only one bezier handle - treat as a line to be safe
                        operations.push(LoOp::new(
                            OP_PATH_CONST_LINE_TO,
                            vec![current.p.x.into(), current.p.y.into()],
                        ));
                        i += 1;
                    }
                } else {
                    // Not enough points left for a bezier curve
                    operations.push(LoOp::new(
                        OP_PATH_CONST_LINE_TO,
                        vec![current.p.x.into(), current.p.y.into()],
                    ));
                    i += 1;
                }
            } else {
                // Regular point - draw a straight line
                operations.push(LoOp::new(
                    OP_PATH_CONST_LINE_TO,
                    vec![current.p.x.into(), current.p.y.into()],
                ));
                i += 1;
            }
        }
    }

    // Explicitly close the path with 'h' before applying painting operations
    operations.push(LoOp::new("h", vec![]));
    
    // Apply the painting operation based on the mode
    match poly.mode {
        PaintMode::Clip => {
            operations.push(LoOp::new(poly.winding_order.get_clip_op(), vec![]));
            // End the path with 'n' only after clipping
            operations.push(LoOp::new(OP_PATH_PAINT_END, vec![]));
        }
        PaintMode::Fill => {
            operations.push(LoOp::new(poly.winding_order.get_fill_op(), vec![]));
        }
        PaintMode::Stroke => {
            // Use 'S' (stroke) rather than 's' (close and stroke) since we already closed with 'h'
            operations.push(LoOp::new("S", vec![]));
        }
        PaintMode::FillStroke => {
            operations.push(LoOp::new(
                poly.winding_order.get_fill_stroke_close_op(),
                vec![],
            ));
        }
    }

    operations
}

pub(crate) fn prepare_fonts(
    resources: &PdfResources,
    pages: &[PdfPage],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<FontId, PreparedFont> {
    let mut fonts_in_pdf = BTreeMap::new();

    for (font_id, font) in resources.fonts.map.iter() {
        let glyph_ids = font.get_used_glyph_ids(font_id, pages);
        if glyph_ids.is_empty() {
            continue; // unused font
        }
        let subset_font =
            match font.subset(&glyph_ids.iter().map(|s| (*s.0, *s.1)).collect::<Vec<_>>()) {
                Ok(o) => o,
                Err(e) => {
                    warnings.push(PdfWarnMsg::error(
                        0,
                        0,
                        format!("failed to subset font: {e}"),
                    ));
                    continue;
                }
            };
        let font = match ParsedFont::from_bytes(&subset_font.bytes, 0, warnings) {
            Some(s) => s,
            None => continue,
        };
        let glyph_ids = font.get_used_glyph_ids(font_id, pages);
        let cid_to_unicode = font.generate_cid_to_unicode_map(font_id, &glyph_ids);
        let widths = font.get_normalized_widths(&glyph_ids);
        fonts_in_pdf.insert(
            font_id.clone(),
            PreparedFont {
                original: font.clone(),
                subset_font,
                cid_to_unicode_map: cid_to_unicode,
                vertical_writing: false, // !font.vmtx_data.is_empty(),
                ascent: font.font_metrics.ascender as i64,
                descent: font.font_metrics.descender as i64,
                widths_list: widths,
                max_height: font.get_max_height(&glyph_ids),
                total_width: font.get_total_width(&glyph_ids),
            },
        );
    }

    fonts_in_pdf
}

fn add_font_to_pdf(
    doc: &mut lopdf::Document,
    font_id: &FontId,
    prepared: &PreparedFont,
) -> LoDictionary {
    let face_name = font_id.0.clone();

    let vertical = prepared.vertical_writing;

    // WARNING: Font stream MAY NOT be compressed
    let font_stream = LoStream::new(
        LoDictionary::from_iter(vec![(
            "Length1",
            Integer(prepared.subset_font.bytes.len() as i64),
        )]),
        prepared.subset_font.bytes.clone(),
    )
    .with_compression(false);

    let font_stream_ref = doc.add_object(font_stream);

    LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("Type0".into())),
        ("BaseFont", Name(face_name.clone().into_bytes())),
        (
            "Encoding",
            Name(if vertical { "Identity-V" } else { "Identity-H" }.into()),
        ),
        (
            "ToUnicode",
            Reference(doc.add_object(LoStream::new(
                LoDictionary::new(),
                prepared.cid_to_unicode_map.as_bytes().to_vec(),
            ))),
        ),
        (
            "DescendantFonts",
            Array(vec![Dictionary(LoDictionary::from_iter(vec![
                ("Type", Name("Font".into())),
                ("Subtype", Name("CIDFontType2".into())),
                ("BaseFont", Name(face_name.clone().into_bytes())),
                (
                    "CIDSystemInfo",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Registry", LoString("Adobe".into(), Literal)),
                        ("Ordering", LoString("Identity".into(), Literal)),
                        ("Supplement", Integer(0)),
                    ])),
                ),
                (
                    if vertical { "W2" } else { "W" },
                    Array(prepared.widths_list.clone()),
                ),
                (
                    if vertical { "DW2" } else { "DW" },
                    Integer(DEFAULT_CHARACTER_WIDTH),
                ),
                (
                    "FontDescriptor",
                    Reference(doc.add_object(LoDictionary::from_iter(vec![
                        ("Type", Name("FontDescriptor".into())),
                        ("FontName", Name(face_name.clone().into_bytes())),
                        ("Ascent", Integer(prepared.ascent)),
                        ("Descent", Integer(prepared.descent)),
                        ("CapHeight", Integer(prepared.ascent)),
                        ("ItalicAngle", Integer(0)),
                        ("Flags", Integer(32)),
                        ("StemV", Integer(80)),
                        ("FontFile2", Reference(font_stream_ref)),
                        (
                            "FontBBox",
                            Array(vec![
                                Integer(0),
                                Integer(0),
                                Integer(prepared.total_width),
                                Integer(prepared.max_height),
                            ]),
                        ),
                    ]))),
                ),
            ]))]),
        ),
    ])
}

fn docinfo_to_dict(m: &PdfDocumentInfo) -> LoDictionary {
    let trapping = if m.trapped { "True" } else { "False" };
    let gts_pdfx_version = m.conformance.get_identifier_string();

    let info_mod_date = crate::utils::to_pdf_time_stamp_metadata(&m.modification_date);
    let info_create_date = crate::utils::to_pdf_time_stamp_metadata(&m.creation_date);

    let creation_date = LoString(info_create_date.into_bytes(), Literal);
    let title = LoString(m.document_title.to_string().as_bytes().to_vec(), Literal);
    let identifier = LoString(m.identifier.as_bytes().to_vec(), Literal);
    let keywords = LoString(m.keywords.join(",").as_bytes().to_vec(), Literal);

    LoDictionary::from_iter(vec![
        ("Trapped", trapping.into()),
        ("CreationDate", creation_date),
        ("ModDate", LoString(info_mod_date.into_bytes(), Literal)),
        (
            "GTS_PDFXVersion",
            LoString(gts_pdfx_version.into(), Literal),
        ),
        ("Title", title),
        ("Author", LoString(m.author.as_bytes().to_vec(), Literal)),
        ("Creator", LoString(m.creator.as_bytes().to_vec(), Literal)),
        (
            "Producer",
            LoString(m.producer.as_bytes().to_vec(), Literal),
        ),
        ("Subject", LoString(m.subject.as_bytes().to_vec(), Literal)),
        ("Identifier", identifier),
        ("Keywords", keywords),
    ])
}

fn icc_to_stream(val: &IccProfile) -> LoStream {
    use lopdf::{Dictionary as LoDictionary, Object::*, Stream as LoStream};

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

fn link_annotation_to_dict(la: &LinkAnnotation, page_ids: &[lopdf::ObjectId]) -> LoDictionary {
    let ll = la.rect.lower_left();
    let ur = la.rect.upper_right();

    let mut dict: LoDictionary = LoDictionary::new();
    dict.set("Type", Name("Annot".into()));
    dict.set("Subtype", Name("Link".into()));
    dict.set(
        "Rect",
        Array(vec![Real(ll.x.0), Real(ll.y.0), Real(ur.x.0), Real(ur.y.0)]),
    );
    dict.set("A", Dictionary(actions_to_dict(&la.actions, page_ids)));
    dict.set(
        "Border",
        Array(la.border.to_array().into_iter().map(Real).collect()),
    );
    dict.set(
        "C",
        Array(
            color_array_to_f32(&la.color)
                .into_iter()
                .map(Real)
                .collect(),
        ),
    );
    dict.set("H", Name(la.highlighting.get_id().into()));
    dict
}

fn actions_to_dict(a: &Actions, page_ids: &[lopdf::ObjectId]) -> LoDictionary {
    let mut dict = LoDictionary::new();
    dict.set("S", Name(a.get_action_type_id().into()));
    match a {
        Actions::Goto(destination) => {
            dict.set("D", destination_to_obj(destination, page_ids));
        }
        Actions::Uri(uri) => {
            dict.set("URI", LoString(uri.clone().into_bytes(), Literal));
        }
    }
    dict
}

fn destination_to_obj(d: &Destination, page_ids: &[lopdf::ObjectId]) -> lopdf::Object {
    match d {
        Destination::Xyz {
            page,
            left,
            top,
            zoom,
        } => Array(vec![
            page_ids
                .get(page.saturating_sub(1))
                .copied()
                .map(Reference)
                .unwrap_or(Null),
            Name("XYZ".into()),
            left.map(Real).unwrap_or(Null),
            top.map(Real).unwrap_or(Null),
            zoom.map(Real).unwrap_or(Null),
        ]),
    }
}

fn color_array_to_f32(c: &ColorArray) -> Vec<f32> {
    match c {
        ColorArray::Transparent => Vec::new(),
        ColorArray::Gray(arr) => arr.to_vec(),
        ColorArray::Rgb(arr) => arr.to_vec(),
        ColorArray::Cmyk(arr) => arr.to_vec(),
    }
}
