//! deserialize.rs
//!
//! This module implements parsing of a PDF (using lopdf) and converting it into a
//! printpdf::PdfDocument. In particular, it decompresses the content streams and then
//! converts lopdf operations to printpdf Ops.

use std::collections::BTreeMap;
use std::sync::Arc;

use lopdf::{
    Dictionary as LopdfDictionary, Document as LopdfDocument, Object as LopdfObject, Object,
    ObjectId, Stream as LopdfStream,
};
use serde_derive::{Deserialize, Serialize};

use crate::{
    font::ParsedFont,
    xobject::{ExternalStream, ExternalXObject},
    BuiltinFont, BuiltinOrExternalFontId, Color, DictItem, ExtendedGraphicsState, ExtendedGraphicsStateId, ExtendedGraphicsStateMap, FontId, LayerInternalId, Line, LineDashPattern, LinePoint, LinkAnnotation, Op, PageAnnotId, PageAnnotMap, PaintMode, PdfDocument, PdfDocumentInfo, PdfFontMap, PdfLayerMap, PdfMetadata, PdfPage, PdfResources, Point, Polygon, PolygonRing, Pt, RawImage, Rect, RenderingIntent, TextMatrix, TextRenderingMode, WindingOrder, XObject, XObjectId, XObjectMap, cmap::ToUnicodeCMap, conformance::PdfConformance, date::{OffsetDateTime, parse_pdf_date}
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
    /// Link annotations with the 0-based index of the page they belong to.
    link_annots: Vec<(usize, LinkAnnotation)>,
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

    // The Pages entry should be an indirect reference, but tolerate an inline
    // dictionary too (seen in the wild).
    let pages_dict = match pages_obj {
        Object::Reference(r) => doc
            .get_object(*r)
            .map_err(|e| format!("Failed to get Pages object: {}", e))?
            .as_dict()
            .map_err(|e| format!("Pages object is not a dictionary: {}", e))?,
        Object::Dictionary(d) => {
            warnings.push(PdfWarnMsg::warning(
                0,
                0,
                "Catalog /Pages is an inline dictionary instead of a reference".to_string(),
            ));
            d
        }
        _ => return Err("Pages key is not a reference".to_string()),
    };

    // Check if Pages tree has Resources dictionary and include it
    if let Ok(resources) = pages_dict.get(b"Resources") {
        objs_to_search_for_resources.push((resources.clone(), None));
    }

    // Recursively collect all page object references.
    let page_refs = collect_page_refs(pages_dict, &doc, warnings)?;

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
        link_annots,
    } = initial_pdf;

    // Build the per-resource-name text decode map used by the content stream
    // parser. Keys are the resource names that `Tf` operators reference.
    let font_decode: BTreeMap<String, FontTextDecodeKind> = fonts
        .iter()
        .filter_map(|(id, pf)| {
            let name = match id {
                BuiltinOrExternalFontId::External(fid) => fid.0.clone(),
                // Builtin entries are keyed by their enum, not by a resource
                // name from this document; `Tf` falls back to
                // `BuiltinFont::from_id` for the F1-F14 naming convention.
                BuiltinOrExternalFontId::Builtin(_) => return None,
            };
            let kind = match pf {
                ParsedOrBuiltinFont::B(b) => FontTextDecodeKind::Builtin(*b),
                ParsedOrBuiltinFont::P(p) => {
                    if p.is_cid {
                        FontTextDecodeKind::Cid {
                            to_unicode: p.to_unicode.clone(),
                            cid_to_gid: p.cid_to_gid.clone(),
                        }
                    } else {
                        FontTextDecodeKind::Simple {
                            to_unicode: p.to_unicode.clone(),
                        }
                    }
                }
                ParsedOrBuiltinFont::CidStub(to_unicode) => FontTextDecodeKind::Cid {
                    to_unicode: to_unicode.clone(),
                    // No parsable font program — no charset / CIDToGIDMap to
                    // resolve through; the identity assumption is all there is.
                    cid_to_gid: None,
                },
            };
            Some((name, kind))
        })
        .collect();

    for (i, page_ref) in page_refs.into_iter().enumerate() {
        let page_obj = doc
            .get_object(page_ref)
            .map_err(|e| format!("Failed to get page object: {}", e))?
            .as_dict()
            .map_err(|e| format!("Page object is not a dictionary: {}", e))?;

        let pdf_page = parse_page(i, page_obj, &doc, &xobjects, &font_decode, warnings)?;

        pages.push(pdf_page);
    }

    // Re-attach link annotations to their pages: the serializer builds each
    // page's /Annots from `Op::LinkAnnotation`, so without this every link is
    // silently dropped on re-save.
    for (page_idx, link) in link_annots {
        match pages.get_mut(page_idx) {
            Some(page) => page.ops.push(Op::LinkAnnotation { link }),
            None => warnings.push(PdfWarnMsg::warning(
                page_idx,
                0,
                "link annotation references a page that was not parsed".to_string(),
            )),
        }
    }

    // Extract bookmarks and layers from the document
    let bookmarks = links_and_bookmarks::extract_bookmarks(&doc);
    let layers = layers::extract_layers(&doc);
    // Optional-content ops (`BDC /OC /Name`) reference layers by the *resource
    // name* under the page's `/Properties` dictionary. The layer map must be
    // keyed by those same names, otherwise every re-saved `BDC` points at a
    // property that no longer exists and viewers report
    // "Marked Content 'X' is unknown" for every layer.
    let layers: Vec<(LayerInternalId, crate::ops::Layer)> = layers
        .into_iter()
        .enumerate()
        .map(|(i, (name, layer))| {
            let key = name.unwrap_or_else(|| format!("layer_{}", i));
            (LayerInternalId(key), layer)
        })
        .collect();

    // Extract ExtGStates from resources
    let extgstates = extract_extgstates(&doc, &objs_to_search_for_resources, warnings);

    // Extract /Shading resources (gradients) — see extract_shadings for scope.
    let shadings = extract_shadings(&doc, &objs_to_search_for_resources, warnings);

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
            shadings: crate::ShadingMap { map: shadings },
            layers: PdfLayerMap {
                map: layers.into_iter().collect(),
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

/// An external font parsed out of the PDF, together with everything the
/// content-stream text decoder needs to know about it.
pub struct ParsedExternalFont {
    pub font: ParsedFont,
    /// The font's ToUnicode CMap (parsed, or synthesized from the font's cmap
    /// table when the PDF has none). Keyed by content-stream code: CID for
    /// composite fonts, single byte for simple fonts.
    pub to_unicode: Option<Arc<ToUnicodeCMap>>,
    /// `true` when the PDF font dictionary was a `/Subtype /Type0` (composite)
    /// font: text-showing operators carry 2-byte big-endian codes. `false` for
    /// simple fonts (TrueType/Type1): one byte per code.
    pub is_cid: bool,
    /// CID -> glyph id, for composite fonts where the two are NOT the same:
    /// a `/CIDToGIDMap` stream (CIDFontType2), or the CFF charset of a CID-keyed
    /// CIDFontType0 program (ISO 32000-1, 9.7.4.2). `None` means CID == GID
    /// (Identity). Without this, every code was stored as if it were a glyph id,
    /// so PDFs from spec-following producers (Adobe, dvipdfmx — and printpdf
    /// itself since the #280 fix) re-rendered with wrong outlines and widths.
    pub cid_to_gid: Option<Arc<BTreeMap<u16, u16>>>,
}

pub enum ParsedOrBuiltinFont {
    P(ParsedExternalFont),
    B(BuiltinFont),
    /// A composite (Type0) font whose font program could not be extracted or
    /// parsed (e.g. not embedded). It cannot be re-embedded, but its 2-byte
    /// code layout and ToUnicode CMap are kept so the text still *decodes*
    /// correctly for extraction.
    CidStub(Option<Arc<ToUnicodeCMap>>),
}

impl ParsedOrBuiltinFont {
    fn as_parsed_font(self) -> Option<ParsedFont> {
        match self {
            ParsedOrBuiltinFont::P(p) => Some(p.font),
            ParsedOrBuiltinFont::B(_) => None,
            ParsedOrBuiltinFont::CidStub(_) => None,
        }
    }

    #[allow(dead_code)]
    fn cmap(&self) -> Option<&ToUnicodeCMap> {
        match self {
            ParsedOrBuiltinFont::P(p) => p.to_unicode.as_deref(),
            ParsedOrBuiltinFont::B(_) => None,
            ParsedOrBuiltinFont::CidStub(c) => c.as_deref(),
        }
    }
}

/// How the bytes of text-showing operators (`Tj`, `TJ`, `'`, `"`) must be
/// decoded for one font resource. Resolved once per page from the parsed font
/// resources, then tracked across `Tf` operators in [`PageState`].
///
/// Getting this wrong garbles text: the pre-0.12 parser guessed the code width
/// from the *byte length parity* of each string, so `(Hi) Tj` in Helvetica
/// (2 bytes, even) was decoded as one 16-bit CID 0x4869 instead of "Hi".
#[derive(Debug, Clone)]
pub enum FontTextDecodeKind {
    /// One of the 14 standard fonts: one-byte WinAnsi codes.
    Builtin(BuiltinFont),
    /// Simple external font (TrueType/Type1): one-byte codes, decoded through
    /// the ToUnicode CMap when present, WinAnsi otherwise.
    Simple {
        to_unicode: Option<Arc<ToUnicodeCMap>>,
    },
    /// Composite (Type0) font: two-byte big-endian CIDs. Under the Identity-H
    /// encoding the code IS the CID — but the CID is only equal to the glyph id
    /// when the descendant font says so: `cid_to_gid` carries the `/CIDToGIDMap`
    /// stream (CIDFontType2) or CID-keyed CFF charset (CIDFontType0) mapping
    /// when the two differ.
    Cid {
        to_unicode: Option<Arc<ToUnicodeCMap>>,
        cid_to_gid: Option<Arc<BTreeMap<u16, u16>>>,
    },
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
        // #216: XObjects are stream objects; accept a stream and return its dict so
        // that XObject stream refs (e.g. Form/Image XObjects) parse instead of being
        // rejected as an "unexpected type".
        Object::Stream(stream) => Some(&stream.dict),
        Object::Reference(r) => match doc.get_dictionary(*r) {
            Ok(s) => Some(s),
            // The reference may point at a *stream* object (the common case for
            // XObjects), which `get_dictionary` rejects. Resolve the object and
            // return the stream's dict before treating it as an error.
            Err(_) => match doc.get_object(*r) {
                Ok(Object::Stream(stream)) => Some(&stream.dict),
                Ok(Object::Dictionary(dict)) => Some(dict),
                _ => {
                    warnings.push(PdfWarnMsg::error(
                        page.unwrap_or(0),
                        0,
                        format!("{id}: Invalid dictionary reference {r:?}"),
                    ));
                    return None;
                }
            },
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

/// Intermediate result of scanning the XObject resources: either an encoded
/// image that still needs to go through the (possibly async) image decoder, or
/// an XObject that is already fully represented.
pub(crate) enum PendingXObject {
    /// An image in a self-describing container format (JPEG via `DCTDecode`,
    /// JPEG2000 via `JPXDecode`, ...). `fallback` preserves the original
    /// stream verbatim in case the decoder does not understand the bytes, so
    /// that re-saving the document never silently drops the image (#216).
    DecodeImage {
        bytes: Vec<u8>,
        smask: Option<SMaskData>,
        fallback: ExternalXObject,
    },
    /// Fully parsed (raw bitmaps we can construct directly, preserved Form
    /// XObjects, unknown subtypes kept verbatim, ...).
    Ready(XObject),
}

/// A decoded (uncompressed) SMask alpha channel: 8-bit gray, `width * height` bytes.
pub(crate) struct SMaskData {
    pub width: usize,
    pub height: usize,
    pub gray: Vec<u8>,
}

/// Merge an 8-bit gray SMask into a decoded [`RawImage`], turning RGB8 into
/// RGBA8 and R8 into RG8 (gray+alpha). Returns the image unchanged when the
/// dimensions do not match or the pixel format has no 8-bit alpha companion.
fn apply_smask_to_rawimage(mut img: RawImage, smask: SMaskData) -> RawImage {
    use crate::image_types::{RawImageData, RawImageFormat};
    if smask.width != img.width || smask.height != img.height {
        return img;
    }
    let expected = img.width * img.height;
    if smask.gray.len() < expected {
        return img;
    }
    match (&img.data_format, &img.pixels) {
        (RawImageFormat::RGB8, RawImageData::U8(rgb)) => {
            if rgb.len() < expected * 3 {
                return img;
            }
            let mut rgba = Vec::with_capacity(expected * 4);
            for i in 0..expected {
                rgba.extend_from_slice(&rgb[i * 3..i * 3 + 3]);
                rgba.push(smask.gray[i]);
            }
            img.pixels = RawImageData::U8(rgba);
            img.data_format = RawImageFormat::RGBA8;
            img
        }
        (RawImageFormat::R8, RawImageData::U8(gray)) => {
            if gray.len() < expected {
                return img;
            }
            let mut ga = Vec::with_capacity(expected * 2);
            for i in 0..expected {
                ga.push(gray[i]);
                ga.push(smask.gray[i]);
            }
            img.pixels = RawImageData::U8(ga);
            img.data_format = RawImageFormat::RG8;
            img
        }
        _ => img,
    }
}

fn process_xobjects(
    xobjects: BTreeMap<XObjectId, PendingXObject>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<XObjectId, XObject> {
    let mut map = BTreeMap::new();
    for (xobject_id, pending) in xobjects {
        match pending {
            PendingXObject::Ready(x) => {
                map.insert(xobject_id, x);
            }
            PendingXObject::DecodeImage {
                bytes,
                smask,
                fallback,
            } => {
                warnings.push(PdfWarnMsg::info(
                    0,
                    0,
                    format!("process XObject {} ({} bytes)", xobject_id.0, bytes.len()),
                ));
                match RawImage::decode_from_bytes(&bytes, warnings) {
                    Ok(mut o) => {
                        if let Some(sm) = smask {
                            o = apply_smask_to_rawimage(o, sm);
                        }
                        map.insert(xobject_id, XObject::Image(o));
                    }
                    Err(e) => {
                        warnings.push(PdfWarnMsg::warning(
                            0,
                            0,
                            format!(
                                "could not decode image XObject {} ({} bytes): {e}; preserving \
                                 original stream",
                                xobject_id.0,
                                bytes.len()
                            ),
                        ));
                        map.insert(xobject_id, XObject::External(fallback));
                    }
                }
            }
        }
    }
    map
}

/// Extract `/Shading` resources (axial/radial gradients) into the typed
/// [`crate::Shading`] model. Shading types other than 2 (axial) and 3 (radial),
/// and function types other than 2 (exponential) / 3 (stitching), are reported
/// as warnings and skipped — those are exactly the shapes printpdf itself
/// writes, so its own gradients round-trip. Before this existed, `/Shading`
/// resources were dropped entirely and every `sh` op became `Op::Unknown`,
/// which the default (`secure`) save then deleted — gradients silently
/// vanished on re-save.
fn extract_shadings(
    doc: &LopdfDocument,
    objs_to_search_for_resources: &[(LopdfObject, Option<usize>)],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<crate::ShadingId, crate::Shading> {
    let mut map = BTreeMap::new();
    for (obj, page_idx) in objs_to_search_for_resources {
        let page_num = page_idx.unwrap_or(0);
        let dict = match obj {
            LopdfObject::Dictionary(d) => Some(d),
            LopdfObject::Reference(r) => doc.get_dictionary(*r).ok(),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        let resources = if let Ok(res) = dict.get(b"Resources") {
            get_dict_or_resolve_ref(
                &format!("extract_shadings Resources page {page_num}"),
                doc,
                res,
                warnings,
                Some(page_num),
            )
        } else {
            Some(dict)
        };
        let Some(resources) = resources else { continue };
        let Ok(shading_res) = resources.get(b"Shading") else { continue };
        let Some(shading_dict) = get_dict_or_resolve_ref(
            &format!("extract_shadings Shading page {page_num}"),
            doc,
            shading_res,
            warnings,
            Some(page_num),
        ) else {
            continue;
        };
        for (key, value) in shading_dict.iter() {
            let id = crate::ShadingId(String::from_utf8_lossy(key).to_string());
            if map.contains_key(&id) {
                continue; // first definition wins, consistent with other resources
            }
            let Some(sh) = get_dict_or_resolve_ref(
                &format!("extract_shadings entry page {page_num}"),
                doc,
                value,
                warnings,
                Some(page_num),
            ) else {
                continue;
            };
            match parse_shading_dict(doc, sh) {
                Ok(parsed) => {
                    map.insert(id, parsed);
                }
                Err(e) => warnings.push(PdfWarnMsg::warning(
                    page_num,
                    0,
                    format!("shading {:?} not preserved: {e}", id.0),
                )),
            }
        }
    }
    map
}

/// Translate a ShadingType 2/3 dictionary (with an exponential or stitching
/// function) into [`crate::Shading`] — the exact inverse of `Shading::to_dict`.
fn parse_shading_dict(
    doc: &LopdfDocument,
    dict: &LopdfDictionary,
) -> Result<crate::Shading, String> {
    fn as_f32(o: &LopdfObject) -> Option<f32> {
        match o {
            LopdfObject::Integer(i) => Some(*i as f32),
            LopdfObject::Real(r) => Some(*r),
            _ => None,
        }
    }
    fn num_array(doc: &LopdfDocument, o: &LopdfObject, n: usize) -> Option<Vec<f32>> {
        let o = match o {
            LopdfObject::Reference(r) => doc.get_object(*r).ok()?,
            other => other,
        };
        let arr = o.as_array().ok()?;
        let v: Vec<f32> = arr.iter().filter_map(as_f32).collect();
        (v.len() == n).then_some(v)
    }
    fn rgb(v: &[f32]) -> [f32; 3] {
        match v.len() {
            1 => [v[0], v[0], v[0]], // DeviceGray
            3 => [v[0], v[1], v[2]],
            _ => [0.0, 0.0, 0.0],
        }
    }
    fn resolve<'a>(doc: &'a LopdfDocument, o: &'a LopdfObject) -> &'a LopdfObject {
        match o {
            LopdfObject::Reference(r) => doc.get_object(*r).unwrap_or(o),
            other => other,
        }
    }
    fn fn_dict<'a>(o: &'a LopdfObject) -> Option<&'a LopdfDictionary> {
        match o {
            LopdfObject::Dictionary(d) => Some(d),
            LopdfObject::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }

    let shading_type = dict
        .get(b"ShadingType")
        .ok()
        .and_then(as_f32)
        .ok_or("missing ShadingType")? as i64;

    let geometry = match shading_type {
        2 => {
            let c = dict
                .get(b"Coords")
                .ok()
                .and_then(|o| num_array(doc, o, 4))
                .ok_or("axial shading without 4-element Coords")?;
            crate::ShadingGeometry::Axial {
                coords: [c[0], c[1], c[2], c[3]],
            }
        }
        3 => {
            let c = dict
                .get(b"Coords")
                .ok()
                .and_then(|o| num_array(doc, o, 6))
                .ok_or("radial shading without 6-element Coords")?;
            crate::ShadingGeometry::Radial {
                coords: [c[0], c[1], c[2], c[3], c[4], c[5]],
            }
        }
        other => return Err(format!("unsupported ShadingType {other}")),
    };

    let extend = dict
        .get(b"Extend")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            let b = |i: usize| matches!(a.get(i), Some(LopdfObject::Boolean(true)));
            (b(0), b(1))
        })
        .unwrap_or((false, false));

    let func_obj = resolve(
        doc,
        dict.get(b"Function").map_err(|_| "shading without Function")?,
    );
    let func = fn_dict(func_obj).ok_or("shading Function is not a dictionary/stream")?;
    let func_type = func
        .get(b"FunctionType")
        .ok()
        .and_then(as_f32)
        .ok_or("Function without FunctionType")? as i64;

    // One exponential segment over [t0, t1] → two stops.
    fn exponential_stops(
        doc: &LopdfDocument,
        f: &LopdfDictionary,
        t0: f32,
        t1: f32,
    ) -> Result<Vec<crate::GradientStop>, String> {
        let get = |k: &[u8]| -> Vec<f32> {
            f.get(k)
                .ok()
                .and_then(|o| {
                    let o = match o {
                        LopdfObject::Reference(r) => doc.get_object(*r).ok()?,
                        other => other,
                    };
                    o.as_array().ok()
                })
                .map(|a| {
                    a.iter()
                        .filter_map(|x| match x {
                            LopdfObject::Integer(i) => Some(*i as f32),
                            LopdfObject::Real(r) => Some(*r),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![0.0])
        };
        let c0 = get(b"C0");
        let c1 = get(b"C1");
        Ok(vec![
            crate::GradientStop {
                offset: t0,
                color: rgb(&c0),
            },
            crate::GradientStop {
                offset: t1,
                color: rgb(&c1),
            },
        ])
    }

    let mut stops = match func_type {
        2 => exponential_stops(doc, func, 0.0, 1.0)?,
        3 => {
            let bounds = func
                .get(b"Bounds")
                .ok()
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_array().ok())
                .map(|a| a.iter().filter_map(as_f32).collect::<Vec<_>>())
                .unwrap_or_default();
            let funcs = func
                .get(b"Functions")
                .ok()
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_array().ok())
                .ok_or("stitching function without Functions array")?;
            let mut edges = Vec::with_capacity(funcs.len() + 1);
            edges.push(0.0);
            edges.extend(bounds.iter().copied());
            edges.push(1.0);
            let mut all = Vec::new();
            for (i, sub) in funcs.iter().enumerate() {
                let sub = resolve(doc, sub);
                let sub = fn_dict(sub).ok_or("stitching sub-function is not a dictionary")?;
                let (t0, t1) = (
                    edges.get(i).copied().unwrap_or(0.0),
                    edges.get(i + 1).copied().unwrap_or(1.0),
                );
                all.extend(exponential_stops(doc, sub, t0, t1)?);
            }
            all
        }
        other => return Err(format!("unsupported FunctionType {other}")),
    };

    // Merge duplicate boundary stops (segment N's end == segment N+1's start).
    stops.dedup_by(|b, a| (a.offset - b.offset).abs() < 1e-6 && a.color == b.color);
    if stops.is_empty() {
        return Err("shading function produced no stops".to_string());
    }

    Ok(crate::Shading {
        geometry,
        stops,
        extend,
    })
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
    xobjects: BTreeMap<XObjectId, PendingXObject>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<XObjectId, XObject> {
    let mut map = BTreeMap::new();
    for (xobject_id, pending) in xobjects {
        match pending {
            PendingXObject::Ready(x) => {
                map.insert(xobject_id, x);
            }
            PendingXObject::DecodeImage {
                bytes,
                smask,
                fallback,
            } => match RawImage::decode_from_bytes_async(&bytes, warnings).await {
                Ok(mut o) => {
                    if let Some(sm) = smask {
                        o = apply_smask_to_rawimage(o, sm);
                    }
                    map.insert(xobject_id, XObject::Image(o));
                }
                Err(e) => {
                    warnings.push(PdfWarnMsg::warning(
                        0,
                        0,
                        format!(
                            "could not decode image XObject {}: {e}; preserving original stream",
                            xobject_id.0
                        ),
                    ));
                    map.insert(xobject_id, XObject::External(fallback));
                }
            },
        }
    }
    map
}

/// Maximum recursion depth (container nesting + reference hops) when inlining
/// indirect references into a preserved XObject dictionary. Real-world SVG
/// conversions nest forms → resources → patterns → shadings → functions →
/// ICC streams, so this must be generous; actual cycles are caught by the
/// reference path stack, not the depth cap.
const MAX_RESOLVE_DEPTH: usize = 96;

/// Convert a lopdf object into a [`DictItem`], resolving indirect references
/// by *inlining* the referenced objects.
///
/// Preserved XObjects are carried across the parse/save round trip as
/// [`ExternalXObject`]s. Their dictionaries routinely reference other objects
/// (a Form XObject's `/Resources`, pattern functions, ICC profiles, ...) — but
/// object ids are meaningless in the re-saved document, so a verbatim
/// `Reference` would dangle. Inlining the referenced object keeps the XObject
/// self-contained; `into_lopdf_with_indirect_streams` re-hoists nested streams
/// into indirect objects at save time.
///
/// `ref_path` holds the reference ids currently being resolved (the *path*,
/// not all visited ids — legitimate documents share objects diamond-style);
/// re-entering an id on the same path is a cycle and yields `Null`.
fn dictitem_from_lopdf_resolved(
    doc: &LopdfDocument,
    obj: &LopdfObject,
    depth: usize,
    ref_path: &mut Vec<ObjectId>,
) -> DictItem {
    if depth > MAX_RESOLVE_DEPTH {
        return DictItem::Null;
    }
    match obj {
        Object::Reference(r) => {
            if ref_path.contains(r) {
                // reference cycle
                return DictItem::Null;
            }
            match doc.get_object(*r) {
                Ok(target) => {
                    ref_path.push(*r);
                    let item = dictitem_from_lopdf_resolved(doc, target, depth + 1, ref_path);
                    ref_path.pop();
                    item
                }
                Err(_) => DictItem::Null,
            }
        }
        Object::Array(objects) => DictItem::Array(
            objects
                .iter()
                .map(|o| dictitem_from_lopdf_resolved(doc, o, depth + 1, ref_path))
                .collect(),
        ),
        Object::Dictionary(dict) => DictItem::Dict {
            map: dict
                .iter()
                .map(|(k, v)| {
                    (
                        String::from_utf8_lossy(k).to_string(),
                        dictitem_from_lopdf_resolved(doc, v, depth + 1, ref_path),
                    )
                })
                .collect(),
        },
        Object::Stream(stream) => DictItem::Stream {
            stream: external_stream_preserved(doc, stream, depth + 1, ref_path),
        },
        other => DictItem::from_lopdf(other),
    }
}

/// Preserve a lopdf stream verbatim as an [`ExternalStream`]: the dictionary
/// with references inlined (minus `/Length`, which is recomputed at save time)
/// and the *raw, still-encoded* content, so existing `/Filter` entries remain
/// valid.
fn external_stream_preserved(
    doc: &LopdfDocument,
    stream: &LopdfStream,
    depth: usize,
    ref_path: &mut Vec<ObjectId>,
) -> ExternalStream {
    let dict = stream
        .dict
        .iter()
        .filter(|(k, _)| k.as_slice() != b"Length")
        .map(|(k, v)| {
            (
                String::from_utf8_lossy(k).to_string(),
                dictitem_from_lopdf_resolved(doc, v, depth, ref_path),
            )
        })
        .collect();
    ExternalStream {
        dict,
        content: stream.content.clone(),
        // The content is already encoded exactly as the preserved /Filter
        // describes; re-compressing it would corrupt the stream.
        compress: false,
    }
}

/// Preserve an XObject stream verbatim for the round trip.
fn preserve_xobject_stream(doc: &LopdfDocument, stream: &LopdfStream) -> ExternalXObject {
    ExternalXObject {
        stream: external_stream_preserved(doc, stream, 0, &mut Vec::new()),
        // Leave width/height/dpi unset: placement comes from the content
        // stream's own `cm` + the preserved `/BBox`/`/Matrix`, and a `Some`
        // width/height would make the serializer inject an extra scale.
        width: None,
        height: None,
        dpi: None,
    }
}

/// Number of color components for a (resolved) `/ColorSpace` entry, for the
/// raw-bitmap path. Returns `None` for colorspaces that `RawImage` cannot
/// represent losslessly (Indexed, Separation, DeviceN, ...).
fn colorspace_components(doc: &LopdfDocument, cs: &LopdfObject, depth: usize) -> Option<usize> {
    if depth > MAX_RESOLVE_DEPTH {
        return None;
    }
    match cs {
        Object::Reference(r) => colorspace_components(doc, doc.get_object(*r).ok()?, depth + 1),
        Object::Name(n) => match n.as_slice() {
            b"DeviceRGB" | b"CalRGB" => Some(3),
            b"DeviceGray" | b"CalGray" => Some(1),
            _ => None,
        },
        Object::Array(arr) => {
            let first = arr.first()?;
            let family = match first {
                Object::Name(n) => n.as_slice(),
                _ => return None,
            };
            match family {
                b"ICCBased" => {
                    let stream = arr.get(1)?;
                    let stream = match stream {
                        Object::Reference(r) => match doc.get_object(*r).ok()? {
                            Object::Stream(s) => s,
                            _ => return None,
                        },
                        Object::Stream(s) => s,
                        _ => return None,
                    };
                    match stream.dict.get(b"N").ok()? {
                        Object::Integer(1) => Some(1),
                        Object::Integer(3) => Some(3),
                        _ => None,
                    }
                }
                b"CalRGB" => Some(3),
                b"CalGray" => Some(1),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The `/Filter` chain of a stream as a list of names.
fn stream_filters(stream: &LopdfStream) -> Vec<Vec<u8>> {
    match stream.dict.get(b"Filter") {
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(arr)) => arr
            .iter()
            .filter_map(|o| match o {
                Object::Name(n) => Some(n.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Resolve and decode the `/SMask` of an image stream to 8-bit gray alpha.
fn extract_smask(
    doc: &LopdfDocument,
    stream: &LopdfStream,
) -> Option<SMaskData> {
    let smask_obj = stream.dict.get(b"SMask").ok()?;
    let smask_stream = match smask_obj {
        Object::Reference(r) => match doc.get_object(*r).ok()? {
            Object::Stream(s) => s,
            _ => return None,
        },
        Object::Stream(s) => s,
        _ => return None,
    };
    let width = smask_stream.dict.get(b"Width").ok()?.as_i64().ok()? as usize;
    let height = smask_stream.dict.get(b"Height").ok()?.as_i64().ok()? as usize;
    let bpc = smask_stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(8);
    if bpc != 8 {
        return None;
    }
    let gray = smask_stream
        .decompressed_content()
        .unwrap_or_else(|_| smask_stream.content.clone());
    if gray.len() < width * height {
        return None;
    }
    Some(SMaskData {
        width,
        height,
        gray,
    })
}

/// Try to construct a [`RawImage`] directly from a raw-bitmap image stream
/// (no filter, or a lossless filter lopdf can undo). This is how printpdf
/// itself writes images by default (`FlateDecode` over raw pixels), so this
/// path is what makes printpdf's own output round-trippable: the pixel data
/// carries no container magic bytes, so `RawImage::decode_from_bytes` (which
/// sniffs PNG/JPEG/... signatures) can never decode it.
fn raw_bitmap_from_stream(
    doc: &LopdfDocument,
    stream: &LopdfStream,
) -> Option<RawImage> {
    use crate::image_types::{RawImageData, RawImageFormat};

    let width = stream.dict.get(b"Width").ok()?.as_i64().ok()? as usize;
    let height = stream.dict.get(b"Height").ok()?.as_i64().ok()? as usize;
    if width == 0 || height == 0 || width.checked_mul(height).is_none() {
        return None;
    }
    let bpc = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(8);
    let components = colorspace_components(doc, stream.dict.get(b"ColorSpace").ok()?, 0)?;
    // /ImageMask stencils and predictor-encoded data have extra semantics the
    // raw path does not model; preserve those streams instead.
    if let Ok(im) = stream.dict.get(b"ImageMask") {
        if im.as_bool().unwrap_or(false) {
            return None;
        }
    }
    if let Ok(Object::Dictionary(dp)) = stream.dict.get(b"DecodeParms") {
        if dp.get(b"Predictor").map(|p| p.as_i64().unwrap_or(1)).unwrap_or(1) > 1 {
            return None;
        }
    }

    let data = stream.decompressed_content().ok()?;
    let expected = width * height * components * (bpc as usize / 8).max(1);

    match (bpc, components) {
        (8, 3) | (8, 1) => {
            if data.len() < expected {
                return None;
            }
            let format = if components == 3 {
                RawImageFormat::RGB8
            } else {
                RawImageFormat::R8
            };
            let mut pixels = data;
            pixels.truncate(expected);
            let mut img = RawImage {
                pixels: RawImageData::U8(pixels),
                width,
                height,
                data_format: format,
                tag: Vec::new(),
            };
            if let Some(smask) = extract_smask(doc, stream) {
                img = apply_smask_to_rawimage(img, smask);
            }
            Some(img)
        }
        (16, 3) | (16, 1) => {
            if data.len() < expected {
                return None;
            }
            let format = if components == 3 {
                RawImageFormat::RGB16
            } else {
                RawImageFormat::R16
            };
            let pixels: Vec<u16> = data[..expected]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            Some(RawImage {
                pixels: RawImageData::U16(pixels),
                width,
                height,
                data_format: format,
                tag: Vec::new(),
            })
        }
        _ => None,
    }
}

/// Given a Resources dictionary from a page or the document, parse all
/// XObjects. Image XObjects are decoded (or queued for decoding); everything
/// else — Form XObjects, unsupported filters/colorspaces — is preserved
/// verbatim so that a parse/save round trip never drops page content (#216).
fn parse_xobjects_internal(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
    warnings: &mut Vec<PdfWarnMsg>,
    page: Option<usize>,
) -> BTreeMap<XObjectId, PendingXObject> {
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
        let xobject_id = XObjectId(String::from_utf8_lossy(key).to_string());

        // Every XObject is a stream object.
        let stream = match get_stream_or_resolve_ref(doc, value, warnings, page) {
            Some(s) => s,
            None => continue,
        };

        let subtype = match stream.dict.get(b"Subtype") {
            Ok(Object::Name(o)) => o.clone(),
            _ => {
                // Missing /Subtype: sniff — image streams have /Width + /Height.
                if stream.dict.get(b"Width").is_ok() && stream.dict.get(b"Height").is_ok() {
                    b"Image".to_vec()
                } else {
                    warnings.push(PdfWarnMsg::warning(
                        page_num,
                        0,
                        format!(
                            "parse-xobjects: XObject {} has no /Subtype; preserving verbatim",
                            xobject_id.0
                        ),
                    ));
                    xobj_map.insert(
                        xobject_id,
                        PendingXObject::Ready(XObject::External(preserve_xobject_stream(
                            doc, stream,
                        ))),
                    );
                    continue;
                }
            }
        };

        match subtype.as_slice() {
            b"Image" => {
                let filters = stream_filters(stream);
                let has_container_format = filters
                    .iter()
                    .any(|f| f == b"DCTDecode" || f == b"JPXDecode");

                if has_container_format {
                    // JPEG / JPEG2000: the (possibly Flate-wrapped) stream
                    // content *is* the container file.
                    let bytes = if filters.len() == 1 {
                        stream.content.clone()
                    } else {
                        stream
                            .decompressed_content()
                            .unwrap_or_else(|_| stream.content.clone())
                    };
                    xobj_map.insert(
                        xobject_id,
                        PendingXObject::DecodeImage {
                            bytes,
                            smask: extract_smask(doc, stream),
                            fallback: preserve_xobject_stream(doc, stream),
                        },
                    );
                } else if let Some(img) = raw_bitmap_from_stream(doc, stream) {
                    // Without the `images` feature, `XObject::Image` cannot be
                    // *serialized* (xobject.rs `image_to_stream` panics), so
                    // keep the raw bitmap stream verbatim in that case — the
                    // round trip preserves the image either way.
                    let entry = if cfg!(feature = "images") {
                        PendingXObject::Ready(XObject::Image(img))
                    } else {
                        PendingXObject::Ready(XObject::External(preserve_xobject_stream(
                            doc, stream,
                        )))
                    };
                    xobj_map.insert(xobject_id, entry);
                } else {
                    // Could still be an embedded container file without a
                    // filter entry (rare), or something we cannot model —
                    // try the decoder, fall back to byte-level preservation.
                    let bytes = stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    xobj_map.insert(
                        xobject_id,
                        PendingXObject::DecodeImage {
                            bytes,
                            smask: extract_smask(doc, stream),
                            fallback: preserve_xobject_stream(doc, stream),
                        },
                    );
                }
            }
            _other => {
                // Form XObjects (and any other subtype) are preserved
                // verbatim, references inlined. They used to be dropped with
                // an "unknown xobject subtype" error, which deleted all
                // vector content that had been embedded via `Do`.
                xobj_map.insert(
                    xobject_id,
                    PendingXObject::Ready(XObject::External(preserve_xobject_stream(doc, stream))),
                );
            }
        }
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
) -> BTreeMap<XObjectId, PendingXObject> {
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
fn collect_page_refs(
    dict: &LopdfDictionary,
    doc: &LopdfDocument,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Vec<ObjectId>, String> {
    let mut pages = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    collect_page_refs_inner(dict, doc, &mut pages, &mut visited, warnings, 0)?;
    Ok(pages)
}

fn collect_page_refs_inner(
    dict: &LopdfDictionary,
    doc: &LopdfDocument,
    pages: &mut Vec<ObjectId>,
    visited: &mut std::collections::BTreeSet<ObjectId>,
    warnings: &mut Vec<PdfWarnMsg>,
    depth: usize,
) -> Result<(), String> {
    // A malformed (or hostile) page tree can contain reference cycles; without
    // the visited set + depth cap this recursion would never terminate.
    if depth > 64 {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            "Pages tree deeper than 64 levels, ignoring deeper nodes".to_string(),
        ));
        return Ok(());
    }

    // The Pages tree must have a "Kids" array (possibly behind a reference).
    let kids = dict
        .get(b"Kids")
        .map_err(|e| format!("Pages dictionary missing Kids key: {}", e))?;
    let kids = match kids {
        Object::Reference(r) => doc
            .get_object(*r)
            .map_err(|e| format!("Failed to resolve Pages.Kids reference: {}", e))?,
        other => other,
    };

    let page_refs = kids
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(|k| k.as_reference().ok())
                .collect::<Vec<_>>()
        })
        .map_err(|_| "Pages.Kids is not an array".to_string())?;

    for r in page_refs {
        if !visited.insert(r) {
            warnings.push(PdfWarnMsg::warning(
                0,
                0,
                format!("Pages tree contains object {:?} more than once, skipping", r),
            ));
            continue;
        }
        let kid_obj = match doc.get_object(r) {
            Ok(o) => o,
            Err(e) => {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    format!("Failed to get pages-tree kid object {:?}: {}", r, e),
                ));
                continue;
            }
        };

        if let Ok(kid_dict) = kid_obj.as_dict() {
            // /Type is required by the spec but missing in plenty of real
            // documents; infer the node kind from the presence of /Kids.
            let type_name = match kid_dict.get(b"Type") {
                Ok(Object::Name(t)) => Some(t.clone()),
                _ => None,
            };
            match type_name.as_deref() {
                Some(b"Page") => pages.push(r),
                Some(b"Pages") => {
                    collect_page_refs_inner(kid_dict, doc, pages, visited, warnings, depth + 1)?
                }
                Some(other) => {
                    warnings.push(PdfWarnMsg::warning(
                        0,
                        0,
                        format!(
                            "Unknown pages-tree kid type {:?}, skipping",
                            String::from_utf8_lossy(other)
                        ),
                    ));
                }
                None => {
                    if kid_dict.has(b"Kids") {
                        collect_page_refs_inner(kid_dict, doc, pages, visited, warnings, depth + 1)?
                    } else {
                        warnings.push(PdfWarnMsg::warning(
                            0,
                            0,
                            format!("Pages-tree kid {:?} missing /Type, treating as /Page", r),
                        ));
                        pages.push(r);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Look up an inheritable page attribute (`MediaBox`, `CropBox`, `Resources`,
/// `Rotate`, ...): first on the page dictionary itself, then up the `/Parent`
/// chain (ISO 32000-1, 7.7.3.4). Depth-capped to survive cyclic parent links.
fn get_inheritable_page_attr<'a>(
    page: &'a LopdfDictionary,
    doc: &'a LopdfDocument,
    key: &[u8],
) -> Option<&'a Object> {
    let mut current = page;
    for _ in 0..64 {
        if let Ok(obj) = current.get(key) {
            return Some(obj);
        }
        let parent = current.get(b"Parent").ok()?;
        let parent_ref = parent.as_reference().ok()?;
        current = doc.get_dictionary(parent_ref).ok()?;
    }
    None
}

/// Append the bytes of one content stream, given an entry of the `/Contents`
/// value (reference or inline stream). Warns instead of failing on bad
/// entries: a partly readable page is better than no document.
fn append_content_stream(
    doc: &LopdfDocument,
    obj: &Object,
    page_num: usize,
    content_data: &mut Vec<u8>,
    warnings: &mut Vec<PdfWarnMsg>,
) {
    let stream = match obj {
        Object::Reference(r) => match doc.get_object(*r).and_then(|o| o.as_stream()) {
            Ok(s) => s,
            Err(e) => {
                warnings.push(PdfWarnMsg::error(
                    page_num,
                    0,
                    format!("Failed to resolve content stream {:?}: {}", r, e),
                ));
                return;
            }
        },
        Object::Stream(s) => s,
        _other => {
            warnings.push(PdfWarnMsg::error(
                page_num,
                0,
                "Content entry is not a stream".to_string(),
            ));
            return;
        }
    };
    let data = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    content_data.extend(data);
    // PDF 32000-1, 7.8.2: when /Contents is an array, the division between
    // streams can fall in the middle of a token; the streams must be parsed
    // as if separated by (at least) a byte of whitespace.
    content_data.push(b'\n');
}

/// Parses a single page dictionary into a PdfPage.
fn parse_page(
    num: usize,
    page: &LopdfDictionary,
    doc: &LopdfDocument,
    xobjects: &BTreeMap<XObjectId, XObject>,
    font_decode: &BTreeMap<String, FontTextDecodeKind>,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<PdfPage, String> {
    // Parse MediaBox. Required per spec, but *inheritable* from the parent
    // Pages node — a common layout that used to hard-fail the whole document
    // with "Page missing MediaBox". Fall back to A4 with a warning.
    let media_box = match get_inheritable_page_attr(page, doc, b"MediaBox") {
        Some(obj) => match parse_rect(obj) {
            Ok(r) => r,
            Err(e) => {
                warnings.push(PdfWarnMsg::warning(
                    num,
                    0,
                    format!("Invalid MediaBox: {e}; defaulting to A4"),
                ));
                default_media_box()
            }
        },
        None => {
            warnings.push(PdfWarnMsg::warning(
                num,
                0,
                "Page missing MediaBox (also not inherited); defaulting to A4".to_string(),
            ));
            default_media_box()
        }
    };
    // TrimBox and CropBox are optional (and inheritable); use MediaBox as default.
    let trim_box = match get_inheritable_page_attr(page, doc, b"TrimBox").map(parse_rect) {
        Some(Ok(r)) => r,
        _ => media_box.clone(),
    };
    let crop_box = match get_inheritable_page_attr(page, doc, b"CropBox").map(parse_rect) {
        Some(Ok(r)) => r,
        _ => media_box.clone(),
    };

    // Get the Contents entry (could be a reference, an array, or a stream).
    // Contents is *optional* — a page without it is simply empty.
    let mut content_data = Vec::new();
    match page.get(b"Contents") {
        Ok(Object::Array(arr)) => {
            for obj in arr {
                append_content_stream(doc, obj, num, &mut content_data, warnings);
            }
        }
        Ok(Object::Reference(r)) => {
            // A /Contents reference may point at a stream *or* at an array of
            // stream references.
            match doc.get_object(*r) {
                Ok(Object::Array(arr)) => {
                    for obj in arr {
                        append_content_stream(doc, obj, num, &mut content_data, warnings);
                    }
                }
                Ok(other) => {
                    append_content_stream(doc, other, num, &mut content_data, warnings);
                }
                Err(e) => {
                    warnings.push(PdfWarnMsg::error(
                        num,
                        0,
                        format!("Failed to get content stream: {}", e),
                    ));
                }
            }
        }
        Ok(other) => {
            append_content_stream(doc, other, num, &mut content_data, warnings);
        }
        Err(_) => {
            warnings.push(PdfWarnMsg::info(
                num,
                0,
                "Page has no /Contents (empty page)".to_string(),
            ));
        }
    }

    // Decode the content stream into a vector of lopdf operations.
    // A page whose content stream fails to tokenize becomes an empty page
    // with a warning instead of failing the entire document.
    let ops = match lopdf::content::Content::decode(&content_data) {
        Ok(content) => content.operations,
        Err(e) => {
            warnings.push(PdfWarnMsg::error(
                num,
                0,
                format!("Failed to decode content stream: {}", e),
            ));
            Vec::new()
        }
    };

    // Convert lopdf operations to printpdf Ops.
    let mut page_state = PageState {
        font_decode: font_decode.clone(),
        ..PageState::default()
    };
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

/// A4 portrait in PDF points, the fallback when a document specifies no
/// usable MediaBox anywhere.
fn default_media_box() -> Rect {
    Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(595.28), Pt(841.89))
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

    /// How text-showing operator bytes decode for each font resource name of
    /// this page's resources. Filled in by `parse_page` before the ops run.
    pub font_decode: BTreeMap<String, FontTextDecodeKind>,

    /// The decode kind of the currently selected font (set by `Tf`).
    pub current_font_decode: Option<FontTextDecodeKind>,

    /// Current transformation matrix stack. Each entry is a 6-float array [a b c d e f].
    pub transform_stack: Vec<[f32; 6]>,

    /// Absolute text line position (the translation of the PDF text line matrix
    /// Tlm), tracked so the RELATIVE `Td` operator can be emitted as an absolute
    /// `SetTextCursor`. Without this, `50 700 Td … 0 -14 Td` (second line
    /// relative) parsed the second move as absolute `(0, -14)`, dropping every
    /// continuation line to the page's left edge. Reset by `BT`, set by `Tm`.
    pub text_line: [f32; 2],
    /// Text leading (from `TL`/`TD`), so `T*` advances the tracked line by the
    /// right amount.
    pub text_leading: f32,

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

/// Decode the byte string of one text-showing operator according to the
/// currently selected font (see [`FontTextDecodeKind`]).
///
/// * Built-in / simple fonts: one byte per code → decoded to a
///   [`crate::text::TextItem::Text`] (ToUnicode CMap first, WinAnsi fallback),
///   which the serializer re-encodes correctly for whichever font ends up
///   selected at save time.
/// * Composite (Type0) fonts: two-byte big-endian CIDs → [`crate::text::TextItem::GlyphIds`]
///   with each glyph's Unicode string attached from the ToUnicode CMap, so
///   text extraction and regenerated ToUnicode CMaps stay correct.
/// * Unknown font: fall back to the historic length-parity heuristic.
fn decode_show_text_string(
    bytes: &[u8],
    kind: Option<&FontTextDecodeKind>,
    page: usize,
    op_id: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> crate::text::TextItem {
    use crate::text::{decode_simple_font_bytes, Codepoint, TextItem};
    match kind {
        Some(FontTextDecodeKind::Builtin(_)) => {
            TextItem::Text(decode_simple_font_bytes(bytes, None))
        }
        Some(FontTextDecodeKind::Simple { to_unicode }) => TextItem::Text(
            decode_simple_font_bytes(bytes, to_unicode.as_deref()),
        ),
        Some(FontTextDecodeKind::Cid { to_unicode, cid_to_gid }) => {
            if bytes.len() % 2 != 0 {
                warnings.push(PdfWarnMsg::warning(
                    page,
                    op_id,
                    format!(
                        "CID font string has odd byte length {}, dropping trailing byte",
                        bytes.len()
                    ),
                ));
            }
            let glyphs = bytes
                .chunks_exact(2)
                .map(|c| {
                    let code = u16::from_be_bytes([c[0], c[1]]);
                    // ToUnicode is keyed by the content-stream CODE; the glyph id
                    // stored on the Codepoint must be the real GID so outlines,
                    // widths and re-subsetting stay correct when the descendant
                    // font's CID->GID mapping is not identity.
                    let cid_str = to_unicode
                        .as_deref()
                        .and_then(|cmap| cmap.lookup_string(code as u32));
                    let gid = cid_to_gid
                        .as_deref()
                        .and_then(|m| m.get(&code).copied())
                        .unwrap_or(code);
                    match cid_str {
                        Some(s) => Codepoint::with_cid(gid, 0.0, s),
                        None => Codepoint::new(gid, 0.0),
                    }
                })
                .collect();
            TextItem::GlyphIds(glyphs)
        }
        None => {
            // No font information: keep the old behavior for these (rare)
            // streams that show text before any usable `Tf`.
            let items = crate::text::decode_tj_string_as_glyph_ids(bytes);
            items
                .into_iter()
                .next()
                .unwrap_or(TextItem::GlyphIds(Vec::new()))
        }
    }
}

/// Decode a whole `TJ` array (strings interleaved with kerning offsets).
fn decode_show_text_array(
    arr: &[Object],
    kind: Option<&FontTextDecodeKind>,
    page: usize,
    op_id: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Vec<crate::text::TextItem> {
    use crate::text::TextItem;
    let mut items = Vec::new();
    for obj in arr {
        match obj {
            Object::String(bytes, _) => {
                items.push(decode_show_text_string(bytes, kind, page, op_id, warnings));
            }
            Object::Integer(i) => items.push(TextItem::Offset(*i as f32)),
            Object::Real(r) => items.push(TextItem::Offset(*r)),
            _ => {}
        }
    }
    items
}

/// Decode a text-showing string to a plain String (for the `'` and `"`
/// operators, whose printpdf ops carry `String` text).
fn decode_show_text_to_string(
    bytes: &[u8],
    kind: Option<&FontTextDecodeKind>,
    page: usize,
    op_id: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    use crate::text::TextItem;
    match decode_show_text_string(bytes, kind, page, op_id, warnings) {
        TextItem::Text(s) => s,
        TextItem::GlyphIds(glyphs) => glyphs
            .iter()
            .map(|g| g.cid.clone().unwrap_or_else(|| '\u{FFFD}'.to_string()))
            .collect(),
        TextItem::Offset(_) => String::new(),
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

                // Decode according to the *current font*: 1-byte WinAnsi codes
                // for builtin/simple fonts, 2-byte CIDs for composite fonts.
                let text_items = decode_show_text_array(
                    arr,
                    state.current_font_decode.as_ref(),
                    page,
                    op_id,
                    warnings,
                );

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

                // Decode according to the current font (see decode_show_text_string).
                let text_items = vec![decode_show_text_string(
                    bytes,
                    state.current_font_decode.as_ref(),
                    page,
                    op_id,
                    warnings,
                )];

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
            // Move to the start of the next line: keep x, drop y by the leading.
            state.text_line[1] -= state.text_leading;
            out_ops.push(Op::AddLineBreak);
        }
        "TL" => {
            if op.operands.len() == 1 {
                let val = to_f32(&op.operands[0]);
                state.text_leading = val;
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
                // TD = Td + set leading to -ty. Relative move; track it and the leading.
                let tx = to_f32(&op.operands[0]);
                let ty = to_f32(&op.operands[1]);
                state.text_line[0] += tx;
                state.text_line[1] += ty;
                state.text_leading = -ty;
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
                // Tm sets the text line matrix absolutely; track its translation
                // so a following relative `Td` accumulates from here.
                state.text_line = [e, f];
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
            // --- Text mode begin (BT resets the text + text line matrix to identity)
            state.in_text_mode = true;
            state.text_line = [0.0, 0.0];
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
                    // Resolve how this font's text bytes decode. The document's
                    // *actual* font resources take priority: a foreign PDF may
                    // name an embedded font "F1", which the F1-F14 builtin
                    // naming heuristic would otherwise mis-claim as
                    // Times-Roman and destroy both decode and re-encode.
                    state.current_font_decode = match state.font_decode.get(&font_name) {
                        Some(kind) => Some(kind.clone()),
                        None => {
                            BuiltinFont::from_id(&font_name).map(FontTextDecodeKind::Builtin)
                        }
                    };
                }
                let size_val = to_f32(&op.operands[1]);
                state.current_font_size = Some(crate::units::Pt(size_val));

                // Emit new SetFont operation (1:1 PDF mapping)
                if let (Some(font_resource), Some(sz)) = (&state.current_font_resource, &state.current_font_size) {
                    // Builtin or external is decided by the resolved decode
                    // kind (falling back to the F1-F14 name heuristic when the
                    // document's resources don't describe the font).
                    let font_handle = match &state.current_font_decode {
                        Some(FontTextDecodeKind::Builtin(b)) => {
                            crate::ops::PdfFontHandle::Builtin(*b)
                        }
                        Some(_) => {
                            crate::ops::PdfFontHandle::External(crate::FontId(font_resource.clone()))
                        }
                        None => {
                            if let Some(builtin) = BuiltinFont::from_id(font_resource) {
                                crate::ops::PdfFontHandle::Builtin(builtin)
                            } else {
                                crate::ops::PdfFontHandle::External(crate::FontId(
                                    font_resource.clone(),
                                ))
                            }
                        }
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
                // `Td` is RELATIVE to the current text line matrix. Accumulate it
                // into the absolute line position and emit an absolute cursor,
                // since printpdf's SetTextCursor is absolute.
                let tx = to_f32(&op.operands[0]);
                let ty = to_f32(&op.operands[1]);
                state.text_line[0] += tx;
                state.text_line[1] += ty;
                out_ops.push(Op::SetTextCursor {
                    pos: crate::graphics::Point {
                        x: crate::units::Pt(state.text_line[0]),
                        y: crate::units::Pt(state.text_line[1]),
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
                // Emit BeginLayer (not BeginOptionalContent): the serializer
                // only builds the page's /Properties entries — which make the
                // `BDC /OC /Name` reference resolvable — from `BeginLayer`
                // ops. Both ops write identical content-stream bytes.
                out_ops.push(Op::BeginLayer {
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
                        let pattern = arr.iter().map(|item| to_f32(item)).collect();
                        let offset = to_f32(&op.operands[1]);
                        let dash = LineDashPattern{offset, pattern};
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
        // --- Show text with line advance (' and ") ---
        // These show text exactly like Tj; leaving them unhandled turned them
        // into `Op::Unknown`, which the (default, `secure`) serializer drops —
        // deleting the text from the round-tripped document.
        "'" => {
            if let Some(lopdf::Object::String(bytes, _)) = op.operands.get(0) {
                let text = decode_show_text_to_string(
                    bytes,
                    state.current_font_decode.as_ref(),
                    page,
                    op_id,
                    warnings,
                );
                out_ops.push(Op::MoveToNextLineShowText { text });
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    "Warning: `'` operand is not a string".to_string(),
                ));
            }
        }
        "\"" => {
            if op.operands.len() == 3 {
                let word_spacing = to_f32(&op.operands[0]);
                let char_spacing = to_f32(&op.operands[1]);
                if let lopdf::Object::String(bytes, _) = &op.operands[2] {
                    let text = decode_show_text_to_string(
                        bytes,
                        state.current_font_decode.as_ref(),
                        page,
                        op_id,
                        warnings,
                    );
                    out_ops.push(Op::SetSpacingMoveAndShowText {
                        word_spacing,
                        char_spacing,
                        text,
                    });
                } else {
                    warnings.push(PdfWarnMsg::error(
                        page,
                        op_id,
                        "Warning: `\"` third operand is not a string".to_string(),
                    ));
                }
            } else {
                warnings.push(PdfWarnMsg::error(
                    page,
                    op_id,
                    format!("Warning: `\"` expects 3 operands, got {}", op.operands.len()),
                ));
            }
        }

        // Paint a named /Shading resource over the current clip. Parsed into the
        // typed op so the default (`secure`) save no longer deletes it as
        // `Op::Unknown` — gradients used to vanish on every round trip.
        "sh" => {
            if let Some(name_str) = as_name(&op.operands.get(0).unwrap_or(&lopdf::Object::Null)) {
                out_ops.push(Op::PaintShading {
                    id: crate::ShadingId(name_str),
                });
            }
        }

        "Do" => {
            if let Some(name_str) = as_name(&op.operands.get(0).unwrap_or(&lopdf::Object::Null)) {
                let xobj_id = crate::XObjectId(name_str);
                // The document's own `cm` operators (already emitted as
                // `Op::SetTransformationMatrix`) fully position this XObject —
                // tell the serializer not to add its unit-square → pixel-size
                // convenience scale on top (it would scale the image twice).
                out_ops.push(Op::UseXobject {
                    id: xobj_id,
                    transform: crate::xobject::XObjectTransform {
                        no_auto_scale: true,
                        ..Default::default()
                    },
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
    /// Extract link annotations, tagged with the 0-based index of the page
    /// they belong to (so the parser can re-attach them to their pages as
    /// `Op::LinkAnnotation` — otherwise they are silently dropped on re-save).
    pub fn extract_link_annotations(doc: &Document) -> Vec<(usize, LinkAnnotation)> {
        // Build a page mapping from object id to page number.
        let page_map: BTreeMap<ObjectId, usize> = doc
            .get_pages()
            .iter()
            .map(|(num, id)| (*id, *num as usize))
            .collect();

        // For every page, try to extract its annotations.
        doc.get_pages()
            .iter()
            .flat_map(|(page_num, &page_id)| {
                let page_idx = (*page_num as usize).saturating_sub(1);
                let page_dict = doc.get_object(page_id).ok()?.as_dict().ok()?;
                let annots = page_dict.get(b"Annots").ok()?.as_array().ok()?;
                Some(
                    annots
                        .iter()
                        .filter_map(|annot_obj| {
                            // Expect a reference to an annotation dictionary
                            // (tolerate inline dictionaries too).
                            let annot_dict = match annot_obj {
                                Object::Reference(id) => {
                                    doc.get_object(*id).ok()?.as_dict().ok()?
                                }
                                Object::Dictionary(d) => d,
                                _ => return None,
                            };
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
                            // /Rect is [llx lly urx ury]; from_xywh wants
                            // x, y, width, height.
                            let rect = Rect::from_xywh(
                                Pt(coords[0]),
                                Pt(coords[1]),
                                Pt(coords[2] - coords[0]),
                                Pt(coords[3] - coords[1]),
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

                            Some((
                                page_idx,
                                LinkAnnotation {
                                    rect,
                                    actions,
                                    border,
                                    color,
                                    highlighting,
                                },
                            ))
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

    use std::collections::{BTreeMap, BTreeSet};

    use lopdf::{Document, Object, ObjectId};

    use crate::ops::{Layer, LayerIntent, LayerSubtype};

    /// Resolve one level of indirection, returning the target object.
    fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
        match obj {
            Object::Reference(r) => doc.get_object(*r).unwrap_or(obj),
            other => other,
        }
    }

    /// Extract layers (optional content groups) from the PDF document,
    /// together with the resource name under which each is exposed to content
    /// streams.
    ///
    /// Content streams reference layers as `BDC /OC /SomeName`, where
    /// `/SomeName` is a key of the page's `/Resources` → `/Properties`
    /// dictionary that points at the OCG object. The layer map of the parsed
    /// document must be keyed by those names — the parser stores them in
    /// `Op::BeginOptionalContent { layer_id }` — otherwise every re-saved
    /// `BDC` references a property that does not exist and viewers drop the
    /// layer content ("Marked Content 'X' is unknown").
    ///
    /// Returns `(Some(resource_name), layer)` for OCGs reachable from some
    /// page's `/Properties`, `(None, layer)` for OCGs only listed in the
    /// catalog's `/OCProperties`.
    pub fn extract_layers(doc: &Document) -> Vec<(Option<String>, Layer)> {
        // OCG object id -> resource name (first one wins across pages).
        let mut ocg_names: BTreeMap<ObjectId, String> = BTreeMap::new();
        for (_page_num, page_id) in doc.get_pages() {
            let Ok(page_dict) = doc.get_dictionary(page_id) else {
                continue;
            };
            let Ok(res_obj) = page_dict.get(b"Resources") else {
                continue;
            };
            let Object::Dictionary(res_dict) = resolve(doc, res_obj) else {
                continue;
            };
            let Ok(props_obj) = res_dict.get(b"Properties") else {
                continue;
            };
            let Object::Dictionary(props) = resolve(doc, props_obj) else {
                continue;
            };
            for (name, val) in props.iter() {
                if let Object::Reference(ocg_id) = val {
                    ocg_names
                        .entry(*ocg_id)
                        .or_insert_with(|| String::from_utf8_lossy(name).to_string());
                }
            }
        }

        // Get the catalog from the trailer.
        let catalog = match doc
            .trailer
            .get(b"Root")
            .ok()
            .map(|o| resolve(doc, o))
        {
            Some(Object::Dictionary(dict)) => dict,
            _ => return vec![],
        };

        // Get the OCProperties dictionary.
        let ocprops = match catalog.get(b"OCProperties").ok().map(|o| resolve(doc, o)) {
            Some(Object::Dictionary(dict)) => dict,
            _ => return vec![],
        };

        // Get the array of OCGs (optional content groups / layers).
        let ocgs = match ocprops.get(b"OCGs").ok().map(|o| resolve(doc, o)) {
            Some(Object::Array(arr)) => arr,
            _ => return vec![],
        };

        // Also, if available, get the "ON" array from the "D" dictionary
        // to decide which layers are turned on by default.
        let default_on = match ocprops.get(b"D").ok().map(|o| resolve(doc, o)) {
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

            // Extract the layer name (accept both string and hex-string forms).
            let name = match layer_dict.get(b"Name").ok() {
                Some(Object::String(s, _)) => s.clone(),
                _ => continue,
            };

            // Extract the intent from the "Intent" key: either a name, an
            // inline array of names, or a reference to such an array.
            let intent = match layer_dict.get(b"Intent").ok().map(|o| resolve(doc, o)) {
                Some(Object::Array(arr)) => match arr.first() {
                    Some(Object::Name(n)) if n.as_slice() == b"View" => LayerIntent::View,
                    _ => LayerIntent::Design,
                },
                Some(Object::Name(n)) if n.as_slice() == b"View" => LayerIntent::View,
                _ => LayerIntent::Design,
            };

            // Extract usage info from the "Usage" key (inline dict or reference).
            let (creator, usage_subtype) = match layer_dict
                .get(b"Usage")
                .ok()
                .map(|o| resolve(doc, o))
            {
                Some(Object::Dictionary(usage_dict)) => {
                    if let Some(Object::Dictionary(creator_info)) = usage_dict
                        .get(b"CreatorInfo")
                        .ok()
                        .map(|o| resolve(doc, o))
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

            layers.push((
                ocg_names.get(layer_ref).cloned(),
                Layer {
                    name: String::from_utf8_lossy(&name).to_string(),
                    creator,
                    intent: final_intent,
                    usage: usage_subtype,
                },
            ));
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

        // PDF spec: `CA` is the stroking (stroke) alpha, `ca` the nonstroking
        // (fill) alpha. The write side (graphics.rs) emits them that way; parse
        // them back to the matching fields (previously swapped — broke the
        // ExtGState save->parse roundtrip).
        if let Some(obj) = dict.get(b"CA").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.current_stroke_alpha = num;
                changed.insert(ChangedField::CurrentStrokeAlpha);
            }
        }

        if let Some(obj) = dict.get(b"ca").ok() {
            if let Some(num) = parse_f32(obj) {
                gs.current_fill_alpha = num;
                changed.insert(ChangedField::CurrentFillAlpha);
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
                let pattern = arr.iter().filter_map(|o| parse_f32(o)).collect();
                gs.line_dash_pattern = Some(LineDashPattern{offset: 0.0, pattern}); // default offset = 0
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
    use std::sync::Arc;

    use lopdf::{Dictionary, Document, Object};

    use super::{ParsedExternalFont, ParsedOrBuiltinFont};
    use crate::{
        cmap::ToUnicodeCMap,
        deserialize::{get_dict_or_resolve_ref, PdfWarnMsg},
        font::ParsedFont,
        BuiltinFont, FontId,
    };

    /// Build a code -> unicode map from the font's own cmap table, for fonts
    /// that ship without a usable `/ToUnicode` CMap. This makes glyph runs
    /// extractable and lets the serializer regenerate a correct ToUnicode CMap
    /// instead of mapping every glyph to U+FFFD.
    ///
    /// ToUnicode is keyed by the content-stream code — the CID under Identity-H.
    /// When the descendant font maps CIDs to different glyph ids (`cid_to_gid`),
    /// the font-cmap gid must be re-keyed through the inverse of that map;
    /// otherwise code == gid.
    fn synthesize_to_unicode(
        font: &ParsedFont,
        cid_to_gid: Option<&BTreeMap<u16, u16>>,
    ) -> Option<ToUnicodeCMap> {
        let gid_to_cid: Option<BTreeMap<u16, u16>> = cid_to_gid
            .map(|m| m.iter().map(|(cid, gid)| (*gid, *cid)).collect());
        let mut mappings: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        // The full scalar-value range: supplementary-plane codepoints (CJK Ext B+,
        // emoji) are exactly what the CJK fonts hitting this path use.
        for cp in 0x0000..=0x10FFFFu32 {
            // skip UTF-16 surrogate range, not scalar values
            if (0xD800..=0xDFFF).contains(&cp) {
                continue;
            }
            if let Some(gid) = font.lookup_glyph_index(cp) {
                let code = gid_to_cid
                    .as_ref()
                    .and_then(|m| m.get(&gid).copied())
                    .unwrap_or(gid);
                mappings.entry(code as u32).or_insert_with(|| vec![cp]);
            }
        }
        if mappings.is_empty() {
            None
        } else {
            Some(ToUnicodeCMap { mappings })
        }
    }

    /// The descendant font's CID -> glyph-id map, when it is not identity.
    ///
    /// Two sources, matching how viewers resolve Identity-H codes (ISO 32000-1,
    /// 9.7.4.2):
    /// - CIDFontType2: an explicit `/CIDToGIDMap` stream — 2 big-endian bytes per
    ///   CID. (The `/Identity` name, or no entry, means CID == GID.)
    /// - CIDFontType0 with a CID-keyed CFF program: the CFF charset, inverted.
    ///
    /// Only non-identity entries are stored; lookups fall back to CID == GID.
    ///
    /// The CFF-charset source is additionally validated against the document's
    /// ToUnicode CMap and the font's own cmap: printpdf ≤ 0.12 (#280) — and other
    /// producers with the same bug — wrote glyph ids as Identity-H codes even for
    /// CID-keyed CFF fonts whose charset says otherwise. PDFium renders such files
    /// by falling back to code == GID, and re-interpreting them per spec here would
    /// scramble glyph identities on re-save. When the identity reading explains the
    /// document's own code→unicode→glyph triangle better than the charset reading,
    /// the charset map is discarded. (An explicit `/CIDToGIDMap` stream is never
    /// second-guessed — producers that write one mean it.)
    fn extract_cid_to_gid_map(
        doc: &Document,
        font_dict: &Dictionary,
        font_id: &FontId,
        parsed_font: &ParsedFont,
        to_unicode: Option<&ToUnicodeCMap>,
        warnings: &mut Vec<PdfWarnMsg>,
        page_num: usize,
    ) -> Option<BTreeMap<u16, u16>> {
        let descendant = get_descendant_font_dict(doc, font_dict, font_id, warnings, page_num)?;

        let stream_to_map = |s: &lopdf::Stream| -> Option<BTreeMap<u16, u16>> {
            let data = s.decompressed_content().ok().or_else(|| {
                // Already-plain streams return the content as-is on error paths of
                // some lopdf versions; fall back to the raw bytes.
                Some(s.content.clone())
            })?;
            Some(
                data.chunks_exact(2)
                    .enumerate()
                    .filter_map(|(cid, b)| {
                        let cid = u16::try_from(cid).ok()?;
                        let gid = u16::from_be_bytes([b[0], b[1]]);
                        (gid != cid).then_some((cid, gid))
                    })
                    .collect(),
            )
        };

        match descendant.get(b"CIDToGIDMap") {
            Ok(Object::Stream(s)) => return stream_to_map(s),
            Ok(Object::Reference(r)) => {
                if let Ok(Object::Stream(s)) = doc.get_object(*r) {
                    return stream_to_map(s);
                }
            }
            _ => {} // absent or /Identity
        }

        // CID-keyed CFF: invert the charset's gid -> cid map. Identity entries are
        // dropped so `None`/empty means "no remapping needed".
        #[cfg(feature = "text_layout")]
        let bytes = parsed_font.source_bytes()?;
        #[cfg(feature = "text_layout")]
        let bytes = bytes.as_slice();
        #[cfg(not(feature = "text_layout"))]
        let bytes = parsed_font.original_bytes.as_slice();

        let map: BTreeMap<u16, u16> = crate::font::cff_charset_gid_to_cid_map(bytes, 0)?
            .into_iter()
            .filter_map(|(gid, cid)| (gid != cid).then_some((cid, gid)))
            .collect();
        if map.is_empty() {
            return None;
        }

        // Disambiguate spec-correct files from legacy code==GID files: for every
        // ToUnicode entry whose character the font's cmap knows, check which
        // reading of the code lands on the cmap's glyph.
        if let Some(tu) = to_unicode {
            let mut spec_hits = 0usize;
            let mut identity_hits = 0usize;
            for (code, unis) in tu.mappings.iter() {
                let Some(&cp) = unis.first() else { continue };
                let Ok(code16) = u16::try_from(*code) else { continue };
                let Some(cmap_gid) = parsed_font.lookup_glyph_index(cp) else {
                    continue;
                };
                let spec_gid = map.get(&code16).copied().unwrap_or(code16);
                if spec_gid == cmap_gid {
                    spec_hits += 1;
                }
                if code16 == cmap_gid {
                    identity_hits += 1;
                }
            }
            if identity_hits > spec_hits {
                warnings.push(PdfWarnMsg::info(
                    page_num,
                    0,
                    format!(
                        "font {}: CID-keyed CFF charset is not identity, but the \
                         document's codes match glyph ids, not CIDs ({} vs {} cmap \
                         agreements) — treating codes as glyph ids (legacy printpdf \
                         convention, #280)",
                        font_id.0, identity_hits, spec_hits
                    ),
                ));
                return None;
            }
        }

        Some(map)
    }

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
                // The decoder assumes Identity-H: 2-byte big-endian codes, code == CID.
                // Other encodings exist in the wild — predefined CJK CMaps (90ms-RKSJ-H,
                // GBK-EUC-H: variable-width codes) and Identity-V (vertical writing) —
                // and are NOT resolved; the byte stream would be chopped into 2-byte
                // codes regardless and vertical text re-saved as horizontal. Until CMap
                // resolution exists, say so instead of silently scrambling.
                match font_dict.get(b"Encoding") {
                    Ok(Object::Name(n)) if n.as_slice() != b"Identity-H" => {
                        warnings.push(PdfWarnMsg::warning(
                            page_num,
                            0,
                            format!(
                                "Type0 font {} uses /Encoding /{} — only Identity-H is \
                                 supported; text may decode and re-save incorrectly{}",
                                font_id.0,
                                String::from_utf8_lossy(n),
                                if n.as_slice() == b"Identity-V" {
                                    " (vertical writing is converted to horizontal)"
                                } else {
                                    ""
                                }
                            ),
                        ));
                    }
                    Ok(Object::Stream(_)) | Ok(Object::Reference(_)) => {
                        warnings.push(PdfWarnMsg::warning(
                            page_num,
                            0,
                            format!(
                                "Type0 font {} uses an embedded CMap for /Encoding — only \
                                 Identity-H is supported; text may decode and re-save \
                                 incorrectly",
                                font_id.0
                            ),
                        ));
                    }
                    _ => {}
                }
                // Not gated on `text_layout`: an external font is a Type0 (CID) font, and
                // this branch being empty without the feature is why `PdfDocument::parse`
                // silently returned zero font resources for any PDF with an embedded font
                // (#258).
                match process_type0_font(doc, font_dict, &font_id, warnings, page_num) {
                    Some(parsed_font) => {
                        let cid_to_gid = extract_cid_to_gid_map(
                            doc,
                            font_dict,
                            &font_id,
                            &parsed_font,
                            to_unicode_cmap.as_ref(),
                            warnings,
                            page_num,
                        )
                        .map(Arc::new);
                        // Without a ToUnicode CMap, glyph runs would extract as
                        // U+FFFD and the re-saved ToUnicode would be garbage;
                        // synthesize one from the font's own cmap table.
                        let to_unicode = to_unicode_cmap
                            .or_else(|| {
                                synthesize_to_unicode(&parsed_font, cid_to_gid.as_deref())
                            })
                            .map(Arc::new);
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::P(ParsedExternalFont {
                                font: parsed_font,
                                to_unicode,
                                is_cid: true,
                                cid_to_gid,
                            }),
                        );
                    }
                    None => {
                        // Keep the code layout + ToUnicode for decoding even
                        // though the font itself cannot be re-embedded.
                        warnings.push(PdfWarnMsg::warning(
                            page_num,
                            0,
                            format!(
                                "Type0 font {} has no parsable font program; text will decode \
                                 via its ToUnicode CMap but the font cannot be re-embedded",
                                font_id.0
                            ),
                        ));
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::CidStub(to_unicode_cmap.map(Arc::new)),
                        );
                    }
                }
            } else {
                match process_standard_font(doc, font_dict, &font_id, warnings, page_num) {
                    Some(ParsedOrBuiltinFont::B(builtin)) => {
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::B(builtin)
                        );
                    },
                    Some(ParsedOrBuiltinFont::P(p)) => {
                        fonts_map.insert(
                            font_id,
                            ParsedOrBuiltinFont::P(ParsedExternalFont {
                                to_unicode: to_unicode_cmap.map(Arc::new),
                                ..p
                            })
                        );
                    },
                    Some(ParsedOrBuiltinFont::CidStub(_)) | None => {}
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
                    // No cfg split needed any more: `from_bytes` now takes the same warning
                    // type with and without `text_layout` (#260).
                    for fw in font_warnings {
                        warnings.push(PdfWarnMsg::warning(page_num, 0, format!("{fw:?}")));
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
                #[cfg(feature = "text_layout")]
                {
                    match process_type1_font(doc, font_dict, font_id, warnings, page_num) {
                        Some(parsed_font) => {
                            Some(ParsedOrBuiltinFont::P(ParsedExternalFont {
                                font: parsed_font,
                                to_unicode: None,
                                is_cid: false,
                                // Simple font: one-byte codes, no CID indirection.
                                cid_to_gid: None,
                            }))
                        },
                        None => {
                            let substitute = builtin_substitute(&basefont);
                            warnings.push(PdfWarnMsg::warning(
                                page_num,
                                0,
                                format!(
                                    "Font {} ({}) has no embedded font program; substituting \
                                     builtin {:?}",
                                    font_id.0, basefont, substitute
                                ),
                            ));
                            Some(ParsedOrBuiltinFont::B(substitute))
                        }
                    }
                }
                #[cfg(not(feature = "text_layout"))]
                {
                    let substitute = builtin_substitute(&basefont);
                    warnings.push(PdfWarnMsg::warning(
                        page_num,
                        0,
                        format!(
                            "Font {} ({}) has no embedded font program; substituting builtin \
                             {:?}",
                            font_id.0, basefont, substitute
                        ),
                    ));
                    Some(ParsedOrBuiltinFont::B(substitute))
                }
            }
        }
    }

    /// Pick the closest standard-14 font for a simple font that is not
    /// embedded in the document (`Arial`, `TimesNewRomanPSMT`, ...). Rendering
    /// with a substitute face keeps the text readable and re-encodable; the
    /// alternative — dropping the font — turns every string that uses it into
    /// dangling glyph ids on re-save.
    fn builtin_substitute(basefont: &str) -> BuiltinFont {
        use crate::BuiltinFont::*;
        // Strip the "ABCDEF+" subset prefix if present.
        let name = basefont.rsplit('+').next().unwrap_or(basefont).to_ascii_lowercase();
        let bold = name.contains("bold");
        let italic = name.contains("italic") || name.contains("oblique");
        if name.contains("times") || name.contains("serif") || name.contains("georgia")
            || name.contains("garamond") || name.contains("book")
        {
            match (bold, italic) {
                (true, true) => TimesBoldItalic,
                (true, false) => TimesBold,
                (false, true) => TimesItalic,
                (false, false) => TimesRoman,
            }
        } else if name.contains("courier") || name.contains("mono") || name.contains("consol") {
            match (bold, italic) {
                (true, true) => CourierBoldOblique,
                (true, false) => CourierBold,
                (false, true) => CourierOblique,
                (false, false) => Courier,
            }
        } else if name.contains("symbol") {
            Symbol
        } else if name.contains("zapf") || name.contains("dingbat") {
            ZapfDingbats
        } else {
            match (bold, italic) {
                (true, true) => HelveticaBoldOblique,
                (true, false) => HelveticaBold,
                (false, true) => HelveticaOblique,
                (false, false) => Helvetica,
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
