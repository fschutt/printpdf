//! deserialize.rs
//!
//! This module implements parsing of a PDF (using lopdf) and converting it into a
//! printpdf::PdfDocument. In particular, it decompresses the content streams and then
//! converts lopdf operations to printpdf Ops.

use std::collections::BTreeMap;

use lopdf::{
    Dictionary as LopdfDictionary, Document as LopdfDocument, Object as LopdfObject, Object,
    ObjectId, Stream as LopdfStream,
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    BuiltinFont, BuiltinOrExternalFontId, Color, DictItem, ExtendedGraphicsState, ExtendedGraphicsStateId, ExtendedGraphicsStateMap, FontId, LayerInternalId, Line, LineDashPattern, LinePoint, LinkAnnotation, Op, PageAnnotId, PageAnnotMap, PaintMode, ParsedFont, PdfDocument, PdfDocumentInfo, PdfFontMap, PdfLayerMap, PdfMetadata, PdfPage, PdfResources, Point, Polygon, PolygonRing, Pt, RawImage, Rect, RenderingIntent, TextMatrix, TextRenderingMode, WindingOrder, XObject, XObjectId, XObjectMap, cmap::ToUnicodeCMap, conformance::PdfConformance, date::{OffsetDateTime, parse_pdf_date}
};

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfParseOptions {
    #[serde(default)]
    pub fail_on_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfWarnMsg {
    pub page: usize,
    pub op_id: usize,
    pub severity: PdfParseErrorSeverity,
    pub msg: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfParseErrorSeverity {
    Error,
    Info,
    Warning,
}

impl PdfWarnMsg {
    pub fn error(page: usize, op_id: usize, e: String) -> Self {
        PdfWarnMsg {
            page,
            op_id,
            severity: PdfParseErrorSeverity::Error,
            msg: e,
        }
    }

    pub fn warning(page: usize, op_id: usize, msg: String) -> Self {
        PdfWarnMsg {
            page,
            op_id,
            severity: PdfParseErrorSeverity::Warning,
            msg,
        }
    }

    pub fn info(page: usize, op_id: usize, msg: String) -> Self {
        PdfWarnMsg {
            page,
            op_id,
            severity: PdfParseErrorSeverity::Info,
            msg,
        }
    }
}

struct InitialPdf {
    doc: lopdf::Document,
    objs_to_search_for_resources: Vec<(Object, Option<usize>)>,
    page_refs: Vec<(u32, u16)>,
    document_info: PdfDocumentInfo,
    #[allow(dead_code)]
    link_annots: Vec<LinkAnnotation>,
}

fn parse_pdf_from_bytes_start(
    bytes: &[u8],
    _opts: &PdfParseOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<InitialPdf, String> {
    // Load the PDF document using lopdf.
    let doc = LopdfDocument::load_mem(bytes).map_err(|e| format!("Failed to load PDF: {}", e))?;

    let mut objs_to_search_for_resources = Vec::new();

    // Get the catalog from the trailer.
    let root_obj = doc
        .trailer
        .get(b"Root")
        .map_err(|_| "Missing Root in trailer".to_string())?;

    objs_to_search_for_resources.push((root_obj.clone(), None));

    let root_ref = match root_obj {
        Object::Reference(r) => *r,
        _ => return Err("Invalid Root reference".to_string()),
    };

    let catalog_obj = doc
        .get_object(root_ref)
        .map_err(|e| format!("Failed to get catalog: {}", e))?;

    objs_to_search_for_resources.push((catalog_obj.clone(), None));

    let catalog = catalog_obj
        .as_dict()
        .map_err(|e| format!("Catalog is not a dictionary: {}", e))?;

    // Check if catalog has a Resources dictionary and include it
    if let Ok(resources) = catalog.get(b"Resources") {
        objs_to_search_for_resources.push((resources.clone(), None));
    }

    // Get the Pages tree from the catalog.
    let pages_obj = catalog
        .get(b"Pages")
        .map_err(|e| format!("Missing Pages key in catalog: {}", e))?;

    objs_to_search_for_resources.push((pages_obj.clone(), None));

    let pages_ref = match pages_obj {
        Object::Reference(r) => *r,
        _ => return Err("Pages key is not a reference".to_string()),
    };

    let pages_dict = doc
        .get_object(pages_ref)
        .map_err(|e| format!("Failed to get Pages object: {}", e))?
        .as_dict()
        .map_err(|e| format!("Pages object is not a dictionary: {}", e))?;

    // Check if Pages tree has Resources dictionary and include it
    if let Ok(resources) = pages_dict.get(b"Resources") {
        objs_to_search_for_resources.push((resources.clone(), None));
    }

    // Recursively collect all page object references.
    let page_refs = collect_page_refs(pages_dict, &doc)?;

    // Get page objects and their resources
    let page_objects = page_refs
        .iter()
        .enumerate()
        .filter_map(|(i, &page_ref)| {
            doc.get_object(page_ref)
                .ok()
                .map(|obj| (obj.clone(), Some(i)))
        })
        .collect::<Vec<_>>();

    objs_to_search_for_resources.extend(page_objects);

    let document_info = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|s| get_dict_or_resolve_ref(&format!("document_info"), &doc, s, warnings, None))
        .map(|s| parse_document_info(s))
        .unwrap_or_default();

    let link_annots = links_and_bookmarks::extract_link_annotations(&doc);

    Ok(InitialPdf {
        doc,
        objs_to_search_for_resources,
        page_refs,
        document_info,
        link_annots,
    })
}

/// Parses a PDF file from bytes into a printpdf PdfDocument.
pub fn parse_pdf_from_bytes(
    bytes: &[u8],
    opts: &PdfParseOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfDocument, String> {
    let i = parse_pdf_from_bytes_start(bytes, opts, warnings)?;

    let fonts = parse_fonts_from_entire_pdf(&i.doc, &i.objs_to_search_for_resources, warnings);
    let xobjects =
        parse_xobjects_from_entire_pdf(&i.doc, &i.objs_to_search_for_resources, warnings);
    let xobjects = process_xobjects(xobjects, warnings);

    parse_pdf_from_bytes_end(i, opts, fonts, xobjects, warnings)
}

pub async fn parse_pdf_from_bytes_async(
    bytes: &[u8],
    opts: &PdfParseOptions,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfDocument, String> {
    let i = parse_pdf_from_bytes_start(bytes, opts, warnings)?;

    let fonts = parse_fonts_from_entire_pdf(&i.doc, &i.objs_to_search_for_resources, warnings);
    let xobjects =
        parse_xobjects_from_entire_pdf(&i.doc, &i.objs_to_search_for_resources, warnings);
    let xobjects = process_xobjects_async(xobjects, warnings).await;

    parse_pdf_from_bytes_end(i, opts, fonts, xobjects, warnings)
}

fn parse_pdf_from_bytes_end(
    initial_pdf: InitialPdf,
    _opts: &PdfParseOptions,
    fonts: BTreeMap<BuiltinOrExternalFontId, ParsedOrBuiltinFont>,
    xobjects: BTreeMap<XObjectId, XObject>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfDocument, String> {
    let mut pages = Vec::new();

    let InitialPdf {
        doc,
        objs_to_search_for_resources,
        page_refs,
        document_info,
        link_annots: _, // TODO
    } = initial_pdf;

    for (i, page_ref) in page_refs.into_iter().enumerate() {
        let page_obj = doc
            .get_object(page_ref)
            .map_err(|e| format!("Failed to get page object: {}", e))?
            .as_dict()
            .map_err(|e| format!("Page object is not a dictionary: {}", e))?;

        let pdf_page = parse_page(i, page_obj, &doc, &xobjects, warnings)?;

        pages.push(pdf_page);
    }

    // Extract bookmarks and layers from the document
    let bookmarks = links_and_bookmarks::extract_bookmarks(&doc);
    let layers = layers::extract_layers(&doc);

    // Extract ExtGStates from resources
    let extgstates = extract_extgstates(&doc, &objs_to_search_for_resources, warnings);

    let fonts = fonts
        .into_iter()
        .filter_map(|(id, pf)| {
            let parsed_font = pf.as_parsed_font()?;
            let pdf_font = crate::font::PdfFont::new(parsed_font);
            Some((FontId(id.get_id().to_string()), pdf_font))
        })
        .collect();

    // Build the final PdfDocument.
    let pdf_doc = PdfDocument {
        metadata: PdfMetadata {
            info: document_info,
            xmp: None,
        },
        resources: PdfResources {
            fonts: PdfFontMap { map: fonts },
            xobjects: XObjectMap { map: xobjects },
            extgstates: ExtendedGraphicsStateMap { map: extgstates },
            layers: PdfLayerMap {
                map: layers
                    .into_iter()
                    .enumerate()
                    .map(|(i, layer)| (LayerInternalId(format!("layer_{}", i)), layer))
                    .collect(),
            },
        },
        bookmarks: PageAnnotMap {
            map: bookmarks
                .into_iter()
                .enumerate()
                .map(|(i, bookmark)| (PageAnnotId(format!("bookmark_{}", i)), bookmark))
                .collect(),
        },
        pages,
    };

    Ok(pdf_doc)
}

pub enum ParsedOrBuiltinFont {
    P(ParsedFont, Option<ToUnicodeCMap>),
    B(BuiltinFont),
}

impl ParsedOrBuiltinFont {
    fn as_parsed_font(self) -> Option<ParsedFont> {
        match self {
            ParsedOrBuiltinFont::P(p, _) => Some(p),
            ParsedOrBuiltinFont::B(_) => None,
        }
    }

    fn cmap(&self) -> Option<&ToUnicodeCMap> {
        match self {
            ParsedOrBuiltinFont::P(_, cmap) => cmap.as_ref(),
            ParsedOrBuiltinFont::B(_) => None,
        }
    }
}

// Returns the dictionary, or resolves the reference
fn get_dict_or_resolve_ref<'a>(
    id: &str,
    doc: &'a LopdfDocument,
    xobj_obj: &'a LopdfObject,
    warnings: &mut Vec<PdfWarnMsg>,
    page: Option<usize>,
) -> Option<&'a LopdfDictionary> {
    match xobj_obj {
        Object::Dictionary(dict) => Some(dict),
        Object::Reference(r) => match doc.get_dictionary(*r) {
            Ok(s) => Some(s),
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!("{id}: Invalid dictionary reference {r:?}: {e:?}"),
                ));
                return None;
            }
        },
        _ => {
            warnings.push(PdfWarnMsg::error(
                page.unwrap_or(0),
                0,
                format!("Unexpected type for XObject resource"),
            ));
            return None;
        }
    }
}

// Returns the dictionary, or resolves the reference
fn get_stream_or_resolve_ref<'a>(
    doc: &'a LopdfDocument,
    xobj_obj: &'a LopdfObject,
    warnings: &mut Vec<PdfWarnMsg>,
    page: Option<usize>,
) -> Option<&'a LopdfStream> {
    match xobj_obj {
        Object::Stream(dict) => Some(dict),
        Object::Reference(r) => match doc.get_object(*r) {
            Ok(Object::Stream(s)) => Some(s),
            Ok(_) => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!("Invalid stream reference {r:?}"),
                ));
                return None;
            }
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!("Invalid stream reference {r:?}: {e:?}"),
                ));
                return None;
            }
        },
        _ => {
            warnings.push(PdfWarnMsg::error(
                page.unwrap_or(0),
                0,
                format!("Unexpected type for XObject resource"),
            ));
            return None;
        }
    }
}

fn process_xobjects(
    xobjects: BTreeMap<XObjectId, Vec<u8>>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<XObjectId, XObject> {
    let mut map = BTreeMap::new();
    for (xobject_id, content) in xobjects {
        warnings.push(PdfWarnMsg::info(
            0,
            0,
            format!("process XObject {} ({} bytes)", xobject_id.0, content.len()),
        ));
        match RawImage::decode_from_bytes(&content, warnings) {
            Ok(o) => {
                map.insert(xobject_id, XObject::Image(o));
            }
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    0,
                    0,
                    format!(
                        "failed to decode XObject {} ({} bytes): {e}",
                        xobject_id.0,
                        content.len()
                    ),
                ));
            }
        }
    }
    map
}

// Extract all extended graphics states from the document using the extgstate module
fn extract_extgstates(
    doc: &LopdfDocument,
    objs_to_search_for_resources: &[(LopdfObject, Option<usize>)],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<ExtendedGraphicsStateId, ExtendedGraphicsState> {
    objs_to_search_for_resources
        .iter()
        .filter_map(|(obj, page_idx)| {
            let page_num = page_idx.unwrap_or(0);

            // Get dictionary from object
            let dict = match obj {
                LopdfObject::Dictionary(d) => Some(d),
                LopdfObject::Reference(r) => doc.get_dictionary(*r).ok(),
                _ => None,
            }?;

            // Look for Resources
            let resources = if let Ok(res) = dict.get(b"Resources") {
                get_dict_or_resolve_ref(
                    &format!("extract_extgstates Resources page {page_num}"),
                    doc,
                    res,
                    warnings,
                    Some(page_num),
                )
            } else {
                // If this is already a Resources dict
                Some(dict)
            }?;

            // Look for ExtGState in Resources
            let extgstate = resources.get(b"ExtGState").ok()?;
            let extgstate_dict = get_dict_or_resolve_ref(
                &format!("extract_extgstates ExtGState page {page_num}"),
                doc,
                extgstate,
                warnings,
                Some(page_num),
            )?;

            // Build map of ExtGStates
            let gs_entries = extgstate_dict
                .iter()
                .filter_map(|(key, value)| {
                    let gs_dict = get_dict_or_resolve_ref(
                        &format!("extract_extgstates GsDict page {page_num}"),
                        doc,
                        value,
                        warnings,
                        Some(page_num),
                    )?;
                    let gs_id = ExtendedGraphicsStateId(String::from_utf8_lossy(key).to_string());
                    let gs = extgstate::parse_extgstate(gs_dict);
                    Some((gs_id, gs))
                })
                .collect::<BTreeMap<_, _>>();

            Some(gs_entries)
        })
        .fold(BTreeMap::new(), |mut acc, extgstates| {
            acc.extend(extgstates);
            acc
        })
}

async fn process_xobjects_async(
    xobjects: BTreeMap<XObjectId, Vec<u8>>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<XObjectId, XObject> {
    let mut map = BTreeMap::new();
    for (xobject_id, content) in xobjects {
        if let Ok(o) = RawImage::decode_from_bytes_async(&content, warnings).await {
            map.insert(xobject_id, XObject::Image(o));
        }
    }
    map
}

/// Given a Resources dictionary from a page or the document,
/// parse all XObjects and return a mapping from XObjectId to XObject.
/// (In this example we only handle image XObjects.)
fn parse_xobjects_internal(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
    warnings: &mut Vec<PdfWarnMsg>,
    page: Option<usize>,
) -> BTreeMap<XObjectId, Vec<u8>> {
    let mut xobj_map = BTreeMap::new();

    let page_num = page.unwrap_or_default();
    let xobj_obj = match resources.get(b"XObject") {
        Ok(s) => s,
        Err(_) => return xobj_map,
    };

    // The XObject entry may be a dictionary or a reference.
    let xobj_dict = match get_dict_or_resolve_ref(
        &format!("parse_xobjects_internal 1 page {page_num}"),
        doc,
        xobj_obj,
        warnings,
        page,
    ) {
        Some(s) => s,
        None => return xobj_map,
    };

    // Iterate over each entry.
    for (key, value) in xobj_dict.iter() {
        // TODO: better parsing!
        let xobject_id = XObjectId(String::from_utf8_lossy(key).to_string());

        let xobj_entry = match get_dict_or_resolve_ref(
            &format!("parse_xobjects_internal 2 page {page_num}"),
            doc,
            value,
            warnings,
            page,
        ) {
            Some(s) => s,
            None => continue,
        };

        let subtype_bytes = match xobj_entry.get(b"Subtype") {
            Ok(Object::Name(o)) => o.as_slice(),
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!(
                        "parse-xobjects: missing subtype for xobject {}: {e}",
                        xobject_id.0
                    ),
                ));
                continue;
            }
            _ => continue,
        };

        let stream = match subtype_bytes {
            b"Image" => match get_stream_or_resolve_ref(doc, value, warnings, page) {
                Some(s) => s,
                None => continue,
            },
            o => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!(
                        "parse-xobjects: unknown xobject subtype: {}",
                        String::from_utf8_lossy(o)
                    ),
                ));
                continue;
            }
        };

        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());

        xobj_map.insert(xobject_id, content);
    }

    xobj_map
}

fn parse_fonts_from_entire_pdf(
    doc: &LopdfDocument,
    objs_to_search_for_resources: &[(LopdfObject, Option<usize>)],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<BuiltinOrExternalFontId, ParsedOrBuiltinFont> {
    let obj = objs_to_search_for_resources
        .iter()
        .filter_map(|(obj, page_idx)| {
            let page_num = page_idx.unwrap_or(0);
            let dict = match obj {
                LopdfObject::Dictionary(d) => Some(d),
                LopdfObject::Reference(r) => doc.get_dictionary(*r).ok(),
                _ => None,
            }?;
            let resources_obj = dict.get(b"Resources").ok()?;
            let resources_dict = get_dict_or_resolve_ref(
                &format!("parse_fonts_from_entire_pdf 1 page {page_num}"),
                doc,
                resources_obj,
                warnings,
                Some(page_num),
            )?;
            // Use the enhanced font parsing function
            Some(parsefont::parse_fonts_enhanced(
                doc,
                resources_dict,
                warnings,
                Some(page_num),
            ))
        })
        .fold(BTreeMap::new(), |mut acc, fonts| {
            fonts.into_iter().for_each(|(font_id, parsed_font)| {
                acc.entry(BuiltinOrExternalFontId::External(font_id))
                    .or_insert(parsed_font);
            });
            acc
        });

    let mut map = BuiltinFont::all_ids()
        .iter()
        .map(|b| {
            (
                BuiltinOrExternalFontId::Builtin(*b),
                ParsedOrBuiltinFont::B(*b),
            )
        })
        .collect::<BTreeMap<_, _>>();

    map.extend(obj);
    map
}

fn parse_xobjects_from_entire_pdf(
    doc: &LopdfDocument,
    objs_to_search_for_resources: &[(LopdfObject, Option<usize>)],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<XObjectId, Vec<u8>> {
    objs_to_search_for_resources
        .iter()
        .filter_map(|(obj, page_idx)| {
            let page_num = page_idx.unwrap_or(0);
            let dict = match obj {
                LopdfObject::Dictionary(d) => Some(d),
                LopdfObject::Reference(r) => doc.get_dictionary(*r).ok(),
                _ => None,
            }?;
            let resources_obj = dict.get(b"Resources").ok()?;
            let resources_dict = get_dict_or_resolve_ref(
                &format!("parse_xobjects_from_entire_pdf 1 page {page_num}"),
                doc,
                resources_obj,
                warnings,
                Some(page_num),
            )?;
            Some(parse_xobjects_internal(
                doc,
                resources_dict,
                warnings,
                Some(page_num),
            ))
        })
        .fold(BTreeMap::new(), |mut acc, xobjs| {
            xobjs.into_iter().for_each(|(xobj_id, xobj)| {
                acc.entry(xobj_id).or_insert(xobj);
            });
            acc
        })
        .into_iter()
        .collect()
}

/// Recursively collects page object references from a Pages tree dictionary.
fn collect_page_refs(dict: &LopdfDictionary, doc: &LopdfDocument) -> Result<Vec<ObjectId>, String> {
    let mut pages = Vec::new();

    // The Pages tree must have a "Kids" array.
    let kids = dict
        .get(b"Kids")
        .map_err(|e| format!("Pages dictionary missing Kids key: {}", e))?;

    let page_refs = kids
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(|k| k.as_reference().ok())
                .collect::<Vec<_>>()
        })
        .map_err(|_| "Pages.Kids is not an array".to_string())?;

    for r in page_refs {
        let kid_obj = doc
            .get_object(r)
            .map_err(|e| format!("Failed to get kid object: {}", e))?;

        if let Ok(kid_dict) = kid_obj.as_dict() {
            let kid_type = kid_dict
                .get(b"Type")
                .map_err(|e| format!("Kid missing Type: {}", e))?;
            match kid_type {
                Object::Name(ref t) if t == b"Page" => {
                    pages.push(r);
                }
                Object::Name(ref t) if t == b"Pages" => {
                    let mut child_pages = collect_page_refs(kid_dict, doc)?;
                    pages.append(&mut child_pages);
                }
                _ => return Err(format!("Unknown kid type: {:?}", kid_type)),
            }
        }
    }

    Ok(pages)
}

/// Parses a single page dictionary into a PdfPage.
fn parse_page(
    num: usize,
    page: &LopdfDictionary,
    doc: &LopdfDocument,
    xobjects: &BTreeMap<XObjectId, XObject>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfPage, String> {
    // Parse MediaBox (required). PDF defines it as an array of 4 numbers.
    let media_box_obj = page
        .get(b"MediaBox")
        .map_err(|e| format!("Page missing MediaBox: {}", e))?;
    let media_box = parse_rect(media_box_obj)?;
    // TrimBox and CropBox are optional; use MediaBox as default.
    let trim_box = if let Ok(obj) = page.get(b"TrimBox") {
        parse_rect(obj)?
    } else {
        media_box.clone()
    };
    let crop_box = if let Ok(obj) = page.get(b"CropBox") {
        parse_rect(obj)?
    } else {
        media_box.clone()
    };

    // Get the Contents entry (could be a reference, an array, or a stream)
    let contents_obj = page
        .get(b"Contents")
        .map_err(|e| format!("Page missing Contents: {}", e))?;

    let mut content_data = Vec::new();
    match contents_obj {
        Object::Reference(r) => {
            let stream = doc
                .get_object(*r)
                .map_err(|e| format!("Failed to get content stream: {}", e))?
                .as_stream()
                .map_err(|e| format!("Content object is not a stream: {}", e))?;
            let data = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            content_data.extend(data);
        }
        Object::Array(arr) => {
            for obj in arr {
                if let Object::Reference(r) = obj {
                    let stream = doc
                        .get_object(*r)
                        .map_err(|e| format!("Failed to get content stream: {}", e))?
                        .as_stream()
                        .map_err(|e| format!("Content object is not a stream: {}", e))?;
                    let data = stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    content_data.extend(data);
                } else {
                    return Err("Content array element is not a reference".to_string());
                }
            }
        }
        _ => {
            // Try to interpret it as a stream.
            let stream = contents_obj
                .as_stream()
                .map_err(|e| format!("Contents not a stream: {}", e))?;
            let data = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            content_data.extend(data);
        }
    }

    // Decode the content stream into a vector of lopdf operations.
    let content = lopdf::content::Content::decode(&content_data)
        .map_err(|e| format!("Failed to decode content stream: {}", e))?;
    let ops = content.operations;

    // Convert lopdf operations to printpdf Ops.
    let mut page_state = PageState::default();
    let mut printpdf_ops = Vec::new();
    for (op_id, op) in ops.iter().enumerate() {
        let parsed_op = parse_op(num, op_id, &op, &mut page_state, xobjects, warnings)?;
        printpdf_ops.extend(parsed_op.into_iter());
    }

    Ok(PdfPage {
        media_box,
        trim_box,
        crop_box,
        ops: printpdf_ops,
    })
}

/// Converts a single lopdf Operation to a printpdf Op.
/// We use a mutable TextState to keep track of the current font and size.
#[derive(Debug, Clone, Default)]
pub struct PageState {
    /// True if we are inside a `BT ... ET` text block
    pub in_text_mode: bool,

    /// Current font resource and size (only relevant if `in_text_mode` = true)
    pub current_font: Option<crate::BuiltinOrExternalFontId>,

    pub current_font_size: Option<crate::units::Pt>,

    /// Current font resource name (e.g., "F5") - tracked only to emit warnings if text appears without Tf
    pub current_font_resource: Option<String>,

    /// Current transformation matrix stack. Each entry is a 6-float array [a b c d e f].
    pub transform_stack: Vec<[f32; 6]>,

    /// Name of the current marked content, if any (set by BMC/BDC)
    pub current_marked_content: Vec<String>,

    pub path_builder: PathBuilder,

    pub last_emitted_font_size: Option<(FontId, Pt)>,

    pub rectangle_builder: RectangleBuilder,
}

/// Builder for constructing PDF paths
#[derive(Debug, Clone, Default)]
pub struct RectangleBuilder {
    rectangle: Option<Rect>,
}

impl RectangleBuilder {
    pub fn new(&mut self, x: Pt, y: Pt, width: Pt, height: Pt) {
        self.rectangle = Some(Rect::from_xywh(x, y, width, height));
    }

    pub fn set_winding_order(&mut self, winding_order: WindingOrder) {
        if let Some(ref mut rectangle) = self.rectangle {
            rectangle.winding_order = Some(winding_order);
        }
    }

    pub fn get_ops(&self) -> Vec<Op> {
        if self.rectangle.is_none() {
            vec![]
        } else {
            vec![Op::DrawRectangle { rectangle: self.rectangle.as_ref().unwrap().clone() }]
        }
    }

    pub fn clear(&mut self) {
        self.rectangle = None;
    }
}

/// Represents PDF path operations
#[derive(Debug, Clone, PartialEq)]
pub enum PathOperation {
    MoveTo(Point),
    LineTo(Point),
    CurveTo {
        control1: Point,
        control2: Point,
        endpoint: Point,
    },
    ClosePath,
}

/// PDF subpath representation
#[derive(Debug, Clone, PartialEq)]
pub struct PdfSubPath {
    operations: Vec<PathOperation>,
    is_closed: bool,
}

/// Complete path with painting information
#[derive(Debug, Clone, PartialEq)]
pub struct PdfPath {
    subpaths: Vec<PdfSubPath>,
    paint_mode: PaintMode,
    winding_order: WindingOrder,
}

/// Builder for constructing PDF paths
#[derive(Debug, Clone, Default)]
pub struct PathBuilder {
    current_subpath: Vec<PathOperation>,
    subpaths: Vec<Vec<PathOperation>>,
    current_point: Option<Point>,
    start_point: Option<Point>,
}

impl PathBuilder {
    pub fn move_to(&mut self, point: Point) {
        if !self.current_subpath.is_empty() {
            self.subpaths
                .push(std::mem::take(&mut self.current_subpath));
        }

        self.current_subpath.push(PathOperation::MoveTo(point));
        self.current_point = Some(point);
        self.start_point = Some(point);
    }

    pub fn line_to(&mut self, point: Point) {
        if self.current_point.is_none() {
            self.move_to(point);
            return;
        }

        self.current_subpath.push(PathOperation::LineTo(point));
        self.current_point = Some(point);
    }

    pub fn curve_to(&mut self, control1: Point, control2: Point, endpoint: Point) {
        if self.current_point.is_none() {
            self.move_to(control1);
            self.curve_to(control1, control2, endpoint);
            return;
        }

        self.current_subpath.push(PathOperation::CurveTo {
            control1,
            control2,
            endpoint,
        });
        self.current_point = Some(endpoint);
    }

    /// v-curve: current point is first control point
    pub fn v_curve_to(&mut self, control2: Point, endpoint: Point) {
        if let Some(current) = self.current_point {
            self.curve_to(current, control2, endpoint);
        } else {
            // If no current point, treat as regular curve_to
            self.move_to(control2);
            self.curve_to(control2, control2, endpoint);
        }
    }

    /// y-curve: endpoint is second control point
    pub fn y_curve_to(&mut self, control1: Point, endpoint: Point) {
        if self.current_point.is_none() {
            self.move_to(control1);
        }

        self.curve_to(control1, endpoint, endpoint);
    }

    pub fn close_path(&mut self) {
        if self.current_subpath.is_empty() {
            return;
        }

        self.current_subpath.push(PathOperation::ClosePath);

        // Connect back to start point
        if let (Some(start), Some(current)) = (self.start_point, self.current_point) {
            if current != start {
                self.current_point = Some(start);
            }
        }
    }

    pub fn clear(&mut self) {
        self.current_subpath.clear();
        self.subpaths.clear();
        self.current_point = None;
        self.start_point = None;
    }

    #[cfg(test)]
    pub fn finalize_subpath(&mut self) {
        if !self.current_subpath.is_empty() {
            self.subpaths
                .push(std::mem::take(&mut self.current_subpath));
            self.current_point = None;
            self.start_point = None;
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.current_subpath.is_empty() && self.subpaths.is_empty()
    }

    pub fn build(&self, paint_mode: PaintMode, winding_order: WindingOrder) -> PdfPath {
        let mut all_subpaths = self.subpaths.clone();
        if !self.current_subpath.is_empty() {
            all_subpaths.push(self.current_subpath.clone());
        }

        let subpaths = all_subpaths
            .into_iter()
            .map(|ops| {
                let is_closed = ops.iter().any(|op| matches!(op, PathOperation::ClosePath));
                PdfSubPath {
                    operations: ops,
                    is_closed,
                }
            })
            .collect();

        PdfPath {
            subpaths,
            paint_mode,
            winding_order,
        }
    }
}

/// Convert a PdfPath to a DrawPolygon or DrawLine operation
pub fn path_to_op(path: &PdfPath) -> Op {
    let mut all_rings = Vec::new();

    for subpath in &path.subpaths {
        let points = subpath_to_line_points(subpath);
        if !points.is_empty() {
            all_rings.push(PolygonRing { points });
        }
    }

    if path.subpaths.len() == 1
        && path.paint_mode == PaintMode::Stroke
        && !path.subpaths[0].is_closed
    {
        if let Some(ring) = all_rings.first() {
            return Op::DrawLine {
                line: Line {
                    points: ring.points.clone(),
                    is_closed: false,
                },
            };
        }
    }

    Op::DrawPolygon {
        polygon: Polygon {
            rings: all_rings,
            mode: path.paint_mode,
            winding_order: path.winding_order,
        },
    }
}

/// Parse a PDF content stream into path operations
#[cfg(test)]
fn parse_pdf_stream(stream: &str) -> Vec<(String, Vec<f32>)> {
    let mut operations = Vec::new();
    let mut current_operands = Vec::new();

    // Simple tokenizer for PDF path operations
    let mut tokens = stream.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        // Check if token is a number
        if let Ok(num) = token.parse::<f32>() {
            current_operands.push(num);
            continue;
        }

        // If not a number, it's an operator
        if token == "m"
            || token == "l"
            || token == "c"
            || token == "v"
            || token == "y"
            || token == "h"
            || token == "f"
            || token == "S"
            || token == "s"
            || token == "f*"
            || token == "B"
            || token == "B*"
            || token == "b"
            || token == "b*"
        {
            // Store the operation
            operations.push((token.to_string(), current_operands.clone()));
            current_operands = Vec::new();
        }
    }

    operations
}

/// Apply a parsed operation to the PathBuilder
#[cfg(test)]
fn apply_path_operation(builder: &mut PathBuilder, op: &(String, Vec<f32>)) {
    match op.0.as_str() {
        "m" => {
            if op.1.len() >= 2 {
                builder.move_to(Point {
                    x: Pt(op.1[0]),
                    y: Pt(op.1[1]),
                });
            }
        }
        "l" => {
            if op.1.len() >= 2 {
                builder.line_to(Point {
                    x: Pt(op.1[0]),
                    y: Pt(op.1[1]),
                });
            }
        }
        "c" => {
            if op.1.len() >= 6 {
                builder.curve_to(
                    Point {
                        x: Pt(op.1[0]),
                        y: Pt(op.1[1]),
                    },
                    Point {
                        x: Pt(op.1[2]),
                        y: Pt(op.1[3]),
                    },
                    Point {
                        x: Pt(op.1[4]),
                        y: Pt(op.1[5]),
                    },
                );
            }
        }
        "v" => {
            if op.1.len() >= 4 {
                builder.v_curve_to(
                    Point {
                        x: Pt(op.1[0]),
                        y: Pt(op.1[1]),
                    },
                    Point {
                        x: Pt(op.1[2]),
                        y: Pt(op.1[3]),
                    },
                );
            }
        }
        "y" => {
            if op.1.len() >= 4 {
                builder.y_curve_to(
                    Point {
                        x: Pt(op.1[0]),
                        y: Pt(op.1[1]),
                    },
                    Point {
                        x: Pt(op.1[2]),
                        y: Pt(op.1[3]),
                    },
                );
            }
        }
        "h" => {
            builder.close_path();
        }
        "f" | "S" | "s" | "f*" | "B" | "B*" | "b" | "b*" => {
            // These are painting operators, not path construction
            // For the test, we handle them by finalizing the current subpath
            builder.finalize_subpath();
        }
        _ => {}
    }
}

/// Convert a PdfSubPath to a sequence of LinePoints with proper bezier flags
fn subpath_to_line_points(subpath: &PdfSubPath) -> Vec<LinePoint> {
    let mut points = Vec::new();

    for operation in &subpath.operations {
        match operation {
            PathOperation::MoveTo(point) => {
                points.push(LinePoint {
                    p: *point,
                    bezier: false,
                });
            }
            PathOperation::LineTo(point) => {
                points.push(LinePoint {
                    p: *point,
                    bezier: false,
                });
            }
            PathOperation::CurveTo {
                control1,
                control2,
                endpoint,
            } => {
                // First control point - always bezier=true
                points.push(LinePoint {
                    p: *control1,
                    bezier: true,
                });
                // Second control point - always bezier=true
                points.push(LinePoint {
                    p: *control2,
                    bezier: true,
                });
                // Endpoint - always bezier=false
                points.push(LinePoint {
                    p: *endpoint,
                    bezier: false,
                });
            }
            PathOperation::ClosePath => {
                // Nothing to add for close path
            }
        }
    }

    points
}

#[test]
fn test_2() {
    let pdf_stream = "
    20.5 344.5 m
    20.5 344.5 22 333.5 10.5 346.5 c
    S"
    .trim();

    let expected_output = Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point {
                        x: Pt(20.5),
                        y: Pt(344.5),
                    },
                    bezier: false,
                },
                LinePoint {
                    p: Point {
                        x: Pt(20.5),
                        y: Pt(344.5),
                    },
                    bezier: true,
                },
                LinePoint {
                    p: Point {
                        x: Pt(22.0),
                        y: Pt(333.5),
                    },
                    bezier: true,
                },
                LinePoint {
                    p: Point {
                        x: Pt(10.5),
                        y: Pt(346.5),
                    },
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    };

    // Parse using the PathBuilder
    let mut builder = PathBuilder::default();
    let ops = parse_pdf_stream(pdf_stream);
    for op in ops {
        apply_path_operation(&mut builder, &op);
    }

    // Build path and generate commands
    let path = builder.build(PaintMode::Stroke, WindingOrder::NonZero);
    let draw_op = path_to_op(&path);

    // Verify bezier flags are correctly set
    assert_eq!(draw_op, expected_output);
}

#[test]
fn test_bezier_path_parsing() {
    // Test case from tiger.svg
    let pdf_stream = "-122.3 84.285 m -122.3 84.285 -122.2 86.179 -123.03 86.16 c -123.85 86.141 \
                      -140.3 38.066 -160.83 40.309 c -160.83 40.309 -143.05 32.956 -122.3 84.285 \
                      c h f";

    // Parse using the PathBuilder
    let mut builder = PathBuilder::default();
    let ops = parse_pdf_stream(pdf_stream);
    for op in ops {
        apply_path_operation(&mut builder, &op);
    }

    // Build path and generate commands
    let path = builder.build(PaintMode::Fill, WindingOrder::NonZero);
    let draw_op = path_to_op(&path);

    // Verify bezier flags are correctly set
    if let Op::DrawPolygon { polygon } = draw_op {
        assert_eq!(polygon.rings.len(), 1);
        assert_eq!(polygon.rings[0].points.len(), 10);

        // Check flags for points
        let points = &polygon.rings[0].points;
        assert!(!points[0].bezier); // Start point
        assert!(points[1].bezier); // Control point 1
        assert!(points[2].bezier); // Control point 2
        assert!(!points[3].bezier); // End point
        assert!(points[4].bezier); // Next control point 1
                                   // And so on...
    } else {
        panic!("Expected DrawPolygon");
    }
}

/// Convert a single lopdf Operation into zero, one, or many `printpdf::Op`.
/// We maintain / mutate `PageState` so that repeated path operators (`m`, `l`, `c`, etc.)
/// accumulate subpaths, and we only emit path-based Ops at stroke or fill time.
pub fn parse_op(
    page: usize,
    op_id: usize,
    op: &lopdf::content::Operation,
    state: &mut PageState,
    _xobjects: &BTreeMap<XObjectId, XObject>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<Op>, String> {
    use crate::units::Pt;
    let mut out_ops = Vec::new();
    match op.operator.as_str() {
        // --- Graphics State Save/Restore ---
        "q" => {
            let top = state
                .transform_stack
                .last()
                .copied()
                .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            state.transform_stack.push(top);
            out_ops.push(Op::SaveGraphicsState);
        }
        "Q" => {
            if state.transform_stack.pop().is_none() {
                // we won't fail the parse, just warn
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'Q' with empty transform stack"),
                ));
            }
            out_ops.push(Op::RestoreGraphicsState);
        }
        "MP" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "MP expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }

            let id = match as_name(&op.operands[0]) {
                Some(name) => name,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "MP operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };

            out_ops.push(Op::Marker { id });
        }
        "CS" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "CS expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }

            let id = match as_name(&op.operands[0]) {
                Some(name) => name,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "CS operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };

            out_ops.push(Op::SetColorSpaceStroke { id });
        }
        "cs" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "cs expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }

            let id = match as_name(&op.operands[0]) {
                Some(name) => name,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "cs operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };

            out_ops.push(Op::SetColorSpaceFill { id });
        }
        "ri" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "ri expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }
            let intent = match as_name(&op.operands[0]) {
                Some(i) => i,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "ri operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };
            let intent = match intent.as_str() {
                "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
                "RelativeColorimetric" => RenderingIntent::RelativeColorimetric,
                "Saturation" => RenderingIntent::Saturation,
                "Perceptual" => RenderingIntent::Perceptual,
                other => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        format!(
                            "Unknown rendering intent '{}', defaulting to RelativeColorimetric",
                            other
                        ),
                    ));
                    RenderingIntent::RelativeColorimetric
                }
            };
            out_ops.push(Op::SetRenderingIntent { intent });
        }
        "rg" => {
            // 'rg' sets fill color in RGB.
            if op.operands.len() == 3 {
                let r = to_f32(&op.operands[0]);
                let g = to_f32(&op.operands[1]);
                let b = to_f32(&op.operands[2]);
                out_ops.push(Op::SetFillColor {
                    col: Color::Rgb(crate::Rgb {
                        r,
                        g,
                        b,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'rg' expects 3 operands".to_string(),
                ));
            }
        }
        "RG" => {
            // 'RG' sets stroke (outline) color in RGB.
            if op.operands.len() == 3 {
                let r = to_f32(&op.operands[0]);
                let g = to_f32(&op.operands[1]);
                let b = to_f32(&op.operands[2]);
                out_ops.push(Op::SetOutlineColor {
                    col: Color::Rgb(crate::Rgb {
                        r,
                        g,
                        b,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'RG' expects 3 operands".to_string(),
                ));
            }
        }
        "k" => {
            // 'k' sets fill color in Cmyk.
            if op.operands.len() == 4 {
                let c = to_f32(&op.operands[0]);
                let m = to_f32(&op.operands[1]);
                let y = to_f32(&op.operands[2]);
                let k = to_f32(&op.operands[3]);
                out_ops.push(Op::SetFillColor {
                    col: Color::Cmyk(crate::Cmyk {
                        c,
                        m,
                        y,
                        k,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'k' expects 4 operands".to_string(),
                ));
            }
        }
        "K" => {
            // 'K' sets stroke (outline) color in CMYK.
            if op.operands.len() == 4 {
                let c = to_f32(&op.operands[0]);
                let m = to_f32(&op.operands[1]);
                let y = to_f32(&op.operands[2]);
                let k = to_f32(&op.operands[3]);
                out_ops.push(Op::SetOutlineColor {
                    col: Color::Cmyk(crate::Cmyk {
                        c,
                        m,
                        y,
                        k,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'K' expects 4 operands".to_string(),
                ));
            }
        }
        "g" => {
            // 'g' sets the fill color in grayscale.
            if op.operands.len() == 1 {
                let gray = to_f32(&op.operands[0]);
                out_ops.push(Op::SetFillColor {
                    col: Color::Greyscale(crate::Greyscale {
                        percent: gray,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'g' expects 1 operand".to_string(),
                ));
            }
        }
        "G" => {
            // 'G' sets the stroke (outline) color in grayscale.
            if op.operands.len() == 1 {
                let gray = to_f32(&op.operands[0]);
                out_ops.push(Op::SetOutlineColor {
                    col: Color::Greyscale(crate::Greyscale {
                        percent: gray,
                        icc_profile: None,
                    }),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'G' expects 1 operand".to_string(),
                ));
            }
        }

        // --- Text showing with spacing ---
        "TJ" => {
            if !state.in_text_mode {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'TJ' outside of text mode!".to_string(),
                ));
            }

            if op.operands.is_empty() {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'TJ' with no operands".to_string(),
                ));
            } else if let Some(arr) = op.operands.get(0).and_then(|o| o.as_array().ok()) {
                // Warn if no font was set via Tf operator
                if state.current_font_resource.is_none() {
                    warnings.push(PdfWarnMsg::warning(
                        page,
                        op_id,
                        "Warning: 'TJ' without prior 'Tf' (font setup). Renderer will use default font.".to_string(),
                    ));
                }

                // Use raw glyph IDs instead of decoding to Unicode
                // This is more efficient and preserves the original PDF structure
                let text_items = crate::text::decode_tj_operands_as_glyph_ids(arr);

                // Emit new ShowText operation (1:1 PDF mapping)
                out_ops.push(Op::ShowText {
                    items: text_items,
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'TJ' operand is not an array".to_string(),
                ));
            }
        }

        "Tj" => {
            if !state.in_text_mode {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'Tj' outside of text mode!".to_string(),
                ));
            }

            if op.operands.is_empty() {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'Tj' with no operands".to_string(),
                ));
            } else if let lopdf::Object::String(bytes, _format) = &op.operands[0] {
                // Warn if no font was set via Tf operator
                if state.current_font_resource.is_none() {
                    warnings.push(PdfWarnMsg::warning(
                        page,
                        op_id,
                        "Warning: 'Tj' without prior 'Tf' (font setup). Renderer will use default font.".to_string(),
                    ));
                }

                // Use raw glyph IDs instead of decoding to Unicode
                let text_items = crate::text::decode_tj_string_as_glyph_ids(bytes);

                // Emit new ShowText operation (1:1 PDF mapping)
                out_ops.push(Op::ShowText {
                    items: text_items,
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'Tj' operand is not string".to_string(),
                ));
            }
        }
        "T*" => {
            out_ops.push(Op::AddLineBreak);
        }
        "TL" => {
            if op.operands.len() == 1 {
                let val = to_f32(&op.operands[0]);
                out_ops.push(Op::SetLineHeight { lh: Pt(val) });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'TL' expects 1 operand, got {}", op.operands.len()),
                ));
            }
        }
        "Ts" => {
            if op.operands.len() == 1 {
                let rise = to_f32(&op.operands[0]);
                out_ops.push(Op::SetLineOffset { multiplier: rise });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'Ts' expects 1 operand, got {}", op.operands.len()),
                ));
            }
        }
        "Tw" => {
            if op.operands.len() == 1 {
                let spacing = to_f32(&op.operands[0]);
                out_ops.push(Op::SetWordSpacing { pt: Pt(spacing) });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'Tw' expects 1 operand, got {}", op.operands.len()),
                ));
            }
        }
        "Tz" => {
            if op.operands.len() == 1 {
                let scale_percent = to_f32(&op.operands[0]);
                out_ops.push(Op::SetHorizontalScaling {
                    percent: scale_percent,
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'Tz' expects 1 operand, got {}", op.operands.len()),
                ));
            }
        }
        "TD" => {
            if op.operands.len() == 2 {
                let tx = to_f32(&op.operands[0]);
                let ty = to_f32(&op.operands[1]);
                out_ops.push(Op::MoveTextCursorAndSetLeading { tx, ty });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("'TD' expects 2 operands, got {}", op.operands.len()),
                ));
            }
        }
        "Tm" => {
            if op.operands.len() == 6 {
                let a = to_f32(&op.operands[0]);
                let b = to_f32(&op.operands[1]);
                let c = to_f32(&op.operands[2]);
                let d = to_f32(&op.operands[3]);
                let e = to_f32(&op.operands[4]);
                let f = to_f32(&op.operands[5]);
                // Optionally update a field in PageState (e.g. state.current_text_matrix)
                // if you want to track the current text transformation.
                out_ops.push(Op::SetTextMatrix {
                    matrix: TextMatrix::Raw([a, b, c, d, e, f]),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: 'Tm' expects 6 operands".to_string(),
                ));
            }
        }
        "Tc" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Tc expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }
            let spacing = to_f32(&op.operands[0]);
            out_ops.push(Op::SetCharacterSpacing {
                multiplier: spacing,
            });
        }
        "Tr" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Tr expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }
            let mode = match &op.operands[0] {
                Object::Integer(i) => *i as i64,
                Object::Real(r) => *r as i64,
                _ => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "Tr operand is not numeric".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };
            out_ops.push(Op::SetTextRenderingMode {
                mode: TextRenderingMode::from_i64(mode),
            });
        }
        "gs" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "gs expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }

            let gs_name = match as_name(&op.operands[0]) {
                Some(name) => name,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "gs operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };

            out_ops.push(Op::LoadGraphicsState {
                gs: ExtendedGraphicsStateId(gs_name),
            });
        }

        "BT" => {
            // --- Text mode begin
            state.in_text_mode = true;
            out_ops.push(Op::StartTextSection);
        }
        "ET" => {
            // --- Text mode end
            state.in_text_mode = false;
            out_ops.push(Op::EndTextSection);
        }

        "Tf" => {
            // --- Font + size (Tf) ---
            if op.operands.len() == 2 {
                if let Some(font_name) = as_name(&op.operands[0]) {
                    state.current_font = Some(BuiltinOrExternalFontId::from_str(&font_name));
                    state.current_font_resource = Some(font_name.clone());
                }
                let size_val = to_f32(&op.operands[1]);
                state.current_font_size = Some(crate::units::Pt(size_val));

                // Emit new SetFont operation (1:1 PDF mapping)
                if let (Some(font_resource), Some(sz)) = (&state.current_font_resource, &state.current_font_size) {
                    // Parse font_resource to determine if it's builtin or external
                    let font_handle = if let Some(builtin) = BuiltinFont::from_id(font_resource) {
                        crate::ops::PdfFontHandle::Builtin(builtin)
                    } else {
                        crate::ops::PdfFontHandle::External(crate::FontId(font_resource.clone()))
                    };
                    
                    out_ops.push(Op::SetFont {
                        font: font_handle,
                        size: *sz,
                    });
                }
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!(
                        "Warning: 'Tf' expects 2 operands, got {}",
                        op.operands.len()
                    ),
                ));
                out_ops.push(Op::Unknown {
                    key: "Tf".into(),
                    value: op
                        .operands
                        .iter()
                        .map(|s| DictItem::from_lopdf(s))
                        .collect(),
                });
            }
        }

        // --- Move text cursor (Td) ---
        "Td" => {
            if op.operands.len() == 2 {
                let tx = to_f32(&op.operands[0]);
                let ty = to_f32(&op.operands[1]);
                out_ops.push(Op::SetTextCursor {
                    pos: crate::graphics::Point {
                        x: crate::units::Pt(tx),
                        y: crate::units::Pt(ty),
                    },
                });
            }
        }

        // --- Begin/End marked content or optional content (BMC,BDC/EMC) ---
        // NOTE: optional content can be nested
        "BDC" => {
            // Typically something like: [Name("Span"), Properties as a Dictionary]
            // In case of optional content: [Name("OC"), Name("MyLayer")]
            if op.operands.len() != 2 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "BDC expects 2 operands".to_string(),
                ));
                return Ok(Vec::new());
            }
            let Some(marked_content_nm) = as_name(&op.operands[0]) else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "BDC operand is not a name".to_string(),
                ));
                return Ok(Vec::new());
            };
            if marked_content_nm.as_str() == "OC" {
                let Some(layer_nm) = as_name(&op.operands[1]) else {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "BDC OC layer operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                };
                out_ops.push(Op::BeginOptionalContent {
                    layer_id: crate::LayerInternalId(layer_nm),
                });
            } else {
                match op.operands[1].as_dict() {
                    Ok(dict) => {
                        out_ops.push(Op::BeginMarkedContentWithProperties {
                            tag: marked_content_nm.clone(),
                            properties: DictItem::from_lopdf(&Object::Dictionary(dict.to_owned())),
                        });
                    },
                    Err(_err) => {
                        warnings.push(PdfWarnMsg::warning(
                            page,
                            op_id,
                            "BDC properties is not a dictionary".to_string(),
                        ));
                        out_ops.push(Op::Unknown {
                            key: "BDC".into(),
                            value: op
                                .operands
                                .iter()
                                .map(|s| DictItem::from_lopdf(s))
                                .collect(),
                        });
                    }
                }
            }
            state.current_marked_content.push(marked_content_nm.clone());
        }
        "BMC" => {
            if op.operands.len() != 1 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "BMC expects 1 operand".to_string(),
                ));
                return Ok(Vec::new());
            }
            let marked_content_nm = match as_name(&op.operands[0]) {
                Some(t) => t,
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "BMC operand is not a name".to_string(),
                    ));
                    return Ok(Vec::new());
                }
            };
            state.current_marked_content.push(marked_content_nm.clone());
            out_ops.push(Op::BeginMarkedContent {
                tag: marked_content_nm.clone(),
            });
        }
        "EMC" => {
            if let Some(_marked_content_str) = state.current_marked_content.pop() {
                out_ops.push(Op::EndMarkedContent { });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'EMC' with no current_marked_content"),
                ));
            }
        }

        // --- Transformation (cm) ---
        "cm" => {
            if op.operands.len() == 6 {
                let floats: Vec<f32> = op.operands.iter().map(to_f32).collect();
                if let Some(top) = state.transform_stack.last_mut() {
                    // multiply top by these floats
                    let combined = crate::matrix::CurTransMat::combine_matrix(
                        *top,
                        floats.as_slice().try_into().unwrap(),
                    );
                    *top = combined;
                }
                out_ops.push(Op::SetTransformationMatrix {
                    matrix: crate::matrix::CurTransMat::Raw(floats.try_into().unwrap()),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'cm' expects 6 floats"),
                ));
            }
        }

        // --- Path building: moveTo (m), lineTo (l), closepath (h), curveTo (c), etc. ---
        // --- Path building: moveTo (m), lineTo (l), closepath (h), curveTo (c), etc. ---
        "m" => {
            // Start a new subpath
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.path_builder.move_to(Point { x: Pt(x), y: Pt(y) });
        }
        "l" => {
            // lineTo
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.path_builder.line_to(Point { x: Pt(x), y: Pt(y) });
        }
        "c" => {
            // c x1 y1 x2 y2 x3 y3
            if op.operands.len() == 6 {
                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x2 = to_f32(&op.operands[2]);
                let y2 = to_f32(&op.operands[3]);
                let x3 = to_f32(&op.operands[4]);
                let y3 = to_f32(&op.operands[5]);

                state.path_builder.curve_to(
                    Point {
                        x: Pt(x1),
                        y: Pt(y1),
                    },
                    Point {
                        x: Pt(x2),
                        y: Pt(y2),
                    },
                    Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                );
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("'c' expects 6 operands, got {}", op.operands.len()),
                ));
            }
        }
        "v" => {
            // v x2 y2 x3 y3
            // The first control point is implied to be the current point
            if op.operands.len() == 4 {
                let x2 = to_f32(&op.operands[0]);
                let y2 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);

                state.path_builder.v_curve_to(
                    Point {
                        x: Pt(x2),
                        y: Pt(y2),
                    },
                    Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                );
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("'v' expects 4 operands, got {}", op.operands.len()),
                ));
            }
        }
        "y" => {
            // y x1 y1 x3 y3
            // The second control point is implied to be the final point
            if op.operands.len() == 4 {
                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);

                state.path_builder.y_curve_to(
                    Point {
                        x: Pt(x1),
                        y: Pt(y1),
                    },
                    Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                );
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("'y' expects 4 operands, got {}", op.operands.len()),
                ));
            }
        }
        "h" => {
            // closepath, i.e. connect last point to first point
            state.path_builder.close_path();
        }
        "re" => {
            if op.operands.len() == 4 {
                let x = Pt(to_f32(&op.operands[0]));
                let y = Pt(to_f32(&op.operands[1]));
                let width = Pt(to_f32(&op.operands[2]));
                let height = Pt(to_f32(&op.operands[3]));
                state.rectangle_builder.new(x, y, width, height);
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "re expects 4 operands".to_string(),
                ));
            }
        }
        // --- Path painting
        "S" => {
            // Stroke - doesn't close the path
            let path = state
                .path_builder
                .build(PaintMode::Stroke, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "f" => {
            // Fill with non-zero winding rule
            let path = state
                .path_builder
                .build(PaintMode::Fill, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "f*" => {
            // Fill with even-odd winding rule
            let path = state
                .path_builder
                .build(PaintMode::Fill, WindingOrder::EvenOdd);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "B" => {
            // Fill and stroke with non-zero winding
            let path = state
                .path_builder
                .build(PaintMode::FillStroke, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "B*" => {
            // Fill and stroke with even-odd winding
            let path = state
                .path_builder
                .build(PaintMode::FillStroke, WindingOrder::EvenOdd);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "b" => {
            // Close, fill, and stroke with non-zero winding
            state.path_builder.close_path();
            let path = state
                .path_builder
                .build(PaintMode::FillStroke, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "b*" => {
            // Close, fill, and stroke with even-odd winding
            state.path_builder.close_path();
            let path = state
                .path_builder
                .build(PaintMode::FillStroke, WindingOrder::EvenOdd);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "s" => {
            // Close and stroke
            state.path_builder.close_path();
            let path = state
                .path_builder
                .build(PaintMode::Stroke, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            state.path_builder.clear();
        }
        "n" => {
            // End path without filling or stroking
            state.path_builder.clear();
            out_ops.extend_from_slice(&state.rectangle_builder.get_ops());
            state.rectangle_builder.clear();
        }
        "W" => {
            // Set clip path using non-zero winding rule
            let path = state
                .path_builder
                .build(PaintMode::Clip, WindingOrder::NonZero);
            if !path.subpaths.is_empty() {
                out_ops.push(path_to_op(&path));
            }
            // Note: We don't clear the path here since clipping doesn't consume the path
            state.rectangle_builder.set_winding_order(WindingOrder::NonZero);
        }
        "W*" => {
            // Set clip path using even-odd winding rule
            let path = state
                .path_builder
                .build(PaintMode::Clip, WindingOrder::EvenOdd);
            if !path.subpaths.is_empty() || out_ops.last().is_some_and(|last_op| match last_op {
                Op::DrawRectangle { .. } => true,
                _ => false,
            }) {
                out_ops.push(path_to_op(&path));
            }
            // Note: We don't clear the path here since clipping doesn't consume the path
            state.rectangle_builder.set_winding_order(WindingOrder::EvenOdd);
        }

        // --- Painting state operators
        "w" => {
            // Set line width
            // "w" sets the line width (stroke thickness in user‐space units)
            // e.g. "3 w" => 3pt line width
            if let Some(val) = op.operands.get(0) {
                let width = to_f32(val);
                out_ops.push(Op::SetOutlineThickness {
                    pt: crate::units::Pt(width),
                });
            }
        }

        "M" => {
            // Set miter limit
            // PDF operator "M <limit>" sets the miter limit.
            // You don't currently have a specific Op variant for that,
            // so you can either create one or store as `Unknown`.
            if let Some(val) = op.operands.get(0) {
                let limit = to_f32(val);
                // If you create an Op::SetMiterLimit, do that here:
                out_ops.push(Op::SetMiterLimit { limit: Pt(limit) });
            }
        }

        "j" => {
            // Set line join style
            // "0 j" => miter join, "1 j" => round join, "2 j" => bevel
            if let Some(val) = op.operands.get(0) {
                let style_num = to_f32(val).round() as i64;
                let style = match style_num {
                    0 => crate::graphics::LineJoinStyle::Miter,
                    1 => crate::graphics::LineJoinStyle::Round,
                    2 => crate::graphics::LineJoinStyle::Bevel,
                    _ => crate::graphics::LineJoinStyle::Miter, // fallback
                };
                out_ops.push(Op::SetLineJoinStyle { join: style });
            }
        }

        "J" => {
            // Set line cap style
            // "0 J" => butt cap, "1 J" => round cap, "2 J" => projecting square
            if let Some(val) = op.operands.get(0) {
                let style_num = to_f32(val).round() as i64;
                let style = match style_num {
                    0 => crate::graphics::LineCapStyle::Butt,
                    1 => crate::graphics::LineCapStyle::Round,
                    2 => crate::graphics::LineCapStyle::ProjectingSquare,
                    _ => crate::graphics::LineCapStyle::Butt, // fallback
                };
                out_ops.push(Op::SetLineCapStyle { cap: style });
            }
        }

        "d" => {
            // Set dash pattern
            // "d [2 2] 0" => dash 2 on, 2 off, offset=0
            if op.operands.len() == 2 {
                // operand 0 is the dash array, operand 1 is the dash offset
                if let Some(arr_obj) = op.operands.get(0) {
                    // parse array of numbers
                    if let Ok(arr) = arr_obj.as_array() {
                        let pattern: Vec<i64> =
                            arr.iter().map(|item| to_f32(item) as i64).collect();
                        let offset = to_f32(&op.operands[1]) as i64;
                        let dash = LineDashPattern::from_array(&pattern, offset);
                        out_ops.push(Op::SetLineDashPattern { dash });
                    }
                }
            }
        }

        // Fill color: "sc" or "scn"
        // Typically you see "1 1 1 sc" => white fill in DeviceRGB
        "sc" | "scn" => {
            // We interpret the number of operands to guess the color space.
            // e.g. 1 operand => grayscale, 3 => RGB, 4 => CMYK
            let floats = op.operands.iter().map(to_f32).collect::<Vec<_>>();
            match floats.len() {
                1 => {
                    // grayscale
                    out_ops.push(Op::SetFillColor {
                        col: Color::Greyscale(crate::Greyscale {
                            percent: floats[0],
                            icc_profile: None,
                        }),
                    });
                }
                3 => {
                    // rgb
                    out_ops.push(Op::SetFillColor {
                        col: Color::Rgb(crate::Rgb {
                            r: floats[0],
                            g: floats[1],
                            b: floats[2],
                            icc_profile: None,
                        }),
                    });
                }
                4 => {
                    // cmyk
                    out_ops.push(Op::SetFillColor {
                        col: Color::Cmyk(crate::Cmyk {
                            c: floats[0],
                            m: floats[1],
                            y: floats[2],
                            k: floats[3],
                            icc_profile: None,
                        }),
                    });
                }
                _ => {
                    // fallback
                    out_ops.push(Op::Unknown {
                        key: op.operator.clone(),
                        value: op
                            .operands
                            .iter()
                            .map(|s| DictItem::from_lopdf(s))
                            .collect(),
                    });
                }
            }
        }

        // Stroke color: "SC" or "SCN"
        // e.g. "1 0 0 SC" => red stroke
        "SC" | "SCN" => {
            let floats = op.operands.iter().map(to_f32).collect::<Vec<_>>();
            match floats.len() {
                1 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Greyscale(crate::Greyscale {
                            percent: floats[0],
                            icc_profile: None,
                        }),
                    });
                }
                3 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Rgb(crate::Rgb {
                            r: floats[0],
                            g: floats[1],
                            b: floats[2],
                            icc_profile: None,
                        }),
                    });
                }
                4 => {
                    out_ops.push(Op::SetOutlineColor {
                        col: Color::Cmyk(crate::Cmyk {
                            c: floats[0],
                            m: floats[1],
                            y: floats[2],
                            k: floats[3],
                            icc_profile: None,
                        }),
                    });
                }
                _ => {
                    out_ops.push(Op::Unknown {
                        key: op.operator.clone(),
                        value: op
                            .operands
                            .iter()
                            .map(|s| DictItem::from_lopdf(s))
                            .collect(),
                    });
                }
            }
        }

        // --- XObjects: /Do ---
        "Do" => {
            if let Some(name_str) = as_name(&op.operands.get(0).unwrap_or(&lopdf::Object::Null)) {
                let xobj_id = crate::XObjectId(name_str);
                // For simplicity, we ignore any transform that was previously set via `cm`.
                out_ops.push(Op::UseXobject {
                    id: xobj_id,
                    transform: crate::xobject::XObjectTransform::default(),
                });
            }
        }

        // Catch everything else
        other => {
            warnings.push(PdfWarnMsg::error(
                page,
                op_id,
                format!("Info: unhandled operator '{}'", other),
            ));
            out_ops.push(Op::Unknown {
                key: op.operator.clone(),
                value: op
                    .operands
                    .iter()
                    .map(|s| DictItem::from_lopdf(s))
                    .collect(),
            });
        }
    }

    Ok(out_ops)
}

/// Returns a default date (Unix epoch)
fn default_date() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(0).unwrap()
}

/// Parses a DocumentInfo dictionary into a PdfDocumentInfo.
pub fn parse_document_info(dict: &LopdfDictionary) -> PdfDocumentInfo {
    let trapped = dict
        .get(b"Trapped")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(s == b"True"),
            Object::Name(n) => Some(n == b"True"),
            _ => None,
        })
        .unwrap_or_default();

    let creation_date = dict
        .get(b"CreationDate")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => parse_pdf_date(&String::from_utf8_lossy(s).to_string()).ok(),
            _ => None,
        })
        .unwrap_or_else(default_date);

    let modification_date = dict
        .get(b"ModDate")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => parse_pdf_date(&String::from_utf8_lossy(s).to_string()).ok(),
            _ => None,
        })
        .unwrap_or_else(default_date);

    // If metadata date wasn't separately written, we default to the modification date.
    let metadata_date = modification_date.clone();

    let conformance = dict
        .get(b"GTS_PDFXVersion")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(PdfConformance::from_identifier_string(
                &String::from_utf8_lossy(s),
            )),
            _ => None,
        })
        .unwrap_or_default();

    let document_title = dict
        .get(b"Title")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let author = dict
        .get(b"Author")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let creator = dict
        .get(b"Creator")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let producer = dict
        .get(b"Producer")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let subject = dict
        .get(b"Subject")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let identifier = dict
        .get(b"Identifier")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .unwrap_or_default();

    let keywords = dict
        .get(b"Keywords")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(decode_text_from_utf16be(s)),
            _ => None,
        })
        .map(|joined| {
            joined
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    PdfDocumentInfo {
        trapped,
        version: 1, // No version key was written, so we default to 1.
        creation_date,
        modification_date,
        metadata_date,
        conformance,
        document_title,
        author,
        creator,
        producer,
        keywords,
        subject,
        identifier,
    }
}

/// Parses a PDF rectangle from an Object (an array of four numbers).
fn parse_rect(obj: &Object) -> Result<crate::graphics::Rect, String> {
    if let Object::Array(arr) = obj {
        if arr.len() != 4 {
            return Err("Rectangle array does not have 4 elements".to_string());
        }
        let nums: Result<Vec<f32>, String> = arr
            .iter()
            .map(|o| match o {
                Object::Integer(i) => Ok(*i as f32),
                Object::Real(r) => Ok(*r),
                _ => Err("Rectangle element is not a number".to_string()),
            })
            .collect();
        let nums = nums?;
        // In PDF the rectangle is given as [llx, lly, urx, ury].
        let x = nums[0];
        let y = nums[1];
        let urx = nums[2];
        let ury = nums[3];
        let width = urx - x;
        let height = ury - y;
        Ok(crate::graphics::Rect::from_xywh(
            crate::units::Pt(x),
            crate::units::Pt(y),
            crate::units::Pt(width),
            crate::units::Pt(height),
        ))
    } else {
        Err("Rectangle is not an array".to_string())
    }
}

/// Helper to parse an operand into f32
fn to_f32(obj: &Object) -> f32 {
    match obj {
        Object::Integer(i) => *i as f32,
        Object::Real(r) => *r,
        _ => 0.0,
    }
}

/// Helper to parse an operand as a PDF name
fn as_name(obj: &Object) -> Option<String> {
    if let Object::Name(ref bytes) = obj {
        Some(String::from_utf8_lossy(bytes).to_string())
    } else {
        None
    }
}

mod links_and_bookmarks {
    use std::collections::BTreeMap;

    use lopdf::{Dictionary, Document, Object, ObjectId};

    use crate::{
        annotation::{
            Actions, BorderArray, ColorArray, DashPhase, Destination, HighlightingMode,
            LinkAnnotation, PageAnnotation,
        },
        graphics::Rect,
        Pt,
    };

    /// Returns an empty vector if any required key is missing.
    pub fn extract_bookmarks(doc: &Document) -> Vec<PageAnnotation> {
        try_extract_bookmarks(doc).unwrap_or_default()
    }

    fn try_extract_bookmarks(doc: &Document) -> Option<Vec<PageAnnotation>> {
        // Retrieve the catalog from the trailer.
        let catalog_id = match doc.trailer.get(b"Root").ok()? {
            Object::Reference(id) => *id,
            _ => return None,
        };
        let catalog = doc.get_object(catalog_id).ok()?.as_dict().ok()?;

        // Get the Outlines dictionary.
        let outlines_ref = match catalog.get(b"Outlines").ok()? {
            Object::Reference(id) => *id,
            _ => return None,
        };
        let outlines = doc.get_object(outlines_ref).ok()?.as_dict().ok()?;

        // Start with the first bookmark.
        let mut current_ref = match outlines.get(b"First").ok()? {
            Object::Reference(id) => Some(*id),
            _ => None,
        };

        // Build a mapping from page object id to page number.
        let page_map: BTreeMap<ObjectId, usize> = doc
            .get_pages()
            .iter()
            .map(|(num, id)| (*id, *num as usize))
            .collect();

        let mut bookmarks = Vec::new();
        while let Some(bm_ref) = current_ref {
            let bm_dict = doc.get_object(bm_ref).ok()?.as_dict().ok()?;
            // Get the title.
            let title = match bm_dict.get(b"Title").ok()? {
                Object::String(s, _) => s.clone(),
                _ => {
                    current_ref = next_bookmark(bm_dict);
                    continue;
                }
            };
            // Get the destination array.
            let dest_array = match bm_dict.get(b"Dest").ok()? {
                Object::Array(arr) => arr,
                _ => {
                    current_ref = next_bookmark(bm_dict);
                    continue;
                }
            };
            // The first element should be a reference to a page.
            let page_num = if let Some(Object::Reference(page_id)) = dest_array.get(0) {
                *page_map.get(page_id).unwrap_or(&0)
            } else {
                0
            };

            bookmarks.push(PageAnnotation {
                name: decode_possible_utf16be(&title),
                page: page_num,
            });

            // Move on to the next bookmark.
            current_ref = next_bookmark(bm_dict);
        }
        Some(bookmarks)
    }

    /// Decodes a byte array that might be UTF-16BE encoded.
    /// Returns a String regardless of the input encoding.
    fn decode_possible_utf16be(bytes: &[u8]) -> String {
        // Early return if not UTF-16BE BOM
        if bytes.len() < 2 || bytes[0] != 0xFE || bytes[1] != 0xFF {
            return String::from_utf8_lossy(bytes).to_string();
        }

        // Skip BOM (first 2 bytes) and decode as UTF-16BE
        let mut chars = Vec::with_capacity((bytes.len() - 2) / 2);
        let mut i = 2;

        while i + 1 < bytes.len() {
            let high = bytes[i] as u16;
            let low = bytes[i + 1] as u16;
            let code_point = (high << 8) | low;
            chars.push(code_point);
            i += 2;
        }

        // Handle odd length (shouldn't happen in well-formed UTF-16)
        if i < bytes.len() {
            let high = bytes[i] as u16;
            chars.push(high << 8);
        }

        String::from_utf16_lossy(&chars)
    }

    fn next_bookmark(dict: &Dictionary) -> Option<ObjectId> {
        dict.get(b"Next").ok().and_then(|obj| match obj {
            Object::Reference(id) => Some(*id),
            _ => None,
        })
    }

    /// Extracts all link annotations from all pages using functional combinators.
    pub fn extract_link_annotations(doc: &Document) -> Vec<LinkAnnotation> {
        // Build a page mapping from object id to page number.
        let page_map: BTreeMap<ObjectId, usize> = doc
            .get_pages()
            .iter()
            .map(|(num, id)| (*id, *num as usize))
            .collect();

        // For every page, try to extract its annotations.
        doc.get_pages()
            .values()
            .flat_map(|&page_id| {
                let page_dict = doc.get_object(page_id).ok()?.as_dict().ok()?;
                let annots = page_dict.get(b"Annots").ok()?.as_array().ok()?;
                Some(
                    annots
                        .iter()
                        .filter_map(|annot_obj| {
                            // Expect a reference to an annotation dictionary.
                            let annot_ref = match annot_obj {
                                Object::Reference(id) => *id,
                                _ => return None,
                            };
                            let annot_dict = doc.get_object(annot_ref).ok()?.as_dict().ok()?;
                            // Only process annotations with Subtype "Link".
                            if annot_dict.get(b"Subtype").ok()?.as_name().ok()? != b"Link" {
                                return None;
                            }
                            // Extract the rectangle.
                            let rect_arr = annot_dict.get(b"Rect").ok()?.as_array().ok()?;
                            if rect_arr.len() != 4 {
                                return None;
                            }
                            let coords: Vec<f32> = rect_arr
                                .iter()
                                .filter_map(|obj| match obj {
                                    Object::Real(r) => Some(*r as f32),
                                    Object::Integer(i) => Some(*i as f32),
                                    _ => None,
                                })
                                .collect();
                            if coords.len() != 4 {
                                return None;
                            }
                            let rect = Rect::from_xywh(
                                Pt(coords[0]),
                                Pt(coords[1]),
                                Pt(coords[2]),
                                Pt(coords[3]),
                            );

                            // Extract the action.
                            let action_dict = annot_dict.get(b"A").ok()?.as_dict().ok()?;
                            let actions = extract_action(action_dict, &page_map)?;

                            // Extract optional Border.
                            let border = annot_dict
                                .get(b"Border")
                                .ok()
                                .and_then(|obj| obj.as_array().ok())
                                .map(|s| extract_border_array(s.as_slice()))
                                .unwrap_or_else(BorderArray::default);

                            // Extract optional Color.
                            let color = annot_dict
                                .get(b"C")
                                .ok()
                                .and_then(|obj| obj.as_array().ok())
                                .map(|s| extract_color_array(s.as_slice()))
                                .unwrap_or_else(ColorArray::default);

                            // Extract highlighting mode.
                            let highlighting = annot_dict
                                .get(b"H")
                                .and_then(|obj| obj.as_name())
                                .map(|n| match n {
                                    n if n == b"N" => HighlightingMode::None,
                                    n if n == b"I" => HighlightingMode::Invert,
                                    n if n == b"O" => HighlightingMode::Outline,
                                    n if n == b"P" => HighlightingMode::Push,
                                    _ => HighlightingMode::Invert,
                                })
                                .unwrap_or(HighlightingMode::Invert);

                            Some(LinkAnnotation {
                                rect,
                                actions,
                                border,
                                color,
                                highlighting,
                            })
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect()
    }

    /// Converts a PDF array into our BorderArray.
    fn extract_border_array(arr: &[Object]) -> BorderArray {
        let nums: Vec<f32> = arr
            .iter()
            .filter_map(|obj| match obj {
                Object::Integer(i) => Some(*i as f32),
                Object::Real(r) => Some(*r as f32),
                _ => None,
            })
            .collect();
        if nums.len() == 3 {
            BorderArray::Solid([nums[0], nums[1], nums[2]])
        } else if nums.len() == 4 {
            BorderArray::Dashed(
                [nums[0], nums[1], nums[2]],
                DashPhase {
                    dash_array: vec![],
                    phase: nums[3],
                },
            )
        } else {
            BorderArray::default()
        }
    }

    /// Converts a PDF array into our ColorArray.
    fn extract_color_array(arr: &[Object]) -> ColorArray {
        let nums: Vec<f32> = arr
            .iter()
            .filter_map(|obj| match obj {
                Object::Integer(i) => Some(*i as f32),
                Object::Real(r) => Some(*r as f32),
                _ => None,
            })
            .collect();

        match nums.len() {
            0 => ColorArray::Transparent,
            1 => ColorArray::Gray([nums[0]]),
            3 => ColorArray::Rgb([nums[0], nums[1], nums[2]]),
            4 => ColorArray::Cmyk([nums[0], nums[1], nums[2], nums[3]]),
            _ => ColorArray::default(),
        }
    }

    /// Converts a PDF action dictionary into our Actions enum.
    fn extract_action(dict: &Dictionary, page_map: &BTreeMap<ObjectId, usize>) -> Option<Actions> {
        let action_type = dict.get(b"S").ok()?.as_name().ok()?;
        match action_type {
            n if n == b"GoTo" => {
                let arr = dict.get(b"D").ok()?.as_array().ok()?;
                let page = if let Some(Object::Reference(page_id)) = arr.get(0) {
                    *page_map.get(page_id).unwrap_or(&0)
                } else {
                    0
                };
                let left = arr.get(2).and_then(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    _ => None,
                });
                let top = arr.get(3).and_then(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    _ => None,
                });
                let zoom = arr.get(4).and_then(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    _ => None,
                });
                Some(Actions::Goto(Destination::Xyz {
                    page,
                    left,
                    top,
                    zoom,
                }))
            }
            n if n == b"URI" => {
                let uri = String::from_utf8_lossy(&dict.get(b"URI").ok()?.as_str().ok()?);
                Some(Actions::Uri(uri.to_string()))
            }
            _ => None,
        }
    }
}

mod layers {

    use std::collections::BTreeSet;

    use lopdf::{Document, Object, ObjectId};

    use crate::ops::{Layer, LayerIntent, LayerSubtype};

    /// Extract layers from the PDF document.
    /// This function looks into the catalog's "OCProperties" dictionary and parses the "OCGs"
    /// array.
    pub fn extract_layers(doc: &Document) -> Vec<Layer> {
        // Get the catalog from the trailer.
        let catalog_id = match doc.trailer.get(b"Root").ok() {
            Some(Object::Reference(id)) => *id,
            _ => return vec![],
        };
        let catalog = match doc.get_object(catalog_id).ok() {
            Some(Object::Dictionary(dict)) => dict,
            _ => return vec![],
        };

        // Get the OCProperties dictionary.
        let ocprops = match catalog.get(b"OCProperties").ok() {
            Some(Object::Dictionary(dict)) => dict,
            _ => return vec![],
        };

        // Get the array of OCGs (optional content groups / layers).
        let ocgs = match ocprops.get(b"OCGs").ok() {
            Some(Object::Array(arr)) => arr,
            _ => return vec![],
        };

        // Also, if available, get the "ON" array from the "D" dictionary
        // to decide which layers are turned on by default.
        let default_on = match ocprops.get(b"D").ok() {
            Some(Object::Dictionary(d_dict)) => {
                if let Some(Object::Array(on_arr)) = d_dict.get(b"ON").ok() {
                    on_arr
                        .iter()
                        .filter_map(|obj| {
                            if let Object::Reference(id) = obj {
                                Some(*id)
                            } else {
                                None
                            }
                        })
                        .collect::<BTreeSet<ObjectId>>()
                } else {
                    BTreeSet::new()
                }
            }
            _ => BTreeSet::new(),
        };

        let mut layers = Vec::new();
        for obj in ocgs {
            // Each entry should be a reference to a layer dictionary.
            let layer_ref = match obj {
                Object::Reference(id) => id,
                _ => continue,
            };
            let layer_dict = match doc.get_object(*layer_ref) {
                Ok(Object::Dictionary(dict)) => dict,
                _ => continue,
            };

            // Extract the layer name.
            let name = match layer_dict.get(b"Name").ok() {
                Some(Object::String(s, _)) => s.clone(),
                _ => continue,
            };

            // Extract the intent from the "Intent" key.
            // The Intent value is stored as a reference to an array of names.
            let intent = match layer_dict.get(b"Intent").ok() {
                Some(Object::Reference(intent_ref)) => {
                    if let Ok(Object::Array(arr)) = doc.get_object(*intent_ref) {
                        if let Some(Object::Name(n)) = arr.first() {
                            match n.as_slice() {
                                b"View" => LayerIntent::View,
                                b"Design" => LayerIntent::Design,
                                _ => LayerIntent::Design,
                            }
                        } else {
                            LayerIntent::Design
                        }
                    } else {
                        LayerIntent::Design
                    }
                }
                _ => LayerIntent::Design,
            };

            // Extract usage info from the "Usage" key.
            // This is a reference to a dictionary containing "CreatorInfo".
            let (creator, usage_subtype) = match layer_dict.get(b"Usage").ok() {
                Some(Object::Reference(usage_ref)) => {
                    let usage_dict = match doc.get_object(*usage_ref) {
                        Ok(Object::Dictionary(dict)) => dict,
                        _ => continue,
                    };
                    if let Some(Object::Dictionary(creator_info)) =
                        usage_dict.get(b"CreatorInfo").ok()
                    {
                        let creator = match creator_info.get(b"Creator").ok() {
                            Some(Object::String(s, _)) => String::from_utf8_lossy(&s).to_string(),
                            _ => String::new(),
                        };
                        let usage_subtype = match creator_info.get(b"Subtype").ok() {
                            Some(Object::Name(n)) => match n.as_slice() {
                                b"Artwork" => LayerSubtype::Artwork,
                                _ => LayerSubtype::Artwork,
                            },
                            _ => LayerSubtype::Artwork,
                        };
                        (creator, usage_subtype)
                    } else {
                        (String::new(), LayerSubtype::Artwork)
                    }
                }
                _ => (String::new(), LayerSubtype::Artwork),
            };

            // Decide final intent: if this layer's object ID is in the default "ON" set,
            // we treat its intent as View.
            let final_intent = if default_on.contains(&layer_ref) {
                LayerIntent::View
            } else {
                intent
            };

            layers.push(Layer {
                name: String::from_utf8_lossy(&name).to_string(), // TODO: better parsing
                creator,
                intent: final_intent,
                usage: usage_subtype,
            });
        }

        layers
    }
}

mod extgstate {

    use std::collections::HashSet;

    use lopdf::{Dictionary as LoDictionary, Object};

    use crate::{
        graphics::{
            BlendMode, ChangedField, ExtendedGraphicsState, LineCapStyle, LineDashPattern,
            LineJoinStyle, OverprintMode, RenderingIntent,
        },
        BuiltinFont, BuiltinOrExternalFontId, FontId,
    };

    /// Given a PDF ExtGState dictionary, parse it into an ExtendedGraphicsState.
    pub fn parse_extgstate(dict: &LoDictionary) -> ExtendedGraphicsState {
        let mut gs = ExtendedGraphicsState::default();
        let mut changed = HashSet::new();

        if let Some(obj) = dict.get(b"LW").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.line_width = num;
                changed.insert(ChangedField::LineWidth);
            }
        }

        if let Some(obj) = dict.get(b"LC").ok() {
            if let Some(num) = parse_i64(obj) {
                gs.line_cap = match num {
                    0 => LineCapStyle::Butt,
                    1 => LineCapStyle::Round,
                    2 => LineCapStyle::ProjectingSquare,
                    _ => LineCapStyle::Butt,
                };
                changed.insert(ChangedField::LineCap);
            }
        }

        if let Some(obj) = dict.get(b"LJ").ok() {
            if let Some(num) = parse_i64(obj) {
                gs.line_join = match num {
                    0 => LineJoinStyle::Miter,
                    1 => LineJoinStyle::Round,
                    2 => LineJoinStyle::Bevel,
                    _ => LineJoinStyle::Miter,
                };
                changed.insert(ChangedField::LineJoin);
            }
        }

        if let Some(obj) = dict.get(b"ML").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.miter_limit = num;
                changed.insert(ChangedField::MiterLimit);
            }
        }

        if let Some(obj) = dict.get(b"FL").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.flatness_tolerance = num;
                changed.insert(ChangedField::FlatnessTolerance);
            }
        }

        if let Some(obj) = dict.get(b"RI").ok() {
            if let Some(name) = parse_name(obj) {
                gs.rendering_intent = match name.as_str() {
                    "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
                    "RelativeColorimetric" => RenderingIntent::RelativeColorimetric,
                    "Saturation" => RenderingIntent::Saturation,
                    "Perceptual" => RenderingIntent::Perceptual,
                    _ => RenderingIntent::RelativeColorimetric,
                };
                changed.insert(ChangedField::RenderingIntent);
            }
        }

        if let Some(obj) = dict.get(b"SA").ok() {
            if let Some(b) = parse_bool(obj) {
                gs.stroke_adjustment = b;
                changed.insert(ChangedField::StrokeAdjustment);
            }
        }

        if let Some(obj) = dict.get(b"OP").ok() {
            if let Some(b) = parse_bool(obj) {
                gs.overprint_fill = b;
                changed.insert(ChangedField::OverprintFill);
            }
        }

        if let Some(obj) = dict.get(b"op").ok() {
            if let Some(b) = parse_bool(obj) {
                gs.overprint_stroke = b;
                changed.insert(ChangedField::OverprintStroke);
            }
        }

        if let Some(obj) = dict.get(b"OPM").ok() {
            if let Some(num) = parse_i64(obj) {
                gs.overprint_mode = match num {
                    0 => OverprintMode::EraseUnderlying,
                    1 => OverprintMode::KeepUnderlying,
                    _ => OverprintMode::EraseUnderlying,
                };
                changed.insert(ChangedField::OverprintMode);
            }
        }

        if let Some(obj) = dict.get(b"CA").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.current_fill_alpha = num;
                changed.insert(ChangedField::CurrentFillAlpha);
            }
        }

        if let Some(obj) = dict.get(b"ca").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.current_stroke_alpha = num;
                changed.insert(ChangedField::CurrentStrokeAlpha);
            }
        }

        if let Some(obj) = dict.get(b"BM").ok() {
            if let Some(name) = parse_name(obj) {
                gs.blend_mode = parse_blend_mode(&name);
                changed.insert(ChangedField::BlendMode);
            }
        }

        if let Some(obj) = dict.get(b"AIS").ok() {
            if let Some(b) = parse_bool(obj) {
                gs.alpha_is_shape = b;
                changed.insert(ChangedField::AlphaIsShape);
            }
        }

        if let Some(obj) = dict.get(b"TK").ok() {
            if let Some(b) = parse_bool(obj) {
                gs.text_knockout = b;
                changed.insert(ChangedField::TextKnockout);
            }
        }

        if let Some(obj) = dict.get(b"D").ok() {
            if let Some(arr) = obj.as_array().ok() {
                // Parse the dash pattern as a flat array of integers.
                let dashes: Vec<i64> = arr.iter().filter_map(|o| parse_i64(o)).collect();
                gs.line_dash_pattern = Some(LineDashPattern::from_array(&dashes, 0)); // default offset = 0
                changed.insert(ChangedField::LineDashPattern);
            }
        }

        if let Some(obj) = dict.get(b"Font").ok() {
            if let Some(name) = parse_name(obj) {
                // Here we assume FontId is a newtype wrapping a String.
                gs.font = Some(match BuiltinFont::from_id(&name) {
                    Some(s) => BuiltinOrExternalFontId::Builtin(s),
                    None => BuiltinOrExternalFontId::External(FontId(name)),
                });
                changed.insert(ChangedField::Font);
            }
        }

        if let Some(obj) = dict.get(b"SM").ok() {
            if let Some(name) = parse_name(obj) {
                // When written, a missing soft mask was output as "None"
                if name == "None" {
                    gs.soft_mask = None;
                } else {
                    // (Parsing a non-"None" soft mask is left as an exercise.)
                    gs.soft_mask = None;
                }
                changed.insert(ChangedField::SoftMask);
            }
        }

        // (Other keys such as black generation, under-color removal, transfer functions,
        // halftone dictionary, etc. could be parsed similarly when implemented.)

        gs.changed_fields = changed;
        gs
    }

    /// Helper: Convert a PDF Object into an f32, if possible.
    fn parse_f32(obj: &Object) -> Option<f32> {
        match obj {
            Object::Real(r) => Some(*r),
            Object::Integer(i) => Some(*i as f32),
            _ => None,
        }
    }

    /// Helper: Convert a PDF Object into an i64, if possible.
    fn parse_i64(obj: &Object) -> Option<i64> {
        match obj {
            Object::Integer(i) => Some(*i),
            Object::Real(r) => Some(*r as i64),
            _ => None,
        }
    }

    /// Helper: Convert a PDF Object into a bool, if possible.
    fn parse_bool(obj: &Object) -> Option<bool> {
        match obj {
            Object::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Helper: Convert a PDF Name object into a String.
    fn parse_name(obj: &Object) -> Option<String> {
        match obj {
            Object::Name(bytes) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        }
    }

    /// Helper: Convert a name string into a BlendMode.
    fn parse_blend_mode(s: &str) -> BlendMode {
        use crate::graphics::{BlendMode, NonSeperableBlendMode, SeperableBlendMode};
        match s {
            "Normal" => BlendMode::Seperable(SeperableBlendMode::Normal),
            "Multiply" => BlendMode::Seperable(SeperableBlendMode::Multiply),
            "Screen" => BlendMode::Seperable(SeperableBlendMode::Screen),
            "Overlay" => BlendMode::Seperable(SeperableBlendMode::Overlay),
            "Darken" => BlendMode::Seperable(SeperableBlendMode::Darken),
            "Lighten" => BlendMode::Seperable(SeperableBlendMode::Lighten),
            "ColorDodge" => BlendMode::Seperable(SeperableBlendMode::ColorDodge),
            "ColorBurn" => BlendMode::Seperable(SeperableBlendMode::ColorBurn),
            "HardLight" => BlendMode::Seperable(SeperableBlendMode::HardLight),
            "SoftLight" => BlendMode::Seperable(SeperableBlendMode::SoftLight),
            "Difference" => BlendMode::Seperable(SeperableBlendMode::Difference),
            "Exclusion" => BlendMode::Seperable(SeperableBlendMode::Exclusion),
            "Hue" => BlendMode::NonSeperable(NonSeperableBlendMode::Hue),
            "Saturation" => BlendMode::NonSeperable(NonSeperableBlendMode::Saturation),
            "Color" => BlendMode::NonSeperable(NonSeperableBlendMode::Color),
            "Luminosity" => BlendMode::NonSeperable(NonSeperableBlendMode::Luminosity),
            _ => BlendMode::Seperable(SeperableBlendMode::Normal),
        }
    }
}

mod parsefont {
    use std::collections::BTreeMap;

    use lopdf::{Dictionary, Document, Object};

    use super::ParsedOrBuiltinFont;
    use crate::{
        cmap::ToUnicodeCMap,
        deserialize::{get_dict_or_resolve_ref, PdfWarnMsg},
        BuiltinFont, FontId, ParsedFont,
    };

    /// Main function to parse fonts from PDF resources
    pub(crate) fn parse_fonts_enhanced(
        doc: &Document,
        resources: &Dictionary,
        warnings: &mut Vec<PdfWarnMsg>,
        page: Option<usize>,
    ) -> BTreeMap<FontId, ParsedOrBuiltinFont> {
        let mut fonts_map = BTreeMap::new();
        let page_num = page.unwrap_or_default();

        // Get the fonts dictionary from resources
        let fonts_dict = match get_fonts_dict_from_resources(doc, resources, warnings, page_num) {
            Some(dict) => dict,
            None => return fonts_map,
        };

        // Process each font in the dictionary
        for (key, value) in fonts_dict.iter() {
            let font_id = FontId(String::from_utf8_lossy(key).to_string());

            // Resolve font dictionary reference
            let font_dict = match get_dict_or_resolve_ref(
                &format!("parse_fonts font_dict page {page_num}"),
                doc,
                value,
                warnings,
                page,
            ) {
                Some(s) => s,
                None => continue,
            };

            // Check font type
            let font_type = match get_font_subtype(font_dict, &font_id, warnings, page_num) {
                Some(t) => t,
                None => continue,
            };

            // Extract ToUnicode CMap directly from the font dictionary (common to all font types)
            let to_unicode_cmap = extract_to_unicode_cmap(doc, font_dict, warnings, page_num);

            // Handle different font types
            if &font_type == b"Type0" {
                if let Some(parsed_font) =
                    process_type0_font(doc, font_dict, &font_id, warnings, page_num)
                {
                    fonts_map.insert(
                        font_id,
                        ParsedOrBuiltinFont::P(parsed_font, to_unicode_cmap),
                    );
                }
            } else {
                match process_standard_font(doc, font_dict, &font_id, warnings, page_num) {
                    Some(ParsedOrBuiltinFont::B(builtin)) => {
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::B(builtin)
                        );
                    },
                    Some(ParsedOrBuiltinFont::P(parsed_font, _)) => {
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::P(parsed_font, to_unicode_cmap)
                        );
                    },
                    None => {}
                }
            }
        }

        fonts_map
    }

    // Extract ToUnicode CMap from a font dictionary
    pub(crate) fn extract_to_unicode_cmap(
        doc: &Document,
        font_dict: &Dictionary,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<ToUnicodeCMap> {
        // Check if font dictionary has a ToUnicode entry
        if let Ok(to_unicode_ref) = font_dict.get(b"ToUnicode") {
            // Get the ToUnicode stream
            let to_unicode_stream = match to_unicode_ref {
                Object::Stream(s) => Some(s),
                Object::Reference(r) => match doc.get_object(*r) {
                    Ok(Object::Stream(s)) => Some(s),
                    _ => None,
                },
                _ => None,
            };

            if let Some(stream) = to_unicode_stream {
                // Decompress the stream content
                let content = match stream.decompressed_content() {
                    Ok(data) => data,
                    Err(_) => stream.content.clone(),
                };

                // Convert to string
                if let Ok(cmap_str) = String::from_utf8(content) {
                    // Parse using ToUnicodeCMap::parse
                    match ToUnicodeCMap::parse(&cmap_str) {
                        Ok(cmap) => {
                            return Some(cmap);
                        }
                        Err(e) => {
                            warnings.push(PdfWarnMsg::warning(
                                page_num,
                                0,
                                format!("Failed to parse ToUnicode CMap: {}", e),
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    /// Get fonts dictionary from PDF resources
    fn get_fonts_dict_from_resources<'a>(
        doc: &'a Document,
        resources: &'a Dictionary,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<&'a Dictionary> {
        // Get Font dictionary from Resources
        let font_map = match resources.get(b"Font") {
            Ok(s) => s,
            Err(_) => return None,
        };

        // Resolve Font dictionary reference
        get_dict_or_resolve_ref(
            &format!("parse_fonts Font page {page_num}"),
            doc,
            font_map,
            warnings,
            Some(page_num),
        )
    }

    /// Get font subtype from font dictionary
    fn get_font_subtype(
        font_dict: &Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<Vec<u8>> {
        match font_dict.get(b"Subtype") {
            Ok(Object::Name(n)) => Some(n.clone()),
            _ => {
                warnings.push(PdfWarnMsg::warning(
                    page_num,
                    0,
                    format!("Font {} missing Subtype", font_id.0),
                ));
                None
            }
        }
    }

    /// Process a Type0 (composite) font
    fn process_type0_font(
        doc: &Document,
        font_dict: &Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<ParsedFont> {
        // Get the descendant font dictionary
        let descendant_font_dict =
            get_descendant_font_dict(doc, font_dict, font_id, warnings, page_num)?;

        // Get the font descriptor
        let font_descriptor =
            get_font_descriptor(doc, descendant_font_dict, font_id, warnings, page_num)?;

        // Process font data (no need to handle CMap here anymore)
        process_font_data(doc, font_dict, font_descriptor, font_id, warnings, page_num)
    }

    /// Get the descendant font dictionary for a Type0 font
    fn get_descendant_font_dict<'a>(
        doc: &'a Document,
        font_dict: &'a Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<&'a Dictionary> {
        // Get DescendantFonts array
        match font_dict.get(b"DescendantFonts") {
            Ok(Object::Array(arr)) if !arr.is_empty() => {
                // Get first descendant font
                match arr[0].as_dict().ok().or_else(|| {
                    if let Ok(id) = arr[0].as_reference() {
                        doc.get_dictionary(id).ok()
                    } else {
                        None
                    }
                }) {
                    Some(d) => Some(d),
                    None => {
                        warnings.push(PdfWarnMsg::warning(
                            page_num,
                            0,
                            format!("Cannot resolve descendant font for {}", font_id.0),
                        ));
                        None
                    }
                }
            },
            Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
            _ => {
                warnings.push(PdfWarnMsg::warning(
                    page_num,
                    0,
                    format!("Type0 font {} missing DescendantFonts", font_id.0),
                ));
                None
            }
        }
    }

    /// Get the font descriptor dictionary
    fn get_font_descriptor<'a>(
        doc: &'a Document,
        font_dict: &'a Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<&'a Dictionary> {
        match font_dict.get(b"FontDescriptor") {
            Ok(Object::Dictionary(fd)) => Some(fd),
            Ok(Object::Reference(r)) => match doc.get_dictionary(*r) {
                Ok(fd) => Some(fd),
                Err(_) => {
                    warnings.push(PdfWarnMsg::warning(
                        page_num,
                        0,
                        format!("Cannot resolve FontDescriptor for {}", font_id.0),
                    ));
                    None
                }
            },
            _ => {
                warnings.push(PdfWarnMsg::warning(
                    page_num,
                    0,
                    format!("Font {} missing FontDescriptor", font_id.0),
                ));
                None
            }
        }
    }

    /// Process font data (extract, decompress, parse) and handle ToUnicode CMap
    fn process_font_data(
        doc: &Document,
        _font_dict: &Dictionary,
        font_descriptor: &Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<ParsedFont> {
        // Try each font file type
        for font_file_key in &[b"FontFile", b"FontFile2" as &[u8], b"FontFile3"] {
            if let Ok(font_file_ref) = font_descriptor.get(font_file_key) {
                // Get and decompress font data
                let font_data = get_font_data(doc, font_file_ref, warnings)?;

                warnings.push(PdfWarnMsg::info(
                    page_num,
                    0,
                    format!(
                        "Found font data for {} ({} bytes)",
                        font_id.0,
                        font_data.len()
                    ),
                ));

                // Parse the font
                let mut font_warnings = Vec::new();
                if let Some(parsed_font) = ParsedFont::from_bytes(&font_data, 0, &mut font_warnings) {
                    // Convert FontParseWarnings to PdfWarnMsg if needed
                    for fw in font_warnings {
                        warnings.push(PdfWarnMsg::warning(page_num, 0, fw.message));
                    }
                    return Some(parsed_font);
                } else {
                    warnings.push(PdfWarnMsg::error(
                        page_num,
                        0,
                        format!("Failed to parse font data for {}", font_id.0),
                    ));
                }
            }
        }
        None
    }

    /// Get and decompress font data from a font file reference
    fn get_font_data(
        doc: &Document,
        font_file_ref: &Object,
        _warnings: &mut Vec<PdfWarnMsg>,
    ) -> Option<Vec<u8>> {
        // Get font stream
        let font_stream = match font_file_ref {
            Object::Stream(s) => s,
            Object::Reference(r) => match doc.get_object(*r) {
                Ok(Object::Stream(s)) => s,
                _ => return None,
            },
            _ => return None,
        };

        // Decompress font data
        match font_stream.decompressed_content() {
            Ok(data) => Some(data),
            Err(_) => Some(font_stream.content.clone()),
        }
    }

    /// Process a Type1 font
    fn process_type1_font(
        doc: &Document,
        font_dict: &Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<ParsedFont> {
        // Get the font descriptor
        let font_descriptor =
            get_font_descriptor(doc, font_dict, font_id, warnings, page_num)?;

        // Process font data (no need to handle CMap here anymore)
        process_font_data(doc, font_dict, font_descriptor, font_id, warnings, page_num)
    }

    /// Process a standard font (Type1, TrueType, etc.)
    fn process_standard_font(
        doc: &Document,
        font_dict: &Dictionary,
        font_id: &FontId,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<ParsedOrBuiltinFont> {
        // Get BaseFont name
        let basefont = match font_dict.get(b"BaseFont") {
            Ok(Object::Name(basefont_bytes)) => String::from_utf8_lossy(basefont_bytes).to_string(),
            _ => return None,
        };

        // Look up built-in font
        match BuiltinFont::from_id(&basefont) {
            Some(builtin) => Some(ParsedOrBuiltinFont::B(builtin)),
            None => {
                match process_type1_font(doc, font_dict, font_id, warnings, page_num) {
                    Some(parsed_font) => {
                        Some(ParsedOrBuiltinFont::P(parsed_font, None))
                    },
                    None => {
                        warnings.push(PdfWarnMsg::warning(
                            page_num,
                            0,
                            format!("Unknown base font: {}", basefont),
                        ));
                        None
                    }
                }
            }
        }
    }
}

// Decode text from UTF-16BE with BOM
fn decode_text_from_utf16be(bytes: &[u8]) -> String {
    if bytes.len() < 2 {
        // not UTF-16BE encoded, use UTF-8
        return String::from_utf8_lossy(bytes).into_owned();
    }

    // check for Byte Order Mask
    if bytes[0] != 0xFE || bytes[1] != 0xFF {
        // not UTF-16BE encoded, use UTF-8
        return String::from_utf8_lossy(bytes).into_owned();
    }

    // Decode from UTF-16BE
    let (chunks, remainder) = bytes[2..].as_chunks::<2>();
    let string = char::decode_utf16(chunks.iter().copied().map(u16::from_be_bytes))
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    if remainder.is_empty() { string } else { string + "\u{FFFD}" }
}
