use std::collections::BTreeMap;

use svg2pdf::{usvg, ConversionOptions, PageOptions};

use crate::{units::Pt, xobject::ExternalXObject, DictItem, ExternalStream, PdfWarnMsg};

/// Maximum recursion depth when inlining indirect objects from the
/// svg2pdf-generated PDF into the Form XObject dictionary. The object graph
/// svg2pdf emits is a shallow DAG; this bound only guards against pathological
/// or cyclic input.
const MAX_INLINE_DEPTH: usize = 64;

/// SVG - wrapper around an `XObject` to allow for more
/// control within the library.
///
/// When placing multiple copies of the same SVG on the
/// same layer, it is better to use the `into_xobject`
/// method to get a reference, rather than a clone
#[derive(Debug, Clone)]
pub struct Svg {}

impl Svg {
    /// Parses the SVG string, converts it to a PDF XObject
    pub fn parse(
        svg_string: &str,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<ExternalXObject, String> {
        Self::parse_with_fonts(svg_string, &BTreeMap::new(), warnings)
    }

    /// Same as [`Svg::parse`], but `<text>` elements resolve against fonts supplied
    /// by the caller, in addition to the system fonts where those exist. On wasm this
    /// is the only way to get SVG text rendered at all — there is no system font
    /// database to scan there. Map keys are informational; family names are read from
    /// the font data itself.
    ///
    /// Each supplied font is reachable under its typographic family (e.g.
    /// "Roboto"), its legacy family / full name (e.g. "Roboto Medium") and its
    /// PostScript name (e.g. "Roboto-Medium"). `<text>` elements *without* a
    /// `font-family` attribute use the first supplied font instead of usvg's
    /// default "Times New Roman" (which rarely exists outside Windows). If a
    /// `<text>` element still resolves to no font, a warning is pushed instead
    /// of dropping it silently.
    pub fn parse_with_fonts(
        svg_string: &str,
        fonts: &BTreeMap<String, Vec<u8>>,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> Result<ExternalXObject, String> {
        // Parses the SVG and converts it to a single-page PDF document using the
        // svg2pdf crate. That page then becomes a self-contained Form XObject:
        // its content stream is carried over verbatim, and — critically — so is
        // the page's full `/Resources` subtree (ColorSpace, ExtGState, XObject,
        // Pattern, Shading, Font, ...), with every indirect reference resolved
        // and inlined.
        //
        // Earlier versions re-parsed the svg2pdf output through printpdf's own
        // op parser and re-serialized the ops, replacing the resources with a
        // hardcoded (and malformed) `/Resources << /ColorSpace /DeviceRGB >>`.
        // That dropped every named resource the content stream references
        // (`/cs0 cs`, `/gs0 gs`, `/p0 scn`, `/x0 Do`, ...), turned gradient
        // fills into flat black, and produced files Adobe Acrobat refuses to
        // render (issues #113, #211).

        // Let's first convert the SVG into an independent chunk.
        let mut options = usvg::Options::default();
        #[cfg(not(target_arch = "wasm32"))]
        options.fontdb_mut().load_system_fonts();
        let supplied_families = register_supplied_fonts(&mut options, fonts);

        // Convert at 72 dpi, i.e. 1 svg px == 1 pt, so that svg2pdf's dpi
        // transform (a `cm` at the start of the page content) is the identity.
        //
        // This is not merely cosmetic: svg2pdf 0.13 writes pattern and shading
        // matrices in svg user space, anchored to the *page* coordinate system,
        // while the path geometry is scaled by the dpi transform inside the
        // content stream. Patterns do not follow `cm` (ISO 32000-1, 8.7.3.1),
        // so for any dpi != 72 every gradient and tiling pattern is drawn at
        // the wrong scale/offset (e.g. only the first slice of a gradient ramp
        // is visible). At 72 dpi geometry space == pattern space and the
        // output is consistent in every viewer.
        //
        // The rendered size on the page does not change: the XObject is
        // normalized to a unit square via /Matrix and scaled back up at the
        // `Op::UseXobject` site from its pixel width/height, which are
        // dpi-invariant.
        let dpi = 72.0;
        options.dpi = dpi;
        let tree = usvg::Tree::from_str(svg_string, &options)
            .map_err(|err| format!("usvg parse: {err}"))?;

        // usvg silently drops `<text>` nodes whose font-family (or, without
        // one, `Options::font_family`) cannot be resolved — the worst failure
        // mode of issue #184. Make it loud.
        let text_in_svg = count_svg_text_elements(svg_string);
        if text_in_svg > 0 {
            let text_in_tree = count_tree_text_nodes(tree.root());
            if text_in_tree < text_in_svg {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    format!(
                        "svg: {} of {} <text> element(s) were dropped because no matching font \
                         was found (default family: {:?}, families of supplied fonts: {:?}). \
                         Pass the font via Svg::parse_with_fonts and reference it by one of its \
                         family names.",
                        text_in_svg - text_in_tree,
                        text_in_svg,
                        options.font_family,
                        supplied_families,
                    ),
                ));
            }
        }

        let mut co = ConversionOptions::default();
        co.compress = false;
        co.embed_text = false; // TODO!

        let po = PageOptions { dpi };
        let pdf_bytes = svg2pdf::to_pdf(&tree, co, po)
            .map_err(|err| format!("convert svg tree to pdf: {err}"))?;

        let doc = lopdf::Document::load_mem(&pdf_bytes)
            .map_err(|err| format!("convert svg tree to pdf: parse pdf: {err}"))?;

        let page_id = doc
            .page_iter()
            .next()
            .ok_or_else(|| "convert svg tree to pdf: no page rendered".to_string())?;

        // Decompressed concatenation of the page's content stream(s).
        let content = doc.get_page_content(page_id);

        let page_dict = doc
            .get_dictionary(page_id)
            .map_err(|err| format!("convert svg tree to pdf: page is not a dictionary: {err}"))?;

        // svg2pdf always writes a `[0 0 w h]` MediaBox on the page; fall back
        // to the usvg tree size (px at `dpi`) if it is ever missing.
        let media_box = get_media_box(&doc, page_id).unwrap_or_else(|| {
            warnings.push(PdfWarnMsg::warning(
                0,
                0,
                "svg: generated page has no readable MediaBox, falling back to usvg tree size"
                    .to_string(),
            ));
            let size = tree.size();
            let scale = 72.0 / dpi;
            [0.0, 0.0, size.width() * scale, size.height() * scale]
        });

        let width_pt = media_box[2] - media_box[0];
        let height_pt = media_box[3] - media_box[1];
        if !(width_pt > 0.0) || !(height_pt > 0.0) {
            return Err(format!(
                "convert svg tree to pdf: page has degenerate media box {media_box:?}"
            ));
        }

        // Carry over the page's full /Resources subtree, with all indirect
        // references inlined so the Form XObject is self-contained. Streams
        // stay `DictItem::Stream` here; the serializer hoists them back out
        // into indirect objects when the final document is written (streams
        // must be indirect objects, ISO 32000-1 § 7.3.8.1).
        let resources = match page_dict.get(b"Resources") {
            Ok(res) => match inline_lopdf_object(&doc, res, 0, warnings) {
                DictItem::Dict { map } => DictItem::Dict { map },
                other => {
                    warnings.push(PdfWarnMsg::warning(
                        0,
                        0,
                        format!("svg: page /Resources is not a dictionary ({other:?}), ignoring"),
                    ));
                    DictItem::Dict {
                        map: BTreeMap::new(),
                    }
                }
            },
            Err(_) => DictItem::Dict {
                map: BTreeMap::new(),
            },
        };

        // Scale the PDF content down to a 1:1 unit square,
        // so that it behaves like an image
        let sx = 1.0 / width_pt;
        let sy = 1.0 / height_pt;

        let mut dict: BTreeMap<String, DictItem> = [
            ("Type", DictItem::Name("XObject".into())),
            ("Subtype", DictItem::Name("Form".into())),
            (
                "ProcSet",
                DictItem::Array(vec![
                    DictItem::Name("PDF".into()),
                    DictItem::Name("Text".into()),
                    DictItem::Name("ImageC".into()),
                    DictItem::Name("ImageB".into()),
                ]),
            ),
            (
                "BBox",
                DictItem::Array(vec![
                    DictItem::Real(media_box[0]),
                    DictItem::Real(media_box[1]),
                    DictItem::Real(media_box[2]),
                    DictItem::Real(media_box[3]),
                ]),
            ),
            (
                "Matrix",
                DictItem::Array(vec![
                    DictItem::Real(sx),
                    DictItem::Real(0.0),
                    DictItem::Real(0.0),
                    DictItem::Real(sy),
                    DictItem::Real(-media_box[0] * sx),
                    DictItem::Real(-media_box[1] * sy),
                ]),
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        dict.insert("Resources".to_string(), resources);

        // svg2pdf marks the page as an isolated transparency group; carrying
        // that over keeps alpha compositing (group opacity, soft masks)
        // correct — Acrobat is strict about this.
        if let Ok(group) = page_dict.get(b"Group") {
            match inline_lopdf_object(&doc, group, 0, warnings) {
                DictItem::Dict { map } => {
                    dict.insert("Group".to_string(), DictItem::Dict { map });
                }
                _ => {}
            }
        }

        Ok(ExternalXObject {
            stream: ExternalStream {
                dict,
                content,
                compress: false,
            },
            width: Some(Pt(width_pt).into_px(dpi)),
            height: Some(Pt(height_pt).into_px(dpi)),
            dpi: Some(dpi),
        })
    }
}

/// Loads the caller-supplied fonts into the fontdb of `options` and makes
/// them reachable the way users expect (issue #184). Returns every family
/// name the supplied fonts are now registered under (for diagnostics).
///
/// 1. Every face is *also* registered under its legacy family (name ID 1),
///    full name (ID 4) and PostScript name (ID 6): fontdb only indexes the
///    typographic family (e.g. "Roboto" for RobotoMedium.ttf), while SVGs in
///    the wild routinely say `font-family="Roboto Medium"` — without the
///    aliases usvg finds no match and silently drops the text.
/// 2. `Options::font_family` — the family used by `<text>` elements without
///    a `font-family` attribute — defaults to "Times New Roman", which is
///    absent on most Linux systems and always absent on wasm. When the
///    caller supplied fonts, the first supplied family becomes the default
///    instead, so bare `<text>` uses the font that was explicitly provided.
fn register_supplied_fonts(
    options: &mut usvg::Options,
    fonts: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    use svg2pdf::usvg::fontdb;

    let mut registered_families: Vec<String> = Vec::new();
    let mut default_family: Option<String> = None;

    for bytes in fonts.values() {
        let db = options.fontdb_mut();
        let faces_before = db.len();
        db.load_font_data(bytes.clone());

        let new_faces: Vec<fontdb::FaceInfo> = db.faces().skip(faces_before).cloned().collect();

        for face in &new_faces {
            for (name, _) in &face.families {
                if !registered_families.iter().any(|f| f == name) {
                    registered_families.push(name.clone());
                }
            }
        }

        // Extra name-table names (legacy family, full, PostScript). Only
        // attached when the data contained exactly one face — for collections
        // the names could belong to a different face than the one they would
        // be attached to.
        if let [face] = new_faces.as_slice() {
            if default_family.is_none() {
                default_family = face.families.first().map(|(n, _)| n.clone());
            }
            let known: Vec<String> = face
                .families
                .iter()
                .map(|(n, _)| n.to_ascii_lowercase())
                .collect();
            for alias in collect_font_name_aliases(bytes) {
                if known.contains(&alias.to_ascii_lowercase())
                    || registered_families
                        .iter()
                        .any(|f| f.eq_ignore_ascii_case(&alias))
                {
                    continue;
                }
                registered_families.push(alias.clone());
                if default_family.is_none() {
                    default_family = Some(alias.clone());
                }
                options.fontdb_mut().push_face_info(fontdb::FaceInfo {
                    id: fontdb::ID::dummy(),
                    source: face.source.clone(),
                    index: face.index,
                    families: vec![(alias, fontdb::Language::English_UnitedStates)],
                    post_script_name: face.post_script_name.clone(),
                    style: face.style,
                    weight: face.weight,
                    stretch: face.stretch,
                    monospaced: face.monospaced,
                });
            }
        } else if default_family.is_none() {
            default_family = new_faces
                .first()
                .and_then(|f| f.families.first().map(|(n, _)| n.clone()));
        }
    }

    if let Some(family) = default_family {
        options.font_family = family;
    }

    registered_families
}

/// Extracts alternative names — legacy family (name ID 1), full name (ID 4)
/// and PostScript name (ID 6) — from a TrueType/OpenType font's `name` table.
/// Returns an empty list on any structural problem; this is only used to add
/// lookup aliases, never to reject a font.
fn collect_font_name_aliases(data: &[u8]) -> Vec<String> {
    fn u16_at(d: &[u8], o: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
    }
    fn u32_at(d: &[u8], o: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            *d.get(o)?,
            *d.get(o + 1)?,
            *d.get(o + 2)?,
            *d.get(o + 3)?,
        ]))
    }

    let mut out: Vec<String> = Vec::new();
    let mut push_unique = |s: String| {
        let s = s.trim().to_string();
        if !s.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    };

    let mut parse = || -> Option<()> {
        // Font collections: jump to the first font's offset table.
        let sfnt_start = if data.get(0..4)? == b"ttcf" {
            u32_at(data, 12)? as usize
        } else {
            0
        };

        let num_tables = u16_at(data, sfnt_start + 4)? as usize;
        let mut name_table = None;
        for i in 0..num_tables {
            let rec = sfnt_start + 12 + i * 16;
            if data.get(rec..rec + 4)? == b"name" {
                name_table = Some(u32_at(data, rec + 8)? as usize);
                break;
            }
        }
        let name_table = name_table?;

        let count = u16_at(data, name_table + 2)? as usize;
        let string_storage = name_table + u16_at(data, name_table + 4)? as usize;

        for i in 0..count {
            let rec = name_table + 6 + i * 12;
            let platform = u16_at(data, rec)?;
            let name_id = u16_at(data, rec + 6)?;
            if !matches!(name_id, 1 | 4 | 6) {
                continue;
            }
            let len = u16_at(data, rec + 8)? as usize;
            let off = string_storage + u16_at(data, rec + 10)? as usize;
            let bytes = data.get(off..off + len)?;
            match platform {
                // Unicode / Windows: UTF-16 BE
                0 | 3 => {
                    let units: Vec<u16> = bytes
                        .chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect();
                    if let Ok(s) = String::from_utf16(&units) {
                        push_unique(s);
                    }
                }
                // Macintosh: treat as Latin-1 (correct for the ASCII names
                // fonts put there in practice)
                1 => {
                    push_unique(bytes.iter().map(|&b| b as char).collect());
                }
                _ => {}
            }
        }
        Some(())
    };
    let _ = parse();

    out
}

/// Number of `<text` element starts in the raw SVG source. `<textPath>` (the
/// only other `text*` element; it can only occur *inside* `<text>`) is not
/// counted.
fn count_svg_text_elements(svg: &str) -> usize {
    let bytes = svg.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while let Some(pos) = svg[i..].find("<text") {
        let after = i + pos + "<text".len();
        match bytes.get(after) {
            Some(c) if c.is_ascii_alphanumeric() => {}
            _ => count += 1,
        }
        i = after;
    }
    count
}

/// Number of text nodes that survived usvg parsing (text with unresolvable
/// fonts is dropped by usvg during conversion).
fn count_tree_text_nodes(group: &usvg::Group) -> usize {
    let mut n = 0;
    for child in group.children() {
        match child {
            usvg::Node::Text(_) => n += 1,
            usvg::Node::Group(g) => n += count_tree_text_nodes(g),
            _ => {}
        }
    }
    n
}

/// Reads the (possibly inherited) `/MediaBox` of `page_id` as
/// `[min_x, min_y, max_x, max_y]`.
fn get_media_box(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<[f32; 4]> {
    let mut dict = doc.get_dictionary(page_id).ok()?;
    for _ in 0..MAX_INLINE_DEPTH {
        if let Ok(mb) = dict.get(b"MediaBox") {
            let mb = match mb {
                lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
                other => other,
            };
            let arr = mb.as_array().ok()?;
            if arr.len() != 4 {
                return None;
            }
            let mut vals = [0.0_f32; 4];
            for (i, obj) in arr.iter().enumerate() {
                vals[i] = lopdf_number(obj)?;
            }
            return Some([
                vals[0].min(vals[2]),
                vals[1].min(vals[3]),
                vals[0].max(vals[2]),
                vals[1].max(vals[3]),
            ]);
        }
        // MediaBox is inheritable via the page tree
        let parent = dict.get(b"Parent").ok()?.as_reference().ok()?;
        dict = doc.get_dictionary(parent).ok()?;
    }
    None
}

fn lopdf_number(obj: &lopdf::Object) -> Option<f32> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f32),
        lopdf::Object::Real(r) => Some(*r),
        _ => None,
    }
}

/// Recursively converts a lopdf object from the svg2pdf-generated document
/// into a self-contained [`DictItem`], resolving and inlining every indirect
/// reference on the way. Dangling references become `Null` (with a warning)
/// instead of pointing into an object space that will not exist in the final
/// document.
fn inline_lopdf_object(
    doc: &lopdf::Document,
    obj: &lopdf::Object,
    depth: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> DictItem {
    use lopdf::Object;

    if depth > MAX_INLINE_DEPTH {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            "svg: resource tree exceeds maximum depth (cyclic reference?), truncating".to_string(),
        ));
        return DictItem::Null;
    }

    match obj {
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(resolved) => inline_lopdf_object(doc, resolved, depth + 1, warnings),
            Err(e) => {
                warnings.push(PdfWarnMsg::warning(
                    0,
                    0,
                    format!("svg: unresolvable reference {id:?} in svg2pdf output: {e}"),
                ));
                DictItem::Null
            }
        },
        Object::Array(items) => DictItem::Array(
            items
                .iter()
                .map(|o| inline_lopdf_object(doc, o, depth + 1, warnings))
                .collect(),
        ),
        Object::Dictionary(d) => DictItem::Dict {
            map: d
                .iter()
                .map(|(k, v)| {
                    (
                        String::from_utf8_lossy(k).to_string(),
                        inline_lopdf_object(doc, v, depth + 1, warnings),
                    )
                })
                .collect(),
        },
        Object::Stream(s) => {
            // Prefer decoded content so the written file controls its own
            // filters; keep the raw bytes plus the original /Filter entries
            // for filters lopdf cannot decode (e.g. DCTDecode images).
            let (content, strip_filters) = match s.decompressed_content() {
                Ok(c) => (c, true),
                Err(_) => (s.content.clone(), false),
            };
            let dict = s
                .dict
                .iter()
                .filter(|(k, _)| {
                    let k = k.as_slice();
                    // Length is recomputed on write
                    k != b"Length" && (!strip_filters || (k != b"Filter" && k != b"DecodeParms"))
                })
                .map(|(k, v)| {
                    (
                        String::from_utf8_lossy(k).to_string(),
                        inline_lopdf_object(doc, v, depth + 1, warnings),
                    )
                })
                .collect();
            DictItem::Stream {
                stream: ExternalStream {
                    dict,
                    content,
                    compress: false,
                },
            }
        }
        other => DictItem::from_lopdf(other),
    }
}
