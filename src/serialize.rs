use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

use lopdf::{
    content::Operation as LoOp,
    Dictionary as LoDictionary,
    Object::{Array, Dictionary, Integer, Name, Null, Real, Reference, Stream, String as LoString},
    Stream as LoStream,
    StringFormat::{Hexadecimal, Literal},
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    color::IccProfile,
    font::ParsedFont,
    Actions, BuiltinFont, Color, ColorArray, Destination, FontId, IccProfileType,
    ImageOptimizationOptions, Line, LinkAnnotation, Op, PaintMode, PdfDocument,
    PdfDocumentInfo, PdfPage, PdfResources, PdfWarnMsg, Polygon, TextItem, XObject,
    XObjectId,
};

// NOTE: field names are snake_case ON THE WIRE (`subset_fonts`, `image_optimization`)
// and JS clients depend on that. A `#[serde(rename = "camelCase")]` attribute sat here
// for a long time — that renames the STRUCT to the literal string "camelCase" (a no-op
// for JSON objects); the camelCasing intent was never in effect and must not be
// "fixed" to `rename_all` now, since that would break every existing client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
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

pub fn serialize_pdf<W: Write>(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    mut writer: &mut W,
    warnings: &mut Vec<PdfWarnMsg>,
) -> () {
    let mut doc = to_lopdf_doc(pdf, opts, warnings);
    if opts.optimize {
        // doc.compress();
    }

    let _ = doc.save_to(&mut writer);
}

pub fn to_lopdf_doc(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> lopdf::Document {
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

    // Build fonts dictionary with deferred subsetting
    let mut global_font_dict = LoDictionary::new();
    let (font_infos, subset_fonts) = prepare_fonts_for_serialization(&pdf.resources, &pdf.pages, opts.subset_fonts, warnings);
    for (font_id, subset_info) in subset_fonts.iter() {
        let font_dict = add_subset_font_to_pdf(&mut doc, font_id, subset_info);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(font_id.0.clone(), Reference(font_dict_id));
    }

    for internal_font in get_used_internal_fonts(&pdf.pages) {
        // Never clobber an embedded font that happens to be named F1..F14 (very
        // common in parsed foreign PDFs) with a builtin standard-14 dict — the
        // page ops referencing that name would suddenly point at the wrong font.
        // resolve_current_font applies the same embedded-first rule per op.
        if global_font_dict.has(internal_font.get_pdf_id().as_bytes()) {
            warnings.push(PdfWarnMsg::warning(
                0,
                0,
                format!(
                    "builtin font {} not emitted: an embedded font already uses that resource name",
                    internal_font.get_pdf_id()
                ),
            ));
            continue;
        }
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

    let mut global_shading_dict = LoDictionary::new();
    for (k, v) in pdf.resources.shadings.map.iter() {
        global_shading_dict.set(k.0.clone(), v.to_dict());
    }
    let global_shading_dict_id = doc.add_object(global_shading_dict);

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
            let mut links = Vec::new();
            for op in &page.ops {
                if let Op::LinkAnnotation { link } = op {
                    links.push(Dictionary(link_annotation_to_dict(
                        link,
                        &page_ids_reserved,
                    )))
                }
            }

            page_resources.set("Font", Reference(global_font_dict_id));
            page_resources.set("XObject", Reference(global_xobject_dict_id));
            page_resources.set("ExtGState", Reference(global_extgstate_dict_id));
            page_resources.set("Shading", Reference(global_shading_dict_id));

            let layer_stream = translate_operations(
                &page.ops,
                &font_infos,
                &pdf.resources.xobjects.map,
                opts.secure,
                warnings,
            ); // Vec<u8>
            let merged_layer_stream =
                LoStream::new(LoDictionary::new(), layer_stream);

            let page_obj = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("MediaBox", page.get_media_box()),
                ("TrimBox", page.get_trim_box()),
                ("CropBox", page.get_crop_box()),
                ("Parent", Reference(pages_id)),
                ("Resources", Reference(doc.add_object(page_resources))),
                ("Contents", Reference(doc.add_object(merged_layer_stream))),
                ("Annots", Array(links)),
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
                let page_obj_id = page_ids.get(v.page.saturating_sub(1)).cloned()?;
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
                ("Title", encode_text_to_utf16be(name)),
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

    doc
}
pub fn serialize_pdf_into_bytes(
    pdf: &PdfDocument,
    opts: &PdfSaveOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut writer = std::io::BufWriter::new(&mut bytes);
    serialize_pdf(pdf, opts, &mut writer, warnings);
    std::mem::drop(writer);
    bytes
}
fn get_used_internal_fonts(pages: &[PdfPage]) -> BTreeSet<BuiltinFont> {
    // With new API, builtin fonts are referenced in SetFont operation
    // Parse SetFont operations to extract builtin font usage
    let mut fonts = BTreeSet::new();
    
    for page in pages {
        for op in &page.ops {
            if let Op::SetFont { font, .. } = op {
                // Extract builtin fonts
                if let crate::ops::PdfFontHandle::Builtin(builtin_font) = font {
                    fonts.insert(*builtin_font);
                }
            }
        }
    }
    
    fonts
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
    font_infos: &BTreeMap<FontId, RuntimeFontInfo>,
    xobjects: &BTreeMap<XObjectId, XObject>,
    secure: bool,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<u8> {
    let mut content = Vec::new();

    // Track current font for ShowText operations
    let mut current_font_resource: Option<String> = None;

    // Text-showing operators are only valid between BT and ET. Emitting them
    // outside is not a rendering variation — Acrobat and most viewers drop the
    // text entirely, and the classic symptom is a "blank PDF" with no error
    // (#254). The ops are still written (some tolerant viewers show them), but
    // the mistake is loud now.
    let mut in_text_section = false;
    let mut warned_outside_text_section = false;
    let mut warn_outside = |op_idx: usize,
                            op_name: &str,
                            warned: &mut bool,
                            warnings: &mut Vec<PdfWarnMsg>| {
        if !*warned {
            warnings.push(PdfWarnMsg::error(
                0,
                op_idx,
                format!(
                    "{op_name} outside of a text section: most PDF viewers will not \
                     display this text. Wrap text operations in \
                     Op::StartTextSection .. Op::EndTextSection (reported once per \
                     content stream)"
                ),
            ));
            *warned = true;
        }
    };

    for (op_idx, op) in ops.iter().enumerate() {
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
            Op::EndLayer => {
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
            Op::PaintShading { id } => {
                // `sh` paints the named shading over the current clip region.
                content.push(LoOp::new("sh", vec![Name(id.0.as_bytes().to_vec())]));
            }
            Op::StartTextSection => {
                in_text_section = true;
                content.push(LoOp::new("BT", vec![]));
            }
            Op::EndTextSection => {
                in_text_section = false;
                content.push(LoOp::new("ET", vec![]));
            }
            // New 1:1 PDF operations
            Op::SetFont { font, size } => {
                let font_resource_name = font.get_resource_name();
                current_font_resource = Some(font_resource_name.clone());
                content.push(LoOp::new(
                    "Tf",
                    vec![lopdf::Object::Name(font_resource_name.as_bytes().to_vec()), size.0.into()],
                ));
            }
            Op::ShowText { items } => {
                if !in_text_section {
                    warn_outside(op_idx, "Op::ShowText", &mut warned_outside_text_section, warnings);
                }
                // ShowText maps to Tj/TJ - font must be set via SetFont first
                let (builtin_font, font_info) =
                    resolve_current_font(&current_font_resource, font_infos);

                encode_text_items_to_pdf(items, font_info, builtin_font.as_ref(), &mut content);
            }
            Op::SetLineHeight { lh } => {
                content.push(LoOp::new("TL", vec![Real(lh.0)]));
            }
            Op::SetWordSpacing { pt } => {
                content.push(LoOp::new("Tw", vec![Real(pt.0)]));
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
                let dash_array_floats = dash.pattern.iter().copied().map(Real).collect();
                content.push(LoOp::new(
                    "d",
                    vec![Array(dash_array_floats), Real(dash.offset)],
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
            Op::DrawRectangle { rectangle } => {
                content.append(&mut rectangle_to_stream_ops(rectangle));
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
            Op::LinkAnnotation { link: _ } => {}
            Op::UseXobject { id, transform } => {
                use crate::matrix::CurTransMat;
                let mut t = CurTransMat::Identity;
                // `no_auto_scale`: parsed content already carries its placement
                // in its own matrix ops — suppress the unit-square → pixel-size
                // convenience scale instead of forcing callers to cancel it.
                let wh = if transform.no_auto_scale {
                    None
                } else {
                    xobjects.get(id).and_then(|xobj| xobj.get_width_height())
                };
                for q in transform.get_ctms(wh) {
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
                content.push(LoOp::new(
                    "BDC",
                    vec![Name(tag.clone().into()), properties.to_lopdf()]
                ));
            }
            Op::BeginOptionalContent { layer_id } => {
                content.push(LoOp::new(
                    "BDC",
                    vec![Name("OC".into()), Name(layer_id.0.clone().into())],
                ));
            }
            Op::DefineMarkedContentPoint { tag, properties } => {
                let props = Array(properties.iter().map(|item| item.to_lopdf()).collect());
                content.push(LoOp::new("DP", vec![Name(tag.clone().into()), props]));
            }
            Op::EndMarkedContent { .. } | Op::EndMarkedContentWithProperties { .. } | Op::EndOptionalContent { .. } => {
                content.push(LoOp::new("EMC", vec![]));
            }
            Op::BeginCompatibilitySection => {
                content.push(LoOp::new("BX", vec![]));
            }
            Op::EndCompatibilitySection => {
                content.push(LoOp::new("EX", vec![]));
            }
            // `'` and `"` show text just like Tj does, so they need the *same* encoding as
            // the currently selected font — glyph ids for an external font, WinAnsi for a
            // built-in one. They used to emit `text.as_bytes()` (raw UTF-8) unconditionally,
            // which is wrong for both (issue #273).
            Op::MoveToNextLineShowText { text } => {
                if !in_text_section {
                    warn_outside(
                        op_idx,
                        "Op::MoveToNextLineShowText",
                        &mut warned_outside_text_section,
                        warnings,
                    );
                }
                let (builtin_font, font_info) =
                    resolve_current_font(&current_font_resource, font_infos);
                content.push(LoOp::new(
                    "'",
                    vec![encode_text_for_font(text, font_info, builtin_font.as_ref())],
                ));
            }
            Op::SetSpacingMoveAndShowText {
                word_spacing,
                char_spacing,
                text,
            } => {
                if !in_text_section {
                    warn_outside(
                        op_idx,
                        "Op::SetSpacingMoveAndShowText",
                        &mut warned_outside_text_section,
                        warnings,
                    );
                }
                let (builtin_font, font_info) =
                    resolve_current_font(&current_font_resource, font_infos);
                content.push(LoOp::new(
                    "\"",
                    vec![
                        Real(*word_spacing),
                        Real(*char_spacing),
                        encode_text_for_font(text, font_info, builtin_font.as_ref()),
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

// Helper function to encode text items to PDF operations
// 
// IMPORTANT: This function uses ORIGINAL glyph IDs directly.
// Font subsetting (if enabled) happens at font serialization time,
// and the glyph ID remapping is done there, not here.
/// Resolve the font selected by the most recent `Tf` into either a built-in face or an
/// embedded one. Every text-showing operator (`Tj`, `TJ`, `'`, `"`) needs this, because
/// the two classes encode their strings completely differently.
fn resolve_current_font<'a>(
    current_font_resource: &Option<String>,
    font_infos: &'a BTreeMap<FontId, RuntimeFontInfo>,
) -> (Option<BuiltinFont>, Option<&'a RuntimeFontInfo>) {
    // The document's own fonts take priority. A parsed foreign PDF very often
    // names its embedded fonts /F1../F14; treating those as the builtin
    // standard-14 faces WinAnsi-encoded text that belongs to a Type0 font —
    // instant mojibake on re-save. The builtin-name heuristic applies only
    // when no embedded font claims the resource name (mirrors the parse-side
    // rule in deserialize.rs).
    let info = current_font_resource
        .as_ref()
        .and_then(|r| font_infos.get(&FontId(r.clone())));

    let builtin = if info.is_none() {
        current_font_resource
            .as_ref()
            .and_then(|r| BuiltinFont::from_id(r))
    } else {
        None
    };

    (builtin, info)
}

/// Map an original glyph id to the subset's renumbered id when the font was
/// subset; otherwise return it unchanged. Unmapped gids fall back to 0 (.notdef).
fn remap_gid(font_info: Option<&RuntimeFontInfo>, gid: u16) -> u16 {
    match font_info.and_then(|fi| fi.gid_remap.as_ref()) {
        Some(map) => map.get(&gid).copied().unwrap_or(0),
        None => gid,
    }
}

fn encode_text_items_to_pdf(
    items: &[TextItem],
    font_info: Option<&RuntimeFontInfo>,
    builtin_font: Option<&BuiltinFont>,
    content: &mut Vec<LoOp>,
) {
    // Skip if no items
    if items.is_empty() {
        return;
    }

    // Process text items into PDF objects for TJ/Tj operator
    let mut tj_array = Vec::new();

    for item in items {
        match item {
            TextItem::Text(text) => {
                tj_array.push(encode_text_for_font(text, font_info, builtin_font));
            }
            TextItem::Offset(offset) => {
                tj_array.push(Real(*offset));
            }
            TextItem::GlyphIds(glyphs) => {
                // Use original glyph IDs directly
                // Subsetting remapping happens at font serialization time
                for codepoint in glyphs {
                    let gid = remap_gid(font_info, codepoint.gid);
                    let bytes = gid.to_be_bytes().to_vec();
                    tj_array.push(LoString(bytes, Hexadecimal));
                    if codepoint.offset != 0.0 {
                        tj_array.push(Real(codepoint.offset));
                    }
                }
            }
        }
    }

    // Choose appropriate operator based on complexity
    if tj_array.len() == 1 && !items.iter().any(|i| matches!(i, TextItem::Offset(_) | TextItem::GlyphIds(_))) {
        // Single text item with no kerning - use simpler Tj
        content.push(LoOp::new("Tj", vec![tj_array.swap_remove(0)]));
    } else {
        // Multiple items or has kerning offsets - use TJ
        content.push(LoOp::new("TJ", vec![Array(tj_array)]));
    }
}

// Helper function to determine if bytes need hexadecimal encoding
fn needs_hex_encoding(bytes: &[u8]) -> bool {
    bytes.iter().any(|&b| {
        // Bytes that require hex encoding:
        // - Control characters
        // - Non-ASCII characters
        // - Special characters like (, ), \, etc.
        b < 32 || b > 126 || b == b'(' || b == b')' || b == b'\\' || b == b'%'
    })
}

/// Encode `text` as `WinAnsiEncoding` bytes (PDF 32000-1 Annex D.2 — effectively CP1252).
///
/// The 14 built-in fonts are written with `/Encoding /WinAnsiEncoding`, which is a
/// *single-byte* encoding. Their content streams must therefore carry WinAnsi bytes.
/// Emitting the raw UTF-8 instead means `ü` (U+00FC, UTF-8 `C3 BC`) reaches the reader as
/// the two WinAnsi characters `Ã¼`, so every non-ASCII character comes out as mojibake
/// and the text is not copy-able (issue #273).
///
/// We do this ourselves rather than via `lopdf::Document::encode_text`, because lopdf
/// 0.39 does not recognise the `WinAnsiEncoding` name and silently falls through to
/// `text.as_bytes()` — which is exactly the bug. Owning the table also keeps the encoding
/// correct across lopdf upgrades.
///
/// Characters WinAnsi cannot represent are replaced with `?`.
fn encode_win_ansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| win_ansi_byte(c).unwrap_or(b'?'))
        .collect()
}

/// The single WinAnsi byte for `c`, or `None` if the encoding has no such character.
fn win_ansi_byte(c: char) -> Option<u8> {
    // WinAnsi agrees with ASCII in 0x20..=0x7E and with Latin-1 in 0xA0..=0xFF. It differs
    // only in 0x80..=0x9F, where Latin-1 has C1 control codes and WinAnsi has typographic
    // punctuation — those are spelled out below.
    match c as u32 {
        cp @ 0x20..=0x7E => return Some(cp as u8),
        cp @ 0xA0..=0xFF => return Some(cp as u8),
        _ => {}
    }

    Some(match c {
        '\u{20AC}' => 0x80, // € euro
        '\u{201A}' => 0x82, // ‚ single low-9 quote
        '\u{0192}' => 0x83, // ƒ florin
        '\u{201E}' => 0x84, // „ double low-9 quote
        '\u{2026}' => 0x85, // … ellipsis
        '\u{2020}' => 0x86, // † dagger
        '\u{2021}' => 0x87, // ‡ double dagger
        '\u{02C6}' => 0x88, // ˆ circumflex
        '\u{2030}' => 0x89, // ‰ per mille
        '\u{0160}' => 0x8A, // Š S caron
        '\u{2039}' => 0x8B, // ‹ single left angle quote
        '\u{0152}' => 0x8C, // Œ OE ligature
        '\u{017D}' => 0x8E, // Ž Z caron
        '\u{2018}' => 0x91, // ' left single quote
        '\u{2019}' => 0x92, // ' right single quote
        '\u{201C}' => 0x93, // " left double quote
        '\u{201D}' => 0x94, // " right double quote
        '\u{2022}' => 0x95, // • bullet
        '\u{2013}' => 0x96, // – en dash
        '\u{2014}' => 0x97, // — em dash
        '\u{02DC}' => 0x98, // ˜ small tilde
        '\u{2122}' => 0x99, // ™ trademark
        '\u{0161}' => 0x9A, // š s caron
        '\u{203A}' => 0x9B, // › single right angle quote
        '\u{0153}' => 0x9C, // œ oe ligature
        '\u{017E}' => 0x9E, // ž z caron
        '\u{0178}' => 0x9F, // Ÿ Y diaeresis
        _ => return None,
    })
}

/// Encode a run of text for whichever font is currently selected.
///
/// The two font classes have completely different byte-level contracts, and getting them
/// crossed is silent — the page still renders, it just renders the wrong thing:
///
/// - **External fonts** are Identity-H CID fonts, so the string is a sequence of
///   big-endian `u16` glyph ids (renumbered when the font was subset).
/// - **Built-in fonts** are `/WinAnsiEncoding`, so the string is single-byte WinAnsi.
fn encode_text_for_font(
    text: &str,
    font_info: Option<&RuntimeFontInfo>,
    builtin_font: Option<&BuiltinFont>,
) -> lopdf::Object {
    if let Some(font_info) = font_info {
        let bytes: Vec<u8> = text
            .chars()
            .flat_map(|c| {
                let orig = font_info.parsed_font.lookup_glyph_index(c as u32).unwrap_or(0);
                remap_gid(Some(font_info), orig).to_be_bytes()
            })
            .collect();
        // CID strings are always hex — a raw glyph id byte pair is not printable text.
        return LoString(bytes, Hexadecimal);
    }

    // No external font selected: the 14 standard fonts (and the "no font set" fallback)
    // are all WinAnsi.
    let _ = builtin_font;
    let bytes = encode_win_ansi(text);
    let format = if needs_hex_encoding(&bytes) {
        Hexadecimal
    } else {
        Literal
    };
    LoString(bytes, format)
}

/// Runtime font information for text encoding
/// This is used during PDF serialization to look up glyph IDs
#[derive(Debug, Clone)]
pub(crate) struct RuntimeFontInfo {
    pub parsed_font: ParsedFont,
    /// When the font is subset, maps each original glyph id -> the renumbered
    /// subset glyph id. The content stream must emit the subset ids so they line
    /// up with the renumbered font program, its `/W` widths and `/ToUnicode`
    /// (all keyed by the new ids). `None` when the full font is embedded — then
    /// the content keeps original gids (which match the un-renumbered font).
    pub gid_remap: Option<BTreeMap<u16, u16>>,
}

/// Font subsetting information computed at serialization time (when subsetting is enabled)
#[derive(Debug, Clone)]
pub(crate) struct RuntimeSubsetInfo {
    pub original_font: ParsedFont,
    pub subset_font_bytes: Vec<u8>,
    pub cid_to_unicode_map: String,
    pub widths_list: Vec<lopdf::Object>,
    pub ascent: i64,
    pub descent: i64,
    /// Original glyph id -> the Identity-H code the content stream must emit.
    ///
    /// That code is the renumbered subset glyph id when the font was subset, and —
    /// for CID-keyed CFF fonts — the CID the embedded font's charset assigns to the
    /// glyph (viewers resolve codes through that charset, #280). Empty when codes
    /// stay the original glyph ids (full TTF / name-keyed CFF embed).
    pub gid_remap: BTreeMap<u16, u16>,
    /// Whether the embedded font program was actually subset (renumbered). Controls
    /// the `XXXXXX+` subset tag on /BaseFont and /FontName — a full embed must not
    /// carry one.
    pub was_subset: bool,
}

/// Analyze all PDF operations to collect used glyph IDs for each font - now trivial with Codepoint!
fn collect_used_glyphs_from_pages(
    pages: &[PdfPage],
    fonts: &BTreeMap<FontId, crate::font::PdfFont>,
) -> BTreeMap<FontId, BTreeMap<u16, String>> {
    let mut used_glyphs: BTreeMap<FontId, BTreeMap<u16, String>> = BTreeMap::new();
    
    for page in pages {
        let mut current_font_id: Option<FontId> = None;
        
        for op in &page.ops {
            match op {
                Op::SetFont { font, .. } => {
                    // Track the current font for subsequent text operations
                    match font {
                        crate::ops::PdfFontHandle::External(font_id) => {
                            current_font_id = Some(font_id.clone());
                        }
                        crate::ops::PdfFontHandle::Builtin(_) => {
                            current_font_id = None; // Builtin fonts don't need subsetting
                        }
                    }
                }
                Op::ShowText { items } => {
                    if let Some(ref font_id) = current_font_id {
                        if let Some(pdf_font) = fonts.get(font_id) {
                            let font_glyphs = used_glyphs.entry(font_id.clone()).or_insert_with(BTreeMap::new);
                            
                            for item in items {
                                match item {
                                    TextItem::Text(text) => {
                                        // Convert each character to glyph ID using the font
                                        for c in text.chars() {
                                            if let Some(glyph_id) = pdf_font.parsed_font.lookup_glyph_index(c as u32) {
                                                font_glyphs.insert(glyph_id, c.to_string());
                                            }
                                        }
                                    }
                                    TextItem::GlyphIds(glyphs) => {
                                        // Direct glyph IDs - get the unicode codepoint from CID
                                        for codepoint in glyphs {
                                            let character = if let Some(ref cid) = codepoint.cid {
                                                // The FULL extraction text for this glyph — several
                                                // chars for ligatures ("fi"). Truncating to the first
                                                // char here used to break copy-paste of every ligated
                                                // word in regenerated ToUnicode CMaps.
                                                cid.clone()
                                            } else {
                                                // Try reverse lookup from the font's cache
                                                pdf_font.parsed_font.get_glyph_primary_char(codepoint.gid)
                                                    .map(|c| c.to_string())
                                                    .unwrap_or_else(|| "\u{FFFD}".to_string())
                                            };
                                            // #261: always record the glyph, even when the
                                            // reverse char lookup failed (cid is None and the
                                            // font has no primary char for this gid). Otherwise
                                            // the glyph is silently dropped from the used-glyph
                                            // map and the font is never embedded, producing a PDF
                                            // that references missing glyphs. The fallback char
                                            // ('\u{FFFD}') only affects the ToUnicode mapping.
                                            font_glyphs.insert(codepoint.gid, character);
                                        }
                                    }
                                    TextItem::Offset(_) => {
                                        // Offsets don't contribute glyphs
                                    }
                                }
                            }
                        }
                    }
                }
                // `'` and `"` show text just as `Tj` does. They were missing here, so a
                // page that drew text *only* through them registered zero used glyphs for
                // its font — and `prepare_fonts_for_serialization` skips fonts with no used
                // glyphs, so the font was never embedded and the text silently vanished.
                Op::MoveToNextLineShowText { text }
                | Op::SetSpacingMoveAndShowText { text, .. } => {
                    if let Some(ref font_id) = current_font_id {
                        if let Some(pdf_font) = fonts.get(font_id) {
                            let font_glyphs = used_glyphs
                                .entry(font_id.clone())
                                .or_insert_with(BTreeMap::new);

                            for c in text.chars() {
                                if let Some(glyph_id) =
                                    pdf_font.parsed_font.lookup_glyph_index(c as u32)
                                {
                                    font_glyphs.insert(glyph_id, c.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Other operations don't affect glyph usage
                }
            }
        }
    }

    used_glyphs
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
                            control1.p.x.into(),
                            control1.p.y.into(),
                            control2.p.x.into(),
                            control2.p.y.into(),
                            end_point.p.x.into(),
                            end_point.p.y.into(),
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
                                control1.p.x.into(),
                                control1.p.y.into(),
                                control2.p.x.into(),
                                control2.p.y.into(),
                                end_point.p.x.into(),
                                end_point.p.y.into(),
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

fn rectangle_to_stream_ops(rectangle: &crate::Rect) -> Vec<LoOp> {
    use crate::graphics::{PaintMode, WindingOrder};

    let mut operations = Vec::new();

    // x, y, width, height. `re` appends a complete (closed) rectangle subpath.
    operations.push(LoOp::new(
        "re",
        vec![
            rectangle.x.into(),
            rectangle.y.into(),
            rectangle.width.into(),
            rectangle.height.into()
        ],
    ));

    // Honor the rectangle's paint mode (mirrors `polygon_to_stream_ops`), instead
    // of always emitting `n` (which paints nothing / makes the rectangle invisible).
    // #259
    let winding = rectangle.winding_order.unwrap_or(WindingOrder::NonZero);
    match rectangle.mode {
        Some(PaintMode::Clip) => {
            // Set the rectangle as a clip path, then end the path with `n`.
            operations.push(LoOp::new(winding.get_clip_op(), vec![]));
            operations.push(LoOp::new("n", vec![]));
        }
        Some(PaintMode::Fill) => {
            operations.push(LoOp::new(winding.get_fill_op(), vec![]));
        }
        Some(PaintMode::Stroke) => {
            operations.push(LoOp::new("S", vec![]));
        }
        Some(PaintMode::FillStroke) => {
            operations.push(LoOp::new(winding.get_fill_stroke_op(), vec![]));
        }
        None => {
            // No explicit paint mode: preserve the legacy behavior of applying any
            // winding_order as a clip and ending the path without painting.
            match rectangle.winding_order {
                Some(WindingOrder::NonZero) => operations.push(LoOp::new("W", vec![])),
                Some(WindingOrder::EvenOdd) => operations.push(LoOp::new("W*", vec![])),
                None => {}
            }
            operations.push(LoOp::new("n", vec![]));
        }
    }

    operations
}

/// Prepare fonts for serialization
/// Returns:
/// - font_infos: Used by translate_operations for glyph ID lookup
/// - subset_infos: Used for font dictionary creation (full fonts or subsetted)
pub(crate) fn prepare_fonts_for_serialization(
    resources: &PdfResources,
    pages: &[PdfPage],
    do_subset: bool,
    warnings: &mut Vec<PdfWarnMsg>,
) -> (BTreeMap<FontId, RuntimeFontInfo>, BTreeMap<FontId, RuntimeSubsetInfo>) {
    let mut font_infos = BTreeMap::new();
    let mut subset_infos = BTreeMap::new();
    
    // First pass: collect used glyphs for each font
    let used_glyphs = collect_used_glyphs_from_pages(pages, &resources.fonts.map);
    
    // Second pass: create info for each font
    for (font_id, pdf_font) in &resources.fonts.map {
        let glyph_usage = used_glyphs.get(font_id).cloned().unwrap_or_default();
        
        if glyph_usage.is_empty() {
            continue; // Skip unused fonts
        }
        
        // Always create RuntimeFontInfo for text encoding. `gid_remap` is filled
        // below iff this font is actually subset (renumbered).
        font_infos.insert(font_id.clone(), RuntimeFontInfo {
            parsed_font: pdf_font.parsed_font.clone(),
            gid_remap: None,
        });
        
        // Create RuntimeSubsetInfo for font dictionary
        #[cfg(feature = "text_layout")]
        let subset_info = if do_subset &&
                             pdf_font.meta.requires_subsetting &&
                             pdf_font.meta.embedding_mode == crate::font::FontEmbeddingMode::Subset {
            // Try subsetting, fall back to full font if it fails
            match create_subset_runtime_info(font_id, pdf_font, &glyph_usage, warnings) {
                Some(info) => info,
                None => create_full_font_runtime_info(font_id, pdf_font, &glyph_usage, warnings),
            }
        } else {
            // Use full font without subsetting
            create_full_font_runtime_info(font_id, pdf_font, &glyph_usage, warnings)
        };

        #[cfg(not(feature = "text_layout"))]
        let subset_info = create_full_font_runtime_info(font_id, pdf_font, &glyph_usage, warnings);

        // If the font was actually subset (renumbered), the content stream must
        // emit the subset glyph ids — thread the remap into the text encoder.
        if !subset_info.gid_remap.is_empty() {
            if let Some(fi) = font_infos.get_mut(font_id) {
                fi.gid_remap = Some(subset_info.gid_remap.clone());
            }
        }

        subset_infos.insert(font_id.clone(), subset_info);
    }
    
    (font_infos, subset_infos)
}

/// Create subset info by actually subsetting the font (requires text_layout feature)
#[cfg(feature = "text_layout")]
fn create_subset_runtime_info(
    font_id: &FontId,
    pdf_font: &crate::font::PdfFont,
    glyph_usage: &BTreeMap<u16, String>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Option<RuntimeSubsetInfo> {
    let subset_result = crate::font::subset_font(&pdf_font.parsed_font, glyph_usage);
    
    match subset_result {
        Ok(subset) => {
            let mut font_warnings = Vec::new();
            if let Some(subset_font) = ParsedFont::from_bytes(&subset.bytes, 0, &mut font_warnings) {

                // The original->new gid renumbering comes from `subset.glyph_mapping`,
                // which mirrors the exact glyph order handed to allsorts (see
                // `crate::font::subset_font` for why that order is authoritative for
                // both glyf and CFF outlines).
                //
                // It must NOT be recovered from the subset font's cmap: shaper-produced
                // glyphs have no usable char->gid cmap entry — ligature glyphs (the
                // "fi" in "Configure") would resolve to the plain 'f' outline, and
                // glyphs whose codepoint maps to a *different* default glyph (Noto CJK
                // digits used for list markers / page numbers) would resolve to
                // nothing at all, get remapped to gid 0 by `remap_gid`, and render as
                // .notdef boxes (#220 F2b/F5).
                // The Identity-H code for each glyph: the subset gid itself for glyf and
                // name-keyed CFF outlines, but the subset charset's CID for CID-keyed
                // CFF — the allsorts subsetter preserves the ORIGINAL font's CIDs there,
                // and spec-following viewers (Acrobat, Preview) resolve our codes
                // through that charset (#280). Content stream, /W and /ToUnicode must
                // all be keyed by these codes.
                let subset_gid_to_code =
                    crate::font::cff_charset_gid_to_cid_map(&subset.bytes, 0);

                let mut gid_remap: BTreeMap<u16, u16> = BTreeMap::new();
                let mut coded_glyphs: Vec<(u16, u16, String)> = Vec::new();
                for (orig_gid, (new_gid, ch)) in subset.glyph_mapping.iter() {
                    let code = subset_gid_to_code
                        .as_ref()
                        .and_then(|m| m.get(new_gid).copied())
                        .unwrap_or(*new_gid);
                    gid_remap.insert(*orig_gid, code);
                    coded_glyphs.push((code, *new_gid, ch.clone()));
                }
                // The /W run-length groups and ToUnicode bfranges need ascending codes.
                // Subset gids ascend with original gids, but charset CIDs need not.
                coded_glyphs.sort_by(|a, b| a.0.cmp(&b.0));

                let coded_unicode: Vec<(u16, String)> = coded_glyphs
                    .iter()
                    .map(|(code, _, ch)| (*code, ch.clone()))
                    .collect();
                let cid_to_unicode_map = crate::font::generate_cmap_string(
                    &subset_font,
                    font_id,
                    &coded_unicode
                );

                let width_entries: Vec<(u16, u16)> = coded_glyphs
                    .iter()
                    .map(|(code, new_gid, _)| (*code, *new_gid))
                    .collect();
                let widths =
                    crate::font::get_normalized_widths_codes(&subset_font, &width_entries);

                Some(RuntimeSubsetInfo {
                    original_font: pdf_font.parsed_font.clone(),
                    subset_font_bytes: subset.bytes,
                    cid_to_unicode_map,
                    widths_list: widths,
                    ascent: subset_font.font_metrics.ascent as i64,
                    descent: subset_font.font_metrics.descent as i64,
                    gid_remap,
                    was_subset: true,
                })
            } else {
                warnings.push(PdfWarnMsg::error(0, 0, 
                    format!("Failed to parse subset font for {}", font_id.0)));
                None
            }
        }
        Err(e) => {
            warnings.push(PdfWarnMsg::error(0, 0, 
                format!("Failed to subset font {}: {}", font_id.0, e)));
            None
        }
    }
}

/// Create runtime info using the full font without subsetting
/// This is used as a fallback when subsetting fails or is disabled
fn create_full_font_runtime_info(
    font_id: &FontId,
    pdf_font: &crate::font::PdfFont,
    glyph_usage: &BTreeMap<u16, String>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> RuntimeSubsetInfo {
    // The font program embedded as /FontFile2 (or /FontFile3) *is* these bytes.
    // `ParsedFont::from_bytes` guarantees they're retained; a face that lost them
    // cannot be embedded, and writing a zero-length font stream produces a PDF that
    // readers reject outright ("Cannot extract the embedded font"). Report it as an
    // error and let the caller decide — never embed silently-empty font data.
    #[cfg(feature = "text_layout")]
    let font_bytes = pdf_font
        .parsed_font
        .source_bytes()
        .map(|fb| fb.as_slice().to_vec())
        .unwrap_or_default();
    #[cfg(not(feature = "text_layout"))]
    let font_bytes = pdf_font.parsed_font.original_bytes.clone();

    if font_bytes.is_empty() {
        warnings.push(PdfWarnMsg::error(
            0,
            0,
            format!(
                "font {} has no source bytes and will NOT be embedded; construct it with \
                 printpdf::ParsedFont::from_bytes so the sfnt bytes are retained",
                font_id.0
            ),
        ));
    }

    #[cfg(feature = "text_layout")]
    let font_index = pdf_font.parsed_font.original_index;
    #[cfg(not(feature = "text_layout"))]
    let font_index = pdf_font.parsed_font.font_index as usize;

    // A collection (`ttcf`) is not a valid font program: repackage the selected face
    // as a standalone sfnt. (The face keeps its glyph ids, so nothing downstream —
    // codes, /W, ToUnicode — changes.) If extraction fails, embed the collection
    // as-is and say so, rather than silently shipping bytes readers reject.
    let font_bytes = if font_bytes.get(..4) == Some(b"ttcf".as_slice()) {
        match crate::font::extract_collection_face(&font_bytes, font_index) {
            Some(face) => face,
            None => {
                warnings.push(PdfWarnMsg::error(
                    0,
                    0,
                    format!(
                        "font {}: cannot extract face {} from font collection; embedding \
                         the whole collection, which most PDF readers reject",
                        font_id.0, font_index
                    ),
                ));
                font_bytes
            }
        }
    } else {
        font_bytes
    };

    // For a CID-keyed CFF the Identity-H codes must be the charset's CIDs, not glyph
    // ids — spec-following viewers resolve every code through the embedded charset,
    // which is not identity in real fonts (NotoSansJP diverges from gid 365 on; #280).
    // `None` for glyf and name-keyed CFF fonts, where code == gid stays correct.
    //
    // Index 0: `font_bytes` is a single face at this point (see above).
    let gid_to_code = crate::font::cff_charset_gid_to_cid_map(&font_bytes, 0);

    let coded_glyphs: Vec<(u16, u16, String)> = {
        let mut v: Vec<(u16, u16, String)> = glyph_usage
            .iter()
            .map(|(gid, s)| {
                let code = gid_to_code
                    .as_ref()
                    .and_then(|m| m.get(gid).copied())
                    .unwrap_or(*gid);
                (code, *gid, s.clone())
            })
            .collect();
        // /W run groups and ToUnicode bfranges need ascending codes; charset CIDs
        // need not ascend with gids.
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };

    let coded_unicode: Vec<(u16, String)> = coded_glyphs
        .iter()
        .map(|(code, _, s)| (*code, s.clone()))
        .collect();
    let cid_to_unicode_map = crate::font::generate_cmap_string(
        &pdf_font.parsed_font,
        font_id,
        &coded_unicode
    );

    let width_entries: Vec<(u16, u16)> = coded_glyphs
        .iter()
        .map(|(code, gid, _)| (*code, *gid))
        .collect();
    let widths = get_normalized_widths(&pdf_font.parsed_font, &width_entries);

    RuntimeSubsetInfo {
        original_font: pdf_font.parsed_font.clone(),
        subset_font_bytes: font_bytes,
        cid_to_unicode_map,
        widths_list: widths,
        ascent: pdf_font.parsed_font.font_metrics.ascent as i64,
        descent: pdf_font.parsed_font.font_metrics.descent as i64,
        // No renumbering happened, but for CID-keyed CFF the content stream must
        // still emit charset CIDs instead of gids — thread the full map through the
        // same remap mechanism. Empty (= codes stay original gids) otherwise.
        gid_remap: gid_to_code.unwrap_or_default(),
        was_subset: false,
    }
}

/// Look up the advance width for a glyph ID, normalized to 1000 units per em.
fn get_scaled_glyph_width(font: &crate::font::ParsedFont, gid: u16) -> i64 {
    #[cfg(feature = "text_layout")]
    let raw_width = font.get_or_decode_glyph(gid).map(|g| g.horz_advance);
    #[cfg(not(feature = "text_layout"))]
    let raw_width = font.get_glyph_width(gid);

    let units_per_em = font.pdf_font_metrics.units_per_em as f32;
    raw_width
        .map(|w| (w as f32 * 1000.0 / units_per_em) as i64)
        .unwrap_or(0)
}

/// Generate the CID `/W` array with run-length encoding, from entries of
/// `(Identity-H code, gid to fetch the width of)` sorted ascending by code.
///
/// Code == gid except for CID-keyed CFF fonts, where the code is the CID the embedded
/// charset assigns to the glyph (see `font::cff_charset_gid_to_cid_map`).
fn get_normalized_widths(
    font: &crate::font::ParsedFont,
    entries: &[(u16, u16)],
) -> Vec<lopdf::Object> {
    let mut widths_list = Vec::new();
    let mut current_low_code = 0u16;
    let mut current_high_code = 0u16;
    let mut current_width_vec: Vec<i64> = Vec::new();

    for &(code, gid) in entries {
        let glyph_width = get_scaled_glyph_width(font, gid);

        if current_width_vec.is_empty() {
            current_low_code = code;
            current_high_code = code;
            current_width_vec.push(glyph_width);
        } else if code == current_high_code + 1 {
            current_high_code = code;
            current_width_vec.push(glyph_width);
        } else {
            widths_list.push(lopdf::Object::Integer(current_low_code as i64));
            widths_list.push(lopdf::Object::Array(
                current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
            ));
            current_low_code = code;
            current_high_code = code;
            current_width_vec = vec![glyph_width];
        }
    }

    if !current_width_vec.is_empty() {
        widths_list.push(lopdf::Object::Integer(current_low_code as i64));
        widths_list.push(lopdf::Object::Array(
            current_width_vec.iter().map(|w| lopdf::Object::Integer(*w)).collect(),
        ));
    }

    widths_list
}

fn add_subset_font_to_pdf(
    doc: &mut lopdf::Document,
    font_id: &FontId,
    subset_info: &RuntimeSubsetInfo,
) -> LoDictionary {
    let font_name = subset_info.original_font
        .font_name
        .clone()
        .unwrap_or(font_id.0.clone());

    // The `XXXXXX+` subset tag (ISO 32000-1, 9.6.4) marks a font program whose glyph
    // complement was reduced. It must NOT appear on a full embed — and it must appear
    // identically on /BaseFont (both levels) and the descriptor's /FontName, which are
    // required to match.
    let face_name = if subset_info.was_subset {
        format!("{}+{}", font_id.0.clone().get(0..6).unwrap_or(&font_id.0), font_name)
    } else {
        font_name.clone()
    };

    // The descendant subtype and the /FontFile key must describe what the embedded bytes
    // ACTUALLY are, so decide from the sfnt magic of the program we are about to write —
    // not from `ParsedFont::font_type`, which reports `TrueType` even for `OTTO` faces
    // and so mislabelled every CFF font as `CIDFontType2` + `/FontFile2` (both of which
    // mean "TrueType `glyf` outlines").
    //
    // `font_tuple` is None when there is no font program to embed at all. A FontDescriptor
    // with *no* /FontFile entry is a legal non-embedded font that readers substitute for;
    // a /FontFile entry pointing at a zero-length stream is a corrupt font that they refuse
    // ("Cannot extract the embedded font ..."). Never emit the latter —
    // `create_full_font_runtime_info` has already logged that as an error.
    let program = &subset_info.subset_font_bytes;
    let has_font_program = !program.is_empty();

    // For a CID-keyed CFF, embed the bare `CFF ` table instead of the whole sfnt:
    // it is the only wrapper under which every viewer family resolves our
    // charset-CID codes the same way — see `font::extract_cid_keyed_cff` for the
    // full story. The extracted bytes are the verbatim table the codes were
    // derived from, so program and codes stay consistent by construction.
    let bare_cid_keyed_cff = crate::font::extract_cid_keyed_cff(program, 0);
    let is_otto = program.starts_with(b"OTTO");

    let (sub_type, font_tuple) = if let Some(cff_table) = bare_cid_keyed_cff {
        // A bare CFF font program goes in /FontFile3 as /Subtype /CIDFontType0C —
        // for bare table bytes (unlike a whole sfnt), that name is the truthful one.
        let font_tuple = has_font_program.then(|| {
            let font_stream = LoStream::new(
                LoDictionary::from_iter(vec![("Subtype", Name("CIDFontType0C".into()))]),
                cff_table,
            )
            .with_compression(false);

            ("FontFile3", Reference(doc.add_object(font_stream)))
        });

        ("CIDFontType0", font_tuple)
    } else if is_otto {
        // A whole OTTO sfnt (name-keyed CFF outlines: codes are glyph ids in every
        // viewer) goes in /FontFile3 as /Subtype /OpenType (PDF 1.6+).
        // /CIDFontType0C would be a lie: that name means a *bare* CFF table, not an sfnt.
        let font_tuple = has_font_program.then(|| {
            let font_stream = LoStream::new(
                LoDictionary::from_iter(vec![("Subtype", Name("OpenType".into()))]),
                program.clone(),
            )
            .with_compression(false);

            ("FontFile3", Reference(doc.add_object(font_stream)))
        });

        ("CIDFontType0", font_tuple)
    } else {
        // TrueType font stream must not be compressed
        let font_tuple = has_font_program.then(|| {
            let font_stream = LoStream::new(LoDictionary::new(), program.clone())
                .with_compression(false);

            ("FontFile2", Reference(doc.add_object(font_stream)))
        });

        ("CIDFontType2", font_tuple)
    };

    // #271: PDF font descriptor metrics (Ascent/Descent/FontBBox/CapHeight) must be
    // expressed in glyph-space units of 1/1000 em, but the font tables store them in
    // the font's own units-per-em (often 2048). Scale everything by 1000/units_per_em.
    let upm = subset_info.original_font.pdf_font_metrics.units_per_em;
    let font_scale = if upm == 0 { 1.0 } else { 1000.0 / upm as f32 };
    let scale_i64 = |v: i64| (v as f32 * font_scale).round() as i64;
    let scale_i16 = |v: i16| (v as f32 * font_scale).round() as i64;

    let ascent_scaled = scale_i64(subset_info.ascent);
    let descent_scaled = scale_i64(subset_info.descent);
    let bbox_x_min = scale_i16(subset_info.original_font.pdf_font_metrics.x_min);
    let bbox_y_min = scale_i16(subset_info.original_font.pdf_font_metrics.y_min);
    let bbox_x_max = scale_i16(subset_info.original_font.pdf_font_metrics.x_max);
    let bbox_y_max = scale_i16(subset_info.original_font.pdf_font_metrics.y_max);

    // CapHeight: prefer the real OS/2 sCapHeight (font units, scaled to 1000/em).
    // Fall back to the scaled ascent when the font doesn't expose sCapHeight (or when
    // text_layout is disabled and metrics are unavailable) — still far better than 0.
    #[cfg(feature = "text_layout")]
    let cap_height = subset_info
        .original_font
        .font_metrics
        .cap_height
        .map(|c| (c * font_scale).round() as i64)
        .unwrap_or(ascent_scaled);
    #[cfg(not(feature = "text_layout"))]
    let cap_height = ascent_scaled;

    // #271 (residual): ItalicAngle, Flags and StemV used to be hardcoded to 0 / 32 / 80,
    // i.e. "upright, non-symbolic, medium weight" for every font ever embedded. Readers
    // use these when they have to synthesise or substitute a face, so a bold italic that
    // claims to be upright and regular gets substituted badly.
    let m = &subset_info.original_font.pdf_font_metrics;

    // ItalicAngle is the slant in degrees, counter-clockwise from vertical — so an italic
    // face is *negative*. hhea gives the caret slope as a rise/run vector.
    let italic_angle = if m.caret_slope_rise == 0 {
        0
    } else {
        (-(m.caret_slope_run as f32).atan2(m.caret_slope_rise as f32).to_degrees()).round() as i64
    };

    // FontDescriptor /Flags (PDF 32000-1 Table 123).
    //
    // Italic is decided from the caret slope alone. `pdf_font_metrics.font_flags` is
    // head.*flags*, NOT head.macStyle — its bit 1 means "left sidebearing point at x=0",
    // which most fonts set, so testing it flags every font as italic.
    const FLAG_NONSYMBOLIC: i64 = 1 << 5; // uses the standard Latin character set
    const FLAG_ITALIC: i64 = 1 << 6;
    let flags = FLAG_NONSYMBOLIC | if italic_angle != 0 { FLAG_ITALIC } else { 0 };

    // StemV is the vertical stem thickness. No font table carries it, so estimate it from
    // OS/2 usWeightClass the way PDF producers conventionally do: ~88 at weight 400,
    // ~166 at 700. Fall back to the old constant when OS/2 is absent (usWeightClass 0).
    let stem_v = if m.us_weight_class == 0 {
        80
    } else {
        50 + ((m.us_weight_class as f32 / 65.0).powi(2)).round() as i64
    };

    LoDictionary::from_iter(vec![
        ("Type", Name("Font".into())),
        ("Subtype", Name("Type0".into())),
        ("BaseFont", Name(face_name.clone().into_bytes())),
        ("Encoding", Name("Identity-H".into())),
        (
            "ToUnicode",
            Reference(doc.add_object(LoStream::new(
                LoDictionary::new(),
                subset_info.cid_to_unicode_map.as_bytes().to_vec(),
            ))),
        ),
        (
            "DescendantFonts",
            Array(vec![Dictionary(LoDictionary::from_iter(
                vec![
                    ("Type", Name("Font".into())),
                    ("BaseFont", Name(face_name.clone().into_bytes())),
                    ("Subtype", Name(sub_type.into())),
                    (
                        "CIDSystemInfo",
                        Dictionary(LoDictionary::from_iter(vec![
                            ("Registry", LoString("Adobe".into(), Literal)),
                            ("Ordering", LoString("Identity".into(), Literal)),
                            ("Supplement", Integer(0)),
                        ])),
                    ),
                    ("W", Array(subset_info.widths_list.clone())),
                    ("DW", Integer(DEFAULT_CHARACTER_WIDTH)),
                    (
                        "FontDescriptor",
                        Reference(doc.add_object(LoDictionary::from_iter(
                            [
                                ("Type", Name("FontDescriptor".into())),
                                ("FontName", Name(face_name.clone().into_bytes())),
                                ("Ascent", Integer(ascent_scaled)),
                                ("Descent", Integer(descent_scaled)),
                                ("CapHeight", Integer(cap_height)),
                                ("ItalicAngle", Integer(italic_angle)),
                                ("Flags", Integer(flags)),
                                ("StemV", Integer(stem_v)),
                                (
                                    "FontBBox",
                                    Array(vec![
                                        Integer(bbox_x_min),
                                        Integer(bbox_y_min),
                                        Integer(bbox_x_max),
                                        Integer(bbox_y_max),
                                    ]),
                                ),
                            ]
                            .into_iter()
                            .chain(font_tuple),
                        ))),
                    ),
                ]
                .into_iter()
                .chain(
                    // Identity is the spec default, but only CIDFontType2 may carry
                    // the key at all — writing it explicitly costs nothing and keeps
                    // strict validators quiet. CIDFontType0 resolves CIDs through the
                    // CFF charset instead (which is why the codes we emit are charset
                    // CIDs for CID-keyed faces — see RuntimeSubsetInfo::gid_remap).
                    (sub_type == "CIDFontType2")
                        .then(|| ("CIDToGIDMap", Name("Identity".into()))),
                ),
            ))]),
        ),
    ])
}

fn docinfo_to_dict(m: &PdfDocumentInfo) -> LoDictionary {
    let trapping = if m.trapped { "True" } else { "False" };
    let gts_pdfx_version = m.conformance.get_identifier_string();

    let info_mod_date = crate::utils::to_pdf_time_stamp_metadata(&m.modification_date);
    let info_create_date = crate::utils::to_pdf_time_stamp_metadata(&m.creation_date);

    let creation_date = LoString(info_create_date.into_bytes(), Literal);
    let identifier = LoString(m.identifier.as_bytes().to_vec(), Literal);

    LoDictionary::from_iter(vec![
        ("Trapped", trapping.into()),
        ("CreationDate", creation_date),
        ("ModDate", LoString(info_mod_date.into_bytes(), Literal)),
        (
            "GTS_PDFXVersion",
            LoString(gts_pdfx_version.into(), Literal),
        ),
        ("Title", encode_text_to_utf16be(&m.document_title)),
        ("Author", encode_text_to_utf16be(&m.author)),
        ("Creator", encode_text_to_utf16be(&m.creator)),
        ("Producer", encode_text_to_utf16be(&m.producer)),
        ("Subject", encode_text_to_utf16be(&m.subject)),
        ("Identifier", identifier),
        ("Keywords", encode_text_to_utf16be(&m.keywords.join(","))),
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

// Encode text to UTF-16BE with BOM
fn encode_text_to_utf16be(text: &str) -> lopdf::Object {
    if text.is_empty() {
        return lopdf::Object::string_literal("");
    }

    // Byte Order Mark
    let mut bytes = vec![0xFE, 0xFF];

    // Encode as UTF-16BE
    for c in text.encode_utf16() {
        bytes.push((c >> 8) as u8);
        bytes.push((c & 0xFF) as u8);
    }

    // Return as a Hex String
    lopdf::Object::String(bytes, Hexadecimal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{PaintMode, WindingOrder};
    use crate::units::Pt;

    fn rect_with(mode: Option<PaintMode>, winding: Option<WindingOrder>) -> crate::Rect {
        crate::Rect {
            x: Pt(0.0),
            y: Pt(0.0),
            width: Pt(10.0),
            height: Pt(10.0),
            mode,
            winding_order: winding,
        }
    }

    fn operators(ops: &[LoOp]) -> Vec<String> {
        ops.iter().map(|o| o.operator.clone()).collect()
    }

    // #259: rectangle_to_stream_ops must honor `rectangle.mode` (emit S/f/B/B*),
    // not always `n` (which makes the rectangle invisible).
    #[test]
    fn rectangle_honors_paint_mode() {
        // Fill
        let ops = rectangle_to_stream_ops(&rect_with(Some(PaintMode::Fill), None));
        assert_eq!(operators(&ops), vec!["re", "f"]);

        // Stroke
        let ops = rectangle_to_stream_ops(&rect_with(Some(PaintMode::Stroke), None));
        assert_eq!(operators(&ops), vec!["re", "S"]);

        // FillStroke
        let ops = rectangle_to_stream_ops(&rect_with(Some(PaintMode::FillStroke), None));
        assert_eq!(operators(&ops), vec!["re", "B"]);

        // Fill with even-odd winding
        let ops = rectangle_to_stream_ops(&rect_with(Some(PaintMode::Fill), Some(WindingOrder::EvenOdd)));
        assert_eq!(operators(&ops), vec!["re", "f*"]);

        // Clip ends the path with `n` after setting the clip operator.
        let ops = rectangle_to_stream_ops(&rect_with(Some(PaintMode::Clip), None));
        assert_eq!(operators(&ops), vec!["re", "W", "n"]);

        // No mode: legacy behavior (no paint, optional clip via winding_order).
        let ops = rectangle_to_stream_ops(&rect_with(None, None));
        assert_eq!(operators(&ops), vec!["re", "n"]);
    }
}

/// WinAnsi encoding of built-in-font text (issue #273).
///
/// Test cases adapted from @JohnHarrison's PR #274, which reported and diagnosed the bug.
/// The fix here differs — printpdf owns the encoding table rather than relying on lopdf's
/// `SimpleEncoding` name lookup, so it stays correct across lopdf upgrades — but these
/// assertions are the same contract.
#[cfg(test)]
mod win_ansi_tests {
    use super::encode_win_ansi as encode;

    #[test]
    fn ascii_is_unchanged() {
        assert_eq!(encode("Hello, World!"), b"Hello, World!".to_vec());
    }

    #[test]
    fn accented_names_become_single_winansi_bytes() {
        // These are the bytes any WinAnsi reader expects — NOT the UTF-8 bytes.
        assert_eq!(encode("é"), vec![0xE9]);
        assert_eq!(encode("ü"), vec![0xFC]);
        assert_eq!(encode("ñ"), vec![0xF1]);
        assert_eq!(encode("ç"), vec![0xE7]);
        assert_eq!(encode("José"), vec![b'J', b'o', b's', 0xE9]);
        assert_eq!(encode("Grüße"), vec![b'G', b'r', 0xFC, 0xDF, b'e']);
    }

    #[test]
    fn typographic_punctuation_becomes_single_winansi_bytes() {
        // 0x80..=0x9F is where WinAnsi diverges from Latin-1.
        assert_eq!(encode("“"), vec![0x93]); // left double quote
        assert_eq!(encode("”"), vec![0x94]); // right double quote
        assert_eq!(encode("’"), vec![0x92]); // right single quote
        assert_eq!(encode("–"), vec![0x96]); // en dash
        assert_eq!(encode("—"), vec![0x97]); // em dash
        assert_eq!(encode("…"), vec![0x85]); // ellipsis
        assert_eq!(encode("•"), vec![0x95]); // bullet
        assert_eq!(encode("€"), vec![0x80]); // euro
        assert_eq!(encode("™"), vec![0x99]); // trademark
        // ...while these live in the Latin-1 range and pass straight through.
        assert_eq!(encode("×"), vec![0xD7]); // multiplication sign
        assert_eq!(encode("·"), vec![0xB7]); // middle dot
    }

    #[test]
    fn does_not_emit_utf8_for_non_ascii() {
        // The regression guard. `Encoding::SimpleEncoding(b"WinAnsiEncoding")` is a name
        // lopdf 0.39 does not recognise, so it fell through to `text.as_bytes()` and `é`
        // came out as the two bytes `C3 A9` — mojibake in every reader.
        assert_ne!(encode("é"), vec![0xC3, 0xA9]);
        assert_ne!(encode("–"), vec![0xE2, 0x80, 0x93]);
    }

    #[test]
    fn unrepresentable_characters_do_not_corrupt_the_stream() {
        // WinAnsi is single-byte: a CJK character has no encoding. It must not silently
        // become multiple bytes, which would shift every subsequent glyph.
        assert_eq!(encode("世"), vec![b'?']);
        assert_eq!(encode("a世b"), vec![b'a', b'?', b'b']);
    }

    #[test]
    fn every_encoded_character_is_exactly_one_byte() {
        for text in ["Hello", "José", "Grüße", "€ — …", "世界"] {
            assert_eq!(
                encode(text).len(),
                text.chars().count(),
                "WinAnsi is a single-byte encoding; {text:?} must not change length"
            );
        }
    }
}
