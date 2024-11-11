use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::color::IccProfile;
use crate::font::SubsetFont;
use crate::Actions;
use crate::BuiltinFont;
use crate::Color;
use crate::ColorArray;
use crate::Destination;
use crate::FontId;
use crate::IccProfileType;
use crate::Line;
use crate::LinkAnnotation;
use crate::Op;
use crate::PaintMode;
use crate::ParsedFont;
use crate::PdfDocument;
use crate::PdfDocumentInfo;
use crate::PdfPage;
use crate::PdfResources;
use crate::Polygon;
use crate::XObject;
use crate::XObjectId;
use lopdf::content::Operation as LoOp;
use lopdf::Dictionary as LoDictionary;
use lopdf::Object::{
    Array, Dictionary, Integer, Name, Null, Real, Reference, Stream, String as LoString,
};
use lopdf::Stream as LoStream;
use lopdf::StringFormat::{Hexadecimal, Literal};
use serde_derive::Deserialize;
use serde_derive::Serialize;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PdfSaveOptions {
    pub optimize: bool,
    pub subset_fonts: bool,
}

impl Default for PdfSaveOptions {
    fn default() -> Self {
        Self {
            optimize: !(std::cfg!(debug_assertions)),
            subset_fonts: true,
        }
    }
}

pub fn serialize_pdf_into_bytes(pdf: &PdfDocument, opts: &PdfSaveOptions) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.3");
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
    if !pdf.resources.layers.map.is_empty() {
        let layer_ids = pdf
            .resources
            .layers
            .map
            .iter()
            .map(|(id, s)| {
                let usage_ocg_dict = LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    (
                        "CreatorInfo",
                        Dictionary(LoDictionary::from_iter(vec![
                            ("Creator", LoString(s.creator.clone().into(), Literal)),
                            ("Subtype", Name(s.usage.to_string().into())),
                        ])),
                    ),
                ]);

                let usage_ocg_dict_ref = doc.add_object(Dictionary(usage_ocg_dict));
                let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);
                let intent_arr_ref = doc.add_object(intent_arr);

                let pdf_id = doc.add_object(Dictionary(LoDictionary::from_iter(vec![
                    ("Type", Name("OCG".into())),
                    ("Name", LoString(s.name.to_string().into(), Literal)), // TODO: non-ASCII layer names!
                    ("Intent", Reference(intent_arr_ref)),
                    ("Usage", Reference(usage_ocg_dict_ref)),
                ])));

                (id.clone(), pdf_id)
            })
            .collect::<BTreeMap<_, _>>();

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

    // Build fonts dictionary
    let mut global_font_dict = LoDictionary::new();
    let prepared_fonts = prepare_fonts(&pdf.resources, &pdf.pages);
    for (font_id, prepared) in prepared_fonts.iter() {
        let font_dict = add_font_to_pdf(&mut doc, &font_id, &prepared);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(font_id.0.clone(), Reference(font_dict_id));
    }

    for internal_font in get_used_internal_fonts(&pdf.pages) {
        let font_dict = builtin_font_to_dict(&internal_font);
        let font_dict_id = doc.add_object(font_dict);
        global_font_dict.set(internal_font.get_pdf_id(), Reference(font_dict_id));
    }
    let global_font_dict_id = doc.add_object(global_font_dict);

    // Build XObject dictionary
    let mut global_xobject_dict = LoDictionary::new();
    for (k, v) in pdf.resources.xobjects.map.iter() {
        global_xobject_dict.set(
            k.0.clone(),
            crate::xobject::add_xobject_to_document(v, &mut doc),
        );
    }
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
            // gather page annotations
            let mut page_resources = LoDictionary::new(); // get_page_resources(&mut doc, &page);

            // gather page layers
            let page_layers = page
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::BeginLayer { layer_id } => pdf
                        .resources
                        .layers
                        .map
                        .get(layer_id)
                        .map(|q| (layer_id, q)),
                    _ => None,
                })
                .map(|(layer_id, l)| {
                    let usage_dict = doc.add_object(LoDictionary::from_iter(vec![
                        ("Type", Name("OCG".into())),
                        (
                            "CreatorInfo",
                            Dictionary(LoDictionary::from_iter(vec![
                                ("Creator", LoString(l.creator.clone().into(), Literal)),
                                ("Subtype", Name(l.usage.to_string().into())),
                            ])),
                        ),
                    ]));

                    let intent = doc.add_object(Array(vec![
                        Name("View".into()),
                        Name(l.intent.to_string().into()),
                    ]));

                    let id = doc.add_object(LoDictionary::from_iter(vec![
                        ("Type", Name("OCG".into())),
                        ("Name", LoString(l.name.clone().into(), Literal)),
                        ("Intent", Reference(intent)),
                        ("Usage", Reference(usage_dict)),
                    ]));

                    (layer_id.0.clone(), id)
                })
                .collect::<Vec<_>>();

            if !page_layers.is_empty() {
                page_resources.set(
                    "Properties",
                    LoDictionary::from_iter(
                        page_layers
                            .iter()
                            .map(|(id, obj)| (id.as_str(), Reference(*obj))),
                    ),
                );
            }

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
            // page_resources.et("Properties", Dictionary(ocg_dict));

            let layer_stream =
                translate_operations(&page.ops, &prepared_fonts, &pdf.resources.xobjects.map); // Vec<u8>
            let merged_layer_stream =
                LoStream::new(LoDictionary::new(), layer_stream).with_compression(false);

            let page_obj = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("Rotate", Integer(0)),
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
                bookmark_ids.get(i - 1).map(|s| s.3.clone())
            };
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
                Array(
                    page_ids
                        .iter()
                        .map(|q| Reference(q.clone()))
                        .collect::<Vec<_>>(),
                ),
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

fn translate_operations(
    ops: &[Op],
    fonts: &BTreeMap<FontId, PreparedFont>,
    xobjects: &BTreeMap<XObjectId, XObject>,
) -> Vec<u8> {
    let mut content = Vec::new();

    for op in ops {
        match op {
            Op::Marker { id } => {
                content.push(LoOp::new("MP", vec![Name(id.clone().into())]));
            }
            Op::BeginLayer { layer_id } => {
                content.push(LoOp::new("q", vec![]));
                content.push(LoOp::new(
                    "BDC",
                    vec![Name("OC".into()), Name(layer_id.0.clone().into())],
                ));
            }
            Op::EndLayer { layer_id } => {
                content.push(LoOp::new("EMC", vec![]));
                content.push(LoOp::new("Q", vec![]));
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
            Op::WriteText { text, font, size } => {
                if let Some(prepared_font) = fonts.get(&font) {
                    content.push(LoOp::new(
                        "Tf",
                        vec![font.0.clone().into(), (size.0).into()],
                    ));

                    let glyph_ids = text
                        .chars()
                        .filter_map(|s| prepared_font.original.lookup_glyph_index(s as u32))
                        .collect::<Vec<_>>();

                    let bytes = glyph_ids
                        .iter()
                        .flat_map(|x| vec![(x >> 8) as u8, (x & 255) as u8])
                        .collect::<Vec<u8>>();

                    content.push(LoOp::new("Tj", vec![LoString(bytes, Hexadecimal)]));
                }
            }
            Op::WriteTextBuiltinFont { text, font, size } => {
                content.push(LoOp::new(
                    "Tf",
                    vec![font.get_pdf_id().into(), (size.0).into()],
                ));
                let bytes = lopdf::Document::encode_text(Some("WinAnsiEncoding"), &text);
                content.push(LoOp::new("Tj", vec![LoString(bytes, Hexadecimal)]));
            }
            Op::WriteCodepoints { font, cp, size } => {
                if let Some(prepared_font) = fonts.get(&font) {
                    content.push(LoOp::new(
                        "Tf",
                        vec![font.0.clone().into(), (size.0).into()],
                    ));

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
            Op::WriteCodepointsWithKerning { font, cpk, size } => {
                if let Some(font) = fonts.get(&font) {
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
            Op::AddLineBreak => {
                content.push(LoOp::new("T*", vec![]));
            }
            Op::SetLineHeight { lh } => {
                content.push(LoOp::new("TL", vec![Real(lh.0)]));
            }
            Op::SetWordSpacing { percent } => {
                content.push(LoOp::new("Tw", vec![Real(*percent)]));
            }
            Op::SetFontSize { size, font } => {
                content.push(LoOp::new(
                    "Tf",
                    vec![font.0.clone().into(), (size.0).into()],
                ));
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
                let cvec = col.into_vec().into_iter().map(Real).collect();
                content.push(LoOp::new(ci, cvec));
            }
            Op::SetOutlineColor { col } => {
                let ci = match &col {
                    Color::Rgb(_) => "RG",
                    Color::Cmyk(_) | Color::SpotColor(_) => "K",
                    Color::Greyscale(_) => "G",
                };
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
            Op::UseXObject { id, transform } => {
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
            Op::Unknown { key, value } => {
                content.push(LoOp::new(key.as_str(), value.clone()));
            }
        }
    }

    lopdf::content::Content {
        operations: content,
    }
    .encode()
    .unwrap_or_default()
}

struct PreparedFont {
    original: ParsedFont,
    subset_font: SubsetFont,
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
    /// Cubic bezier over four following points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// Cubic bezier with two points in v1
    pub const OP_PATH_CONST_3BEZIER_V1: &str = "v";
    /// Cubic bezier with two points in v2
    pub const OP_PATH_CONST_3BEZIER_V2: &str = "y";
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to the two following points
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Stroke path
    pub const OP_PATH_PAINT_STROKE: &str = "S";
    /// Close and stroke path
    pub const OP_PATH_PAINT_STROKE_CLOSE: &str = "s";

    let mut operations = Vec::new();

    if line.points.is_empty() {
        return operations;
    };

    operations.push(LoOp::new(
        OP_PATH_CONST_MOVE_TO,
        vec![line.points[0].0.x.into(), line.points[0].0.y.into()],
    ));

    // Skip first element
    let mut current = 1;
    let max_len = line.points.len();

    // Loop over every points, determine if v, y, c or l operation should be used and build
    // curve / line accordingly
    while current < max_len {
        let p1 = &line.points[current - 1]; // prev pt
        let p2 = &line.points[current]; // current pt

        if p1.1 && p2.1 {
            // current point is a bezier handle
            // valid bezier curve must have two sequential bezier handles
            // we also can"t build a valid cubic bezier curve if the cuve contains less than
            // four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
            if let Some(p3) = line.points.get(current + 1) {
                if let Some(p4) = line.points.get(current + 2) {
                    if p1.0 == p2.0 {
                        // first control point coincides with initial point of curve
                        operations.push(LoOp::new(
                            OP_PATH_CONST_3BEZIER_V1,
                            vec![p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                        ));
                    } else if p2.0 == p3.0 {
                        // first control point coincides with final point of curve
                        operations.push(LoOp::new(
                            OP_PATH_CONST_3BEZIER_V2,
                            vec![p2.0.x.into(), p2.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                        ));
                    } else {
                        // regular bezier curve with four points
                        operations.push(LoOp::new(
                            OP_PATH_CONST_4BEZIER,
                            vec![
                                p2.0.x.into(),
                                p2.0.y.into(),
                                p3.0.x.into(),
                                p3.0.y.into(),
                                p4.0.x.into(),
                                p4.0.y.into(),
                            ],
                        ));
                    }
                    current += 3;
                    continue;
                }
            }
        }

        // normal straight line
        operations.push(LoOp::new(
            OP_PATH_CONST_LINE_TO,
            vec![p2.0.x.into(), p2.0.y.into()],
        ));
        current += 1;
    }

    // not filled, not closed but only stroked (regular path)
    if line.is_closed {
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]));
    } else {
        operations.push(LoOp::new(OP_PATH_PAINT_STROKE, vec![]));
    }

    operations
}

fn polygon_to_stream_ops(poly: &Polygon) -> Vec<LoOp> {
    /// Cubic bezier over four following points
    pub const OP_PATH_CONST_4BEZIER: &str = "c";
    /// Cubic bezier with two points in v1
    pub const OP_PATH_CONST_3BEZIER_V1: &str = "v";
    /// Cubic bezier with two points in v2
    pub const OP_PATH_CONST_3BEZIER_V2: &str = "y";
    /// Move to point
    pub const OP_PATH_CONST_MOVE_TO: &str = "m";
    /// Straight line to the two following points
    pub const OP_PATH_CONST_LINE_TO: &str = "l";
    /// Close and stroke path
    pub const OP_PATH_PAINT_STROKE_CLOSE: &str = "s";
    /// End path without filling or stroking
    pub const OP_PATH_PAINT_END: &str = "n";

    let mut operations = Vec::new();

    if poly.rings.is_empty() {
        return operations;
    };

    for ring in poly.rings.iter() {
        operations.push(LoOp::new(
            OP_PATH_CONST_MOVE_TO,
            vec![ring[0].0.x.into(), ring[0].0.y.into()],
        ));

        // Skip first element
        let mut current = 1;
        let max_len = ring.len();

        // Loop over every points, determine if v, y, c or l operation should be used and build
        // curve / line accordingly
        while current < max_len {
            let p1 = &ring[current - 1]; // prev pt
            let p2 = &ring[current]; // current pt

            if p1.1 && p2.1 {
                // current point is a bezier handle
                // valid bezier curve must have two sequential bezier handles
                // we also can"t build a valid cubic bezier curve if the cuve contains less than
                // four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
                if let Some(p3) = ring.get(current + 1) {
                    if let Some(p4) = ring.get(current + 2) {
                        if p1.0 == p2.0 {
                            // first control point coincides with initial point of curve
                            operations.push(LoOp::new(
                                OP_PATH_CONST_3BEZIER_V1,
                                vec![p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                            ));
                        } else if p2.0 == p3.0 {
                            // first control point coincides with final point of curve
                            operations.push(LoOp::new(
                                OP_PATH_CONST_3BEZIER_V2,
                                vec![p2.0.x.into(), p2.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                            ));
                        } else {
                            // regular bezier curve with four points
                            operations.push(LoOp::new(
                                OP_PATH_CONST_4BEZIER,
                                vec![
                                    p2.0.x.into(),
                                    p2.0.y.into(),
                                    p3.0.x.into(),
                                    p3.0.y.into(),
                                    p4.0.x.into(),
                                    p4.0.y.into(),
                                ],
                            ));
                        }
                        current += 3;
                        continue;
                    }
                }
            }

            // normal straight line
            operations.push(LoOp::new(
                OP_PATH_CONST_LINE_TO,
                vec![p2.0.x.into(), p2.0.y.into()],
            ));
            current += 1;
        }
    }

    match poly.mode {
        PaintMode::Clip => {
            // set the path as a clipping path
            operations.push(LoOp::new(poly.winding_order.get_clip_op(), vec![]));
        }
        PaintMode::Fill => {
            // is not stroked, only filled
            // closed-ness doesn't matter in this case, an area is always closed
            operations.push(LoOp::new(poly.winding_order.get_fill_op(), vec![]));
        }
        PaintMode::Stroke => {
            // same as line with is_closed = true
            operations.push(LoOp::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]));
        }
        PaintMode::FillStroke => {
            operations.push(LoOp::new(
                poly.winding_order.get_fill_stroke_close_op(),
                vec![],
            ));
        }
    }

    if !operations.is_empty() {
        operations.push(LoOp::new(OP_PATH_PAINT_END, vec![]));
    }

    operations
}

fn prepare_fonts(resources: &PdfResources, pages: &[PdfPage]) -> BTreeMap<FontId, PreparedFont> {
    let mut fonts_in_pdf = BTreeMap::new();

    for (font_id, font) in resources.fonts.map.iter() {
        let glyph_ids = font.get_used_glyph_ids(font_id, pages);
        if glyph_ids.is_empty() {
            continue; // unused font
        }
        let subset_font = match font.subset(&glyph_ids) {
            Ok(o) => o,
            Err(e) => {
                println!("{e}");
                continue;
            }
        };
        let font = match ParsedFont::from_bytes(&subset_font.bytes, 0) {
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
                subset_font: subset_font,
                cid_to_unicode_map: cid_to_unicode,
                vertical_writing: !font.vmtx_data.is_empty(),
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
                    if vertical { "W2" } else { "W" }.into(),
                    Array(prepared.widths_list.clone()),
                ),
                (
                    if vertical { "DW2" } else { "DW" }.into(),
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
                                Integer(prepared.max_height),
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
        Actions::GoTo(destination) => {
            dict.set("D", destination_to_obj(destination, page_ids));
        }
        Actions::URI(uri) => {
            dict.set("URI", LoString(uri.clone().into_bytes(), Literal));
        }
    }
    dict
}

fn destination_to_obj(d: &Destination, page_ids: &[lopdf::ObjectId]) -> lopdf::Object {
    match d {
        Destination::XYZ {
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
        ColorArray::RGB(arr) => arr.to_vec(),
        ColorArray::CMYK(arr) => arr.to_vec(),
    }
}
