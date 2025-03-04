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
    BuiltinFont, BuiltinOrExternalFontId, Color, DictItem, ExtendedGraphicsState,
    ExtendedGraphicsStateId, ExtendedGraphicsStateMap, FontId, LayerInternalId, LineDashPattern,
    LinePoint, Op, PageAnnotId, PageAnnotMap, ParsedFont, PdfDocument, PdfDocumentInfo, PdfFontMap,
    PdfLayerMap, PdfMetadata, PdfPage, PdfResources, PolygonRing, Pt, RawImage, RenderingIntent,
    TextItem, TextMatrix, TextRenderingMode, XObject, XObjectId, XObjectMap,
    cmap::ToUnicodeCMap,
    conformance::PdfConformance,
    date::{OffsetDateTime, UtcOffset},
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
}

fn parse_pdf_from_bytes_start(
    bytes: &[u8],
    opts: &PdfParseOptions,
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

    Ok(InitialPdf {
        doc,
        objs_to_search_for_resources,
        page_refs,
        document_info,
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
    opts: &PdfParseOptions,
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
    } = initial_pdf;

    for (i, page_ref) in page_refs.into_iter().enumerate() {
        let page_obj = doc
            .get_object(page_ref)
            .map_err(|e| format!("Failed to get page object: {}", e))?
            .as_dict()
            .map_err(|e| format!("Page object is not a dictionary: {}", e))?;

        let pdf_page = parse_page(i, page_obj, &doc, &fonts, &xobjects, warnings)?;

        pages.push(pdf_page);
    }

    // Extract bookmarks and layers from the document
    let bookmarks = links_and_bookmarks::extract_bookmarks(&doc);
    let layers = layers::extract_layers(&doc);

    // Extract ExtGStates from resources
    let extgstates = extract_extgstates(&doc, &objs_to_search_for_resources, warnings);

    let fonts = fonts
        .into_iter()
        .filter_map(|(id, pf)| Some((FontId(id.get_id().to_string()), pf.as_parsed_font()?)))
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
    P(ParsedFont),
    B(BuiltinFont),
}

impl ParsedOrBuiltinFont {
    fn as_parsed_font(self) -> Option<ParsedFont> {
        match self {
            ParsedOrBuiltinFont::P(p) => Some(p),
            ParsedOrBuiltinFont::B(_) => None,
        }
    }
}

/// Given a Resources dictionary from a page or a global document dictionary,
/// parse all embedded fonts and return a mapping from FontId to ParsedFont.
fn parse_fonts(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
    warnings: &mut Vec<PdfWarnMsg>,
    page: Option<usize>,
) -> BTreeMap<FontId, ParsedOrBuiltinFont> {
    let mut fonts_map = BTreeMap::new();

    let page_num = page.unwrap_or_default();

    let font_map = match resources.get(b"Font") {
        Ok(s) => s,
        Err(_) => return fonts_map,
    };

    let fonts_dict = match get_dict_or_resolve_ref(
        &format!("parse_fonts Font page {page_num}"),
        doc,
        font_map,
        warnings,
        page,
    ) {
        Some(s) => s,
        None => return fonts_map,
    };

    'outer: for (key, value) in fonts_dict.iter() {
        // TODO: better parsing
        let font_id = FontId(String::from_utf8_lossy(key).to_string());

        let font_entry = match get_dict_or_resolve_ref(
            &format!("parse_fonts FontFile1 FontFile2 FontFile3 page {page_num}"),
            doc,
            value,
            warnings,
            page,
        ) {
            Some(s) => s,
            None => continue,
        };

        for f in ["FontFile", "FontFile2", "FontFile3"] {
            let f_ref = match font_entry.get(f.as_bytes()) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let font_stream = match get_stream_or_resolve_ref(doc, f_ref, warnings, page) {
                Some(s) => s,
                None => continue,
            };

            let stream = font_stream
                .decompressed_content()
                .unwrap_or_else(|_| font_stream.content.clone());

            warnings.push(PdfWarnMsg::info(
                0,
                0,
                format!(
                    "deserializing font stream for {}, {} bytes",
                    font_id.0,
                    stream.len()
                ),
            ));

            match ParsedFont::from_bytes(&stream, 0, warnings) {
                Some(o) => {
                    fonts_map.insert(font_id, ParsedOrBuiltinFont::P(o));
                    continue 'outer;
                }
                None => {
                    warnings.push(PdfWarnMsg::error(
                        page.unwrap_or(0),
                        0,
                        format!("font {}: corrupt {f}", font_id.0),
                    ));
                }
            }
        }

        let basefont = match font_entry.get(b"BaseFont") {
            Ok(Object::Name(basefont_bytes)) => {
                String::from_utf8_lossy(&basefont_bytes).to_string()
            }
            _ => continue,
        };

        match BuiltinFont::from_id(&basefont) {
            Some(s) => {
                fonts_map.insert(font_id, ParsedOrBuiltinFont::B(s));
            }
            None => {
                warnings.push(PdfWarnMsg::error(
                    page.unwrap_or(0),
                    0,
                    format!("font {}: unknown basefont {basefont}", font_id.0),
                ));
            }
        }
    }

    fonts_map
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
            Some(parse_fonts(doc, resources_dict, warnings, Some(page_num)))
        })
        .fold(BTreeMap::new(), |mut acc, fonts| {
            fonts.into_iter().for_each(|(font_id, parsed_font)| {
                acc.entry(font_id).or_insert(parsed_font);
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

    map.extend(
        obj.into_iter()
            .map(|(k, v)| (BuiltinOrExternalFontId::External(k), v)),
    );

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
    fonts: &BTreeMap<BuiltinOrExternalFontId, ParsedOrBuiltinFont>,
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
                .map_err(|e| format!("Failed to decompress content: {}", e))?;
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
        let parsed_op = parse_op(num, op_id, &op, &mut page_state, fonts, xobjects, warnings)?;
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

    /// Current transformation matrix stack. Each entry is a 6-float array [a b c d e f].
    pub transform_stack: Vec<[f32; 6]>,

    /// Name of the current layer, if any (set by BDC with /OC).
    pub current_layer: Option<String>,

    /// Accumulated subpaths. Each subpath is a list of `(Point, is_bezier_control_point)`.
    /// We store multiple subpaths so that if the path has `m`, `l`, `c`, `m` again, etc.,
    /// they become separate “rings” or subpaths. We only produce a final shape on stroke/fill.
    pub subpaths: Vec<Vec<(crate::graphics::Point, bool)>>,

    /// The subpath currently being constructed (i.e. after the last `m`).
    pub current_subpath: Vec<(crate::graphics::Point, bool)>,

    /// True if we have a "closepath" (like the `h` operator) for the current subpath.
    /// Some PDF operators forcibly close subpaths, e.g. `b` / `s` vs. `B` / `S`.
    pub current_subpath_closed: bool,

    pub last_emitted_font_size: Option<(FontId, Pt)>,
}

/// Convert a single lopdf Operation into zero, one, or many `printpdf::Op`.
/// We maintain / mutate `PageState` so that repeated path operators (`m`, `l`, `c`, etc.)
/// accumulate subpaths, and we only emit path-based Ops at stroke or fill time.
pub fn parse_op(
    page: usize,
    op_id: usize,
    op: &lopdf::content::Operation,
    state: &mut PageState,
    fonts: &BTreeMap<BuiltinOrExternalFontId, ParsedOrBuiltinFont>,
    xobjects: &BTreeMap<XObjectId, XObject>,
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
                // Get the font for CMap lookup
                let to_unicode_cmap =
                    if let (Some(fid), _) = (&state.current_font, state.current_font_size) {
                        match fonts.get(fid) {
                            Some(ParsedOrBuiltinFont::P(font)) => {
                                // Try to get the CMap from the parsed font
                                find_to_unicode_cmap(font)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                // Decode the TJ array into TextItems
                let text_items = crate::text::decode_tj_operands(arr, to_unicode_cmap.as_ref());

                let default_font = BuiltinOrExternalFontId::Builtin(BuiltinFont::default());
                let d_font = ParsedOrBuiltinFont::B(BuiltinFont::default());
                let cur_font = state.current_font.as_ref().unwrap_or(&default_font);

                match cur_font {
                    BuiltinOrExternalFontId::Builtin(b) => {
                        out_ops.push(Op::WriteTextBuiltinFont {
                            items: text_items,
                            font: b.clone(),
                        });
                    }
                    BuiltinOrExternalFontId::External(id) => {
                        out_ops.push(Op::WriteText {
                            items: text_items,
                            font: id.clone(),
                        });
                    }
                }
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
            } else if let lopdf::Object::String(bytes, format) = &op.operands[0] {
                // Get the font for CMap lookup
                let to_unicode_cmap =
                    if let (Some(fid), _) = (&state.current_font, state.current_font_size) {
                        match fonts.get(fid) {
                            Some(ParsedOrBuiltinFont::P(font)) => {
                                // Try to get the CMap from the parsed font
                                find_to_unicode_cmap_from_font(font)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                // Create a temporary lopdf::Object for decoding
                let string_obj = lopdf::Object::String(bytes.clone(), *format);

                // Decode the PDF string using the CMap if available
                let text_str =
                    crate::text::decode_pdf_string(&string_obj, to_unicode_cmap.as_ref());

                // Create a single TextItem with no kerning
                let text_items = vec![TextItem::Text(text_str)];

                let default_font = BuiltinOrExternalFontId::Builtin(BuiltinFont::default());
                let d_font = ParsedOrBuiltinFont::B(BuiltinFont::default());
                let cur_font = state.current_font.as_ref().unwrap_or(&default_font);

                match cur_font {
                    BuiltinOrExternalFontId::Builtin(b) => {
                        out_ops.push(Op::WriteTextBuiltinFont {
                            items: text_items,
                            font: b.clone(),
                        });
                    }
                    BuiltinOrExternalFontId::External(id) => {
                        out_ops.push(Op::WriteText {
                            items: text_items,
                            font: id.clone(),
                        });
                    }
                }
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
            state.current_font = None;
            state.current_font_size = None;
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
                }
                let size_val = to_f32(&op.operands[1]);
                state.current_font_size = Some(crate::units::Pt(size_val));

                // produce a corresponding printpdf op:
                if let (Some(fid), Some(sz)) = (&state.current_font, &state.current_font_size) {
                    match fid {
                        BuiltinOrExternalFontId::Builtin(builtin_font) => {
                            out_ops.push(Op::SetFontSizeBuiltinFont {
                                size: *sz,
                                font: builtin_font.clone(),
                            });
                        }
                        BuiltinOrExternalFontId::External(font_id) => {
                            out_ops.push(Op::SetFontSize {
                                size: *sz,
                                font: font_id.clone(),
                            });
                        }
                    }
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

        // --- Begin/End layer (BDC/EMC) ---
        "BDC" => {
            // Typically something like: [Name("OC"), Name("MyLayer")]
            if op.operands.len() == 2 {
                if let Some(layer_nm) = as_name(&op.operands[1]) {
                    state.current_layer = Some(layer_nm.clone());
                    out_ops.push(Op::BeginLayer {
                        layer_id: crate::LayerInternalId(layer_nm),
                    });
                }
            }
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
            let layer_nm = match as_name(&op.operands[0]) {
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
            state.current_layer = Some(layer_nm.clone());
            out_ops.push(Op::BeginLayer {
                layer_id: crate::LayerInternalId(layer_nm),
            });
        }
        "EMC" => {
            if let Some(layer_str) = state.current_layer.take() {
                out_ops.push(Op::EndLayer {
                    layer_id: crate::LayerInternalId(layer_str),
                });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: 'EMC' with no current_layer"),
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
        "m" => {
            // Start a new subpath
            if !state.current_subpath.is_empty() {
                // push the old subpath into subpaths
                state
                    .subpaths
                    .push(std::mem::take(&mut state.current_subpath));
            }
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y),
                },
                false,
            ));
            state.current_subpath_closed = false;
        }
        "l" => {
            // lineTo
            let x = to_f32(&op.operands.get(0).unwrap_or(&lopdf::Object::Null));
            let y = to_f32(&op.operands.get(1).unwrap_or(&lopdf::Object::Null));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y),
                },
                false,
            ));
        }
        "c" => {
            // c x1 y1 x2 y2 x3 y3
            // We should already have a current_subpath with at least 1 point.
            if op.operands.len() == 6 {

                // Fix: Ensure we have a starting point before processing a curve
                if state.current_subpath.is_empty() {
                    // PDF spec requires a moveTo before a curveTo, but some generators don't comply
                    // Insert an implicit moveTo at the same position as the first control point
                    let x = to_f32(&op.operands[0]);
                    let y = to_f32(&op.operands[1]);
                    state.current_subpath.push((
                        crate::graphics::Point {
                            x: crate::units::Pt(x),
                            y: crate::units::Pt(y),
                        },
                        false, // Not a control point - it's our implicit starting point
                    ));
                }

                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x2 = to_f32(&op.operands[2]);
                let y2 = to_f32(&op.operands[3]);
                let x3 = to_f32(&op.operands[4]);
                let y3 = to_f32(&op.operands[5]);

                // Append these points to your “current_subpath” in a way that
                // your final geometry code knows it’s a Bézier curve. That might
                // mean storing them in some specialized “CurveTo” variant or
                // tagging them as control points.

                // For example:
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x1),
                        y: Pt(y1),
                    },
                    true, // could mark as "control point"
                ));
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x2),
                        y: Pt(y2),
                    },
                    true,
                ));
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                    false, // endpoint
                ));
            } else {
                // handle error / warning
            }
        }
        "v" => {
            // v x2 y2 x3 y3
            // The first control point is implied to be the current point.
            // So in standard PDF usage:
            //   c (x0,y0) [current point], (x1,y1) [= current point], (x2,y2), (x3,y3)
            if op.operands.len() == 4 {

                // Fix: Ensure we have a starting point before processing a curve
                if state.current_subpath.is_empty() {
                    // PDF spec requires a moveTo before a curveTo, but some generators don't comply
                    // Insert an implicit moveTo at the same position as the first control point
                    let x = to_f32(&op.operands[0]);
                    let y = to_f32(&op.operands[1]);
                    state.current_subpath.push((
                        crate::graphics::Point {
                            x: crate::units::Pt(x),
                            y: crate::units::Pt(y),
                        },
                        false, // Not a control point - it's our implicit starting point
                    ));
                }

                // The "x1,y1" is the current subpath's last point,
                // so we treat that as control-pt #1 implicitly.
                let x2 = to_f32(&op.operands[0]);
                let y2 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);

                // The first control point is the same as the last subpath point:
                // but we still need to mark the next one as a control point:
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x2),
                        y: Pt(y2),
                    },
                    true, // second control
                ));
                // And the final endpoint:
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                    false,
                ));
            } else {
                // handle error
            }
        }
        "y" => {
            // Cubic Bezier: "y" => first control point + final endpoint
            // y x1 y1 x3 y3
            // The second control point is implied to be x3,y3.
            if op.operands.len() == 4 {

                // Fix: Ensure we have a starting point before processing a curve
                if state.current_subpath.is_empty() {
                    // PDF spec requires a moveTo before a curveTo, but some generators don't comply
                    // Insert an implicit moveTo at the same position as the first control point
                    let x = to_f32(&op.operands[0]);
                    let y = to_f32(&op.operands[1]);
                    state.current_subpath.push((
                        crate::graphics::Point {
                            x: crate::units::Pt(x),
                            y: crate::units::Pt(y),
                        },
                        false, // Not a control point - it's our implicit starting point
                    ));
                }

                let x1 = to_f32(&op.operands[0]);
                let y1 = to_f32(&op.operands[1]);
                let x3 = to_f32(&op.operands[2]);
                let y3 = to_f32(&op.operands[3]);

                // The second control point is the same as final endpoint,
                // so we store the first control point explicitly:
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x1),
                        y: Pt(y1),
                    },
                    true, // first control
                ));
                // Then the final endpoint (which is also the second control)
                state.current_subpath.push((
                    crate::graphics::Point {
                        x: Pt(x3),
                        y: Pt(y3),
                    },
                    false,
                ));
            } else {
                // handle error
            }
        }
        "h" => {
            // closepath, i.e. connect last point to first point
            // We'll just mark a flag that we want to close it in fill/stroke
            state.current_subpath_closed = true;
        }
        "re" => {
            if op.operands.len() != 4 {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "re expects 4 operands".to_string(),
                ));
                return Ok(Vec::new());
            }
            let x = to_f32(&op.operands[0]);
            let y = to_f32(&op.operands[1]);
            let w = to_f32(&op.operands[2]);
            let h = to_f32(&op.operands[3]);
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y),
                },
                false,
            ));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x + w),
                    y: crate::units::Pt(y),
                },
                false,
            ));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x + w),
                    y: crate::units::Pt(y + h),
                },
                false,
            ));
            state.current_subpath.push((
                crate::graphics::Point {
                    x: crate::units::Pt(x),
                    y: crate::units::Pt(y + h),
                },
                false,
            ));
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::FillStroke) {
                out_ops.push(op);
            }
        }
        // --- Path painting
        "S" => {
            // Stroke
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::Stroke) {
                out_ops.push(op);
            }
        }
        "f" => {
            // Fill
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::Fill) {
                out_ops.push(op);
            }
        }
        "f*" => {
            // Fill with the even-odd winding rule, no subpath closing
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Fill,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "b" => {
            // Fill + stroke + close the subpath
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::NonZero,
            ) {
                out_ops.push(op);
            }
        }
        "B" => {
            // fill+stroke
            if let Some(op) = finalize_current_path(state, crate::graphics::PaintMode::FillStroke) {
                out_ops.push(op);
            }
        }
        "b*" => {
            // Fill + stroke using even-odd, plus close subpath
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "B*" => {
            // Fill + stroke with even-odd, but subpath is not forcibly closed
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::FillStroke,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }
        "s" => {
            // Stroke path and close it
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Stroke,
                crate::graphics::WindingOrder::NonZero,
            ) {
                out_ops.push(op);
            }
        }
        "n" => {
            state.current_subpath.clear();
            state.subpaths.clear();
            state.current_subpath_closed = false;
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

        "W" => {
            // Clip with non-zero
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Clip,
                crate::graphics::WindingOrder::NonZero,
            ) {
                out_ops.push(op);
            }
        }
        "W*" => {
            // Clip with even-odd
            state.current_subpath_closed = true;
            if let Some(op) = finalize_current_path_special(
                state,
                crate::graphics::PaintMode::Clip,
                crate::graphics::WindingOrder::EvenOdd,
            ) {
                out_ops.push(op);
            }
        }

        // For completeness, you might also parse "cs", "CS" to track the chosen color space
        // or treat them as Unknown if you don't need them:
        "cs" | "CS" => {
            // sets the fill or stroke color space. Usually you'd store in state, or ignore:
            out_ops.push(Op::Unknown {
                key: op.operator.clone(),
                value: op
                    .operands
                    .iter()
                    .map(|s| DictItem::from_lopdf(s))
                    .collect(),
            });
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

/// Try to find or create a ToUnicodeCMap from a ParsedFont
fn find_to_unicode_cmap_from_font(font: &ParsedFont) -> Option<ToUnicodeCMap> {
    // First check if the font has a direct reference to a CMap
    if let Some(cmap_subtable) = &font.cmap_subtable {
        // Convert from OwnedCmapSubtable to ToUnicodeCMap
        let mut mappings = BTreeMap::new();

        // Construct a manual mapping from the CMap subtable data
        for c in 0..65535u32 {
            if let Ok(Some(gid)) = cmap_subtable.map_glyph(c) {
                mappings.insert(gid as u32, vec![c]);
            }
        }

        return Some(ToUnicodeCMap { mappings });
    }

    // If no CMap found in the font, return None
    None
}

/// Helper to decode TJ array contents using CMap if available
fn find_to_unicode_cmap(font: &ParsedFont) -> Option<ToUnicodeCMap> {
    // Fallback: Try to create a ToUnicode CMap from the font's cmap subtable
    if let Some(cmap_subtable) = &font.cmap_subtable {
        let mut mappings = BTreeMap::new();

        // Construct a mapping from the CMap subtable data
        for unicode in 0..65535u32 {
            if let Ok(Some(gid)) = cmap_subtable.map_glyph(unicode) {
                mappings.insert(gid as u32, vec![unicode]);
            }
        }

        return Some(ToUnicodeCMap { mappings });
    }

    None
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
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let author = dict
        .get(b"Author")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let creator = dict
        .get(b"Creator")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let producer = dict
        .get(b"Producer")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let subject = dict
        .get(b"Subject")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let identifier = dict
        .get(b"Identifier")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        })
        .unwrap_or_default();

    let keywords = dict
        .get(b"Keywords")
        .ok()
        .and_then(|obj| match &obj {
            Object::String(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
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

// Helper that finalizes subpaths and sets the specified winding order
fn finalize_current_path_special(
    state: &mut PageState,
    paint_mode: crate::graphics::PaintMode,
    winding: crate::graphics::WindingOrder,
) -> Option<Op> {
    // If there's a partially built subpath, move it into `subpaths`
    if !state.current_subpath.is_empty() {
        let sub = std::mem::take(&mut state.current_subpath);
        state.subpaths.push(sub);
    }
    if state.subpaths.is_empty() {
        state.current_subpath_closed = false;
        return None;
    }

    let rings = std::mem::take(&mut state.subpaths);
    let polygon = crate::graphics::Polygon {
        rings: rings
            .into_iter()
            .map(|r| PolygonRing {
                points: r
                    .into_iter()
                    .map(|lp| LinePoint {
                        p: lp.0,
                        bezier: lp.1,
                    })
                    .collect(),
            })
            .collect(),
        mode: paint_mode,
        winding_order: winding,
    };
    // reset
    state.current_subpath_closed = false;

    Some(Op::DrawPolygon { polygon })
}

// A small helper to produce a final shape if subpaths exist, e.g. on stroke or fill
fn finalize_current_path(
    state: &mut PageState,
    paint_mode: crate::graphics::PaintMode,
) -> Option<Op> {
    if state.subpaths.is_empty() && state.current_subpath.is_empty() {
        return None;
    }
    // If there's a current_subpath not yet appended, push it in
    if !state.current_subpath.is_empty() {
        let sub = std::mem::take(&mut state.current_subpath);
        state.subpaths.push(sub);
    }
    let rings = std::mem::take(&mut state.subpaths);

    let rings = rings
        .into_iter()
        .map(|r| PolygonRing {
            points: r
                .into_iter()
                .map(|lp| LinePoint {
                    p: lp.0,
                    bezier: lp.1,
                })
                .collect::<Vec<_>>(),
        })
        .collect::<Vec<_>>();

    if rings.is_empty() {
        return None;
    }

    // Check if this should be a Line instead of a Polygon:
    // 1. Path is not closed
    // 2. Using Stroke mode
    // 3. Single subpath
    if !state.current_subpath_closed
        && paint_mode == crate::graphics::PaintMode::Stroke
        && rings.len() == 1
    {
        let line = crate::graphics::Line {
            points: rings[0].points.clone(),
            is_closed: false,
        };

        return Some(Op::DrawLine { line });
    }

    let polygon = crate::graphics::Polygon {
        rings,
        mode: paint_mode,
        // For simplicity, we do not handle even-odd fill vs nonzero, etc.
        winding_order: crate::graphics::WindingOrder::NonZero,
    };

    state.current_subpath_closed = false;

    Some(Op::DrawPolygon { polygon })
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
        Ok(crate::graphics::Rect {
            x: crate::units::Pt(x),
            y: crate::units::Pt(y),
            width: crate::units::Pt(width),
            height: crate::units::Pt(height),
        })
    } else {
        Err("Rectangle is not an array".to_string())
    }
}

/// A simple parser for PDF date strings (e.g. "D:20170505150224+02'00'")
fn parse_pdf_date(s: &str) -> Result<OffsetDateTime, String> {
    // Remove a leading "D:" if present.
    let s = if s.starts_with("D:") { &s[2..] } else { s };
    if s.len() < 14 {
        return Err("Date string too short".to_string());
    }
    let year: i32 = s[0..4].parse::<i32>().map_err(|e| e.to_string())?;
    let month: u8 = s[4..6].parse::<u8>().map_err(|e| e.to_string())?;
    let day: u8 = s[6..8].parse::<u8>().map_err(|e| e.to_string())?;
    let hour: u8 = s[8..10].parse::<u8>().map_err(|e| e.to_string())?;
    let minute: u8 = s[10..12].parse::<u8>().map_err(|e| e.to_string())?;
    let second: u8 = s[12..14].parse::<u8>().map_err(|e| e.to_string())?;
    let month = match month {
        1 => time::Month::January,
        2 => time::Month::February,
        3 => time::Month::March,
        4 => time::Month::April,
        5 => time::Month::May,
        6 => time::Month::June,
        7 => time::Month::July,
        8 => time::Month::August,
        9 => time::Month::September,
        10 => time::Month::October,
        11 => time::Month::November,
        12 => time::Month::December,
        _ => time::Month::January,
    };

    #[cfg(target_arch = "wasm32")]
    {
        Ok(OffsetDateTime::from_unix_timestamp(0).unwrap())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(OffsetDateTime::new_in_offset(
            time::Date::from_calendar_date(year, month, day).map_err(|e| e.to_string())?,
            time::Time::from_hms(hour, minute, second).map_err(|e| e.to_string())?,
            UtcOffset::from_hms(0, 0, 0).map_err(|e| e.to_string())?,
        ))
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
        Pt,
        annotation::{
            Actions, BorderArray, ColorArray, DashPhase, Destination, HighlightingMode,
            LinkAnnotation, PageAnnotation,
        },
        graphics::Rect,
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
                name: String::from_utf8_lossy(&title).to_string(), // TODO: better parsing!
                page: page_num,
            });

            // Move on to the next bookmark.
            current_ref = next_bookmark(bm_dict);
        }
        Some(bookmarks)
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
                            let rect = Rect {
                                x: Pt(coords[0]),
                                y: Pt(coords[1]),
                                width: Pt(coords[2]),
                                height: Pt(coords[3]),
                            };

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
        BuiltinFont, BuiltinOrExternalFontId, FontId,
        graphics::{
            BlendMode, ChangedField, ExtendedGraphicsState, LineCapStyle, LineDashPattern,
            LineJoinStyle, OverprintMode, RenderingIntent,
        },
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
