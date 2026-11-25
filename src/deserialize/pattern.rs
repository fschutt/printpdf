//! Patterns read from a PDF.
//!
//! A pattern (ISO 32000-1, 8.7) is a color that is a repeated drawing (a tiling pattern, a
//! stream) or a gradient (a shading pattern, a dictionary). A page or form lists its patterns
//! under `/Pattern` in its resources and selects one with `/Pattern cs /P1 scn`.
//!
//! [`read_pattern`] reads one pattern into a [`Pattern`]. It is modelled as [`Pattern::Tiling`]
//! or [`Pattern::Shading`] only when writing the model again loses nothing:
//!
//! - A tiling pattern has `/PaintType` 1 or 2, `/TilingType` 1, a usable `/BBox`, a non-zero
//!   `/XStep` and `/YStep`, and a content stream that decodes to ops. Its `/Resources` have only
//!   fonts, graphics states and shadings that are modelled. A pattern with other resources
//!   (nested patterns, XObjects, color spaces), with an operator that has no [`Op`], or with
//!   a font that has no font program is not modelled.
//! - A shading pattern has an axial or radial shading with the color space DeviceRGB or
//!   DeviceGray and exponential functions (type 2) or type 3 functions of them that [`Shading`]
//!   holds exactly. A mesh shading (a stream) is not modelled.
//!
//! Anything else is kept as [`Pattern::External`] with its references resolved, within the same
//! budget that preserved XObjects use, and an info message says why. A pattern that cannot be
//! read at all (it is not a dictionary or stream, or a limit is reached) is an error of that
//! entry, and the caller decides what that means.
//!
//! Hostile input is bounded. A document has at most [`MAX_PATTERNS`] patterns and
//! [`MAX_PATTERN_CONTENT_BYTES`] bytes of pattern content, counted again when a pattern object is
//! used under another name. A pattern is read once per object. Nothing is read recursively: a
//! pattern that names patterns in its own resources is kept as it is, and the resolving of
//! references stops at a cycle.

use std::collections::{BTreeMap, BTreeSet};

use lopdf::{
    Dictionary as LopdfDictionary, Document as LopdfDocument, Object, ObjectId,
    Stream as LopdfStream,
};

use super::{
    convert_operations, decode_content, decode_limited, dictitem_from_lopdf_resolved,
    font_resources, get_dict_or_resolve_ref, parse_rect, parse_shading_dict, parsefont,
    read_extgstates, read_shadings, Decoded, InlineBudget, PdfWarnMsg, MAX_STREAM_BYTES,
    MAX_INLINED_BYTES, MAX_INLINED_NODES,
};
use crate::{
    pattern::{PatternPaintType, IDENTITY_MATRIX},
    xobject::DictItem,
    ExtendedGraphicsState, ExtendedGraphicsStateId, FontId, Op, Pattern, PatternId, PdfResources,
    Shading, ShadingId,
};

/// Most patterns that are read from one document; the others are skipped.
pub(super) const MAX_PATTERNS: usize = 100_000;

/// Most bytes of decoded content that the patterns of one document are read from. A pattern
/// object that is read again under another name counts again.
pub(super) const MAX_PATTERN_CONTENT_BYTES: usize = 256 * 1024 * 1024;

/// Most entries of a `/ColorSpace` category that are looked at for pattern color spaces.
const MAX_COLOR_SPACES: usize = 100_000;

/// The entries of a tiling pattern's dictionary that are modelled.
const TILING_KEYS: &[&[u8]] = &[
    b"Type",
    b"PatternType",
    b"PaintType",
    b"TilingType",
    b"BBox",
    b"XStep",
    b"YStep",
    b"Resources",
    b"Matrix",
    b"Length",
    b"Filter",
    b"DecodeParms",
];

/// The entries of a shading pattern's dictionary that are modelled.
const SHADING_KEYS: &[&[u8]] = &[b"Type", b"PatternType", b"Shading", b"Matrix", b"ExtGState"];

/// The resource categories of a tiling pattern that are modelled. `/ProcSet` is obsolete and
/// has no effect, so it is dropped.
const TILING_RESOURCE_KEYS: &[&[u8]] = &[b"Font", b"ExtGState", b"Shading", b"ProcSet"];

/// What reading the patterns of one document keeps from one pattern to the next.
pub(crate) struct PatternCtx {
    /// What is left of the budget for the references that are copied into preserved patterns.
    budget: InlineBudget,
    /// The patterns that are read already, by object, with the bytes they count for.
    cache: BTreeMap<ObjectId, (Pattern, usize)>,
    patterns_left: usize,
    bytes_left: usize,
    reported_limit: bool,
}

impl PatternCtx {
    pub(crate) fn new() -> Self {
        PatternCtx {
            budget: InlineBudget::new(),
            cache: BTreeMap::new(),
            patterns_left: MAX_PATTERNS,
            bytes_left: MAX_PATTERN_CONTENT_BYTES,
            reported_limit: false,
        }
    }
}

/// The names in the `/ColorSpace` category of `resources` that are `[/Pattern ...]` color
/// spaces. `/Name cs` with one of them selects a pattern, as `/Pattern cs` does.
pub(super) fn pattern_space_names(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(category) = resources.get(b"ColorSpace") else {
        return names;
    };
    let Some(category) =
        get_dict_or_resolve_ref("pattern color spaces", doc, category, &mut Vec::new(), None)
    else {
        return names;
    };
    for (name, value) in category.iter().take(MAX_COLOR_SPACES) {
        if let Object::Array(array) = deref(doc, value) {
            if matches!(array.first(), Some(Object::Name(family)) if family.as_slice() == b"Pattern")
            {
                names.insert(String::from_utf8_lossy(name).into_owned());
            }
        }
    }
    names
}

/// The patterns of the `/Pattern` category of `resources`, a scope that must be read without
/// loss (the resources of a form). `Err` says why one entry cannot be read.
pub(super) fn read_scope_patterns(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
    page: usize,
    ctx: &mut PatternCtx,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<BTreeMap<PatternId, Pattern>, String> {
    let mut patterns = BTreeMap::new();
    let Ok(category) = resources.get(b"Pattern") else {
        return Ok(patterns);
    };
    let category = get_dict_or_resolve_ref("form /Pattern", doc, category, warnings, Some(page))
        .ok_or_else(|| "its /Pattern resource is not a dictionary".to_string())?;
    for (key, value) in category.iter() {
        let name = String::from_utf8_lossy(key).into_owned();
        let pattern = read_pattern(doc, value, &name, page, ctx, warnings)
            .map_err(|e| format!("its pattern {name} cannot be read: {e}"))?;
        patterns.insert(PatternId(name), pattern);
    }
    Ok(patterns)
}

/// Collects the `/Pattern` resources of the document root, the page tree and every page into one
/// map. The first definition of a name wins, as for shadings and fonts.
pub(super) fn extract_patterns(
    doc: &LopdfDocument,
    objs_to_search_for_resources: &[(Object, Option<usize>)],
    warnings: &mut Vec<PdfWarnMsg>,
) -> BTreeMap<PatternId, Pattern> {
    let mut ctx = PatternCtx::new();
    let mut map: BTreeMap<PatternId, Pattern> = BTreeMap::new();
    // The object each name was read from, to tell a name that two pages define differently.
    let mut origin: BTreeMap<PatternId, Option<ObjectId>> = BTreeMap::new();
    for (obj, page_idx) in objs_to_search_for_resources {
        let page = page_idx.unwrap_or(0);
        let dict = match obj {
            Object::Dictionary(d) => Some(d),
            Object::Reference(r) => doc.get_dictionary(*r).ok(),
            _ => None,
        };
        let Some(dict) = dict else { continue };
        let resources = match dict.get(b"Resources") {
            Ok(res) => get_dict_or_resolve_ref(
                &format!("extract_patterns Resources page {page}"),
                doc,
                res,
                warnings,
                Some(page),
            ),
            Err(_) => Some(dict),
        };
        let Some(resources) = resources else { continue };
        let Ok(category) = resources.get(b"Pattern") else {
            continue;
        };
        let Some(category) = get_dict_or_resolve_ref(
            &format!("extract_patterns Pattern page {page}"),
            doc,
            category,
            warnings,
            Some(page),
        ) else {
            continue;
        };
        for (key, value) in category.iter() {
            let name = String::from_utf8_lossy(key).into_owned();
            let id = PatternId(name.clone());
            let this_origin = match value {
                Object::Reference(r) => Some(*r),
                _ => None,
            };
            if let Some(first) = origin.get(&id) {
                if first.is_some() && this_origin.is_some() && *first != this_origin {
                    warnings.push(PdfWarnMsg::info(
                        page,
                        0,
                        format!(
                            "pattern {name} is a different object on this page; the first \
                             definition is used for every page"
                        ),
                    ));
                }
                continue;
            }
            match read_pattern(doc, value, &name, page, &mut ctx, warnings) {
                Ok(pattern) => {
                    origin.insert(id.clone(), this_origin);
                    map.insert(id, pattern);
                }
                Err(e) => warnings.push(PdfWarnMsg::warning(
                    page,
                    0,
                    format!("pattern {name} not preserved: {e}"),
                )),
            }
        }
    }
    map
}

/// Reads the pattern `value`, an entry of a `/Pattern` dictionary, called `name`.
fn read_pattern(
    doc: &LopdfDocument,
    value: &Object,
    name: &str,
    page: usize,
    ctx: &mut PatternCtx,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Pattern, String> {
    let id = match value {
        Object::Reference(id) => Some(*id),
        _ => None,
    };
    if let Some(id) = id {
        if let Some((pattern, cost)) = ctx.cache.get(&id) {
            let cost = *cost;
            if cost > ctx.bytes_left {
                return Err(limit_reached(ctx, name, page, warnings));
            }
            ctx.bytes_left = ctx.bytes_left.saturating_sub(cost);
            return Ok(pattern.clone());
        }
    }
    if ctx.patterns_left == 0 || ctx.bytes_left == 0 {
        return Err(limit_reached(ctx, name, page, warnings));
    }
    ctx.patterns_left = ctx.patterns_left.saturating_sub(1);

    let object = match value {
        Object::Reference(r) => doc
            .get_object(*r)
            .map_err(|e| format!("the reference {r:?} does not resolve: {e}"))?,
        other => other,
    };
    let (dict, stream) = match object {
        Object::Stream(stream) => (&stream.dict, Some(stream)),
        Object::Dictionary(dict) => (dict, None),
        _ => return Err("it is neither a dictionary nor a stream".to_string()),
    };

    let typed = match (integer(doc, dict, b"PatternType"), stream) {
        (Some(1), Some(stream)) => read_tiling(doc, stream, page, ctx, warnings),
        (Some(1), None) => Err("a tiling pattern has to be a stream".to_string()),
        (Some(2), None) => read_shading_pattern(doc, dict, page, warnings).map(|p| (p, 0)),
        (Some(2), Some(_)) => Err("a shading pattern has to be a dictionary".to_string()),
        _ => Err("its /PatternType is not 1 or 2".to_string()),
    };
    let (pattern, cost) = match typed {
        Ok(read) => read,
        Err(reason) => {
            warnings.push(PdfWarnMsg::info(
                page,
                0,
                format!("pattern {name} is kept as it is: {reason}"),
            ));
            let item =
                dictitem_from_lopdf_resolved(doc, value, 0, &mut Vec::new(), &mut ctx.budget);
            if ctx.budget.exhausted && !ctx.budget.reported {
                ctx.budget.reported = true;
                warnings.push(PdfWarnMsg::warning(
                    page,
                    0,
                    format!(
                        "pattern {name}: the references inside it exceed the limit for one \
                         document ({MAX_INLINED_NODES} objects or {MAX_INLINED_BYTES} bytes); \
                         the rest are replaced by null"
                    ),
                ));
            }
            let cost = item_bytes(&item, 0);
            (Pattern::External(item), cost)
        }
    };
    if let Some(id) = id {
        ctx.cache.insert(id, (pattern.clone(), cost));
    }
    Ok(pattern)
}

/// The message for a pattern that is not read because a limit is reached, written once.
fn limit_reached(
    ctx: &mut PatternCtx,
    name: &str,
    page: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> String {
    if !ctx.reported_limit {
        ctx.reported_limit = true;
        warnings.push(PdfWarnMsg::warning(
            page,
            0,
            format!(
                "the document has more than {MAX_PATTERNS} patterns or {MAX_PATTERN_CONTENT_BYTES} \
                 bytes of pattern content; the patterns from {name} on are skipped"
            ),
        ));
    }
    "the limit for patterns of one document is reached".to_string()
}

/// The bytes of the streams and strings in `item`, to count a preserved pattern when it is
/// copied again.
fn item_bytes(item: &DictItem, depth: usize) -> usize {
    // The depth of a preserved object is bounded already; this only keeps the recursion short.
    if depth > 96 {
        return 0;
    }
    match item {
        DictItem::Array(items) => items
            .iter()
            .fold(0usize, |sum, i| sum.saturating_add(item_bytes(i, depth + 1))),
        DictItem::Dict { map } => map
            .values()
            .fold(0usize, |sum, i| sum.saturating_add(item_bytes(i, depth + 1))),
        DictItem::Stream { stream } => stream.dict.values().fold(stream.content.len(), |sum, i| {
            sum.saturating_add(item_bytes(i, depth + 1))
        }),
        DictItem::String { data, .. } | DictItem::Bytes(data) => data.len(),
        _ => 0,
    }
}

/// The tiling pattern `stream` and the bytes of its content, or why it is not modelled.
fn read_tiling(
    doc: &LopdfDocument,
    stream: &LopdfStream,
    page: usize,
    ctx: &mut PatternCtx,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<(Pattern, usize), String> {
    let dict = &stream.dict;
    check_keys(dict, TILING_KEYS, "its dictionary")?;

    let paint_type = match integer(doc, dict, b"PaintType") {
        Some(1) => PatternPaintType::Colored,
        Some(2) => PatternPaintType::Uncolored,
        _ => return Err("its /PaintType is not 1 or 2".to_string()),
    };
    if integer(doc, dict, b"TilingType") != Some(1) {
        return Err("its /TilingType is not 1 (constant spacing)".to_string());
    }
    let bbox = dict
        .get(b"BBox")
        .ok()
        .and_then(|bbox| parse_rect(deref(doc, bbox)).ok())
        .ok_or_else(|| "it has no usable /BBox".to_string())?;
    if ![bbox.x.0, bbox.y.0, bbox.width.0, bbox.height.0]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err("its /BBox is not finite".to_string());
    }
    let step = |key: &[u8]| {
        dict.get(key)
            .ok()
            .and_then(|o| number(deref(doc, o)))
            .filter(|v| v.is_finite() && *v != 0.0)
    };
    let xstep = step(b"XStep").ok_or_else(|| "it has no usable /XStep".to_string())?;
    let ystep = step(b"YStep").ok_or_else(|| "it has no usable /YStep".to_string())?;
    let matrix = read_matrix(doc, dict)?;

    let limit = ctx.bytes_left.min(MAX_STREAM_BYTES);
    let content = match decode_limited(stream, limit) {
        Decoded::Data(data) => data,
        Decoded::Undecodable => return Err("its content stream cannot be decoded".to_string()),
        Decoded::TooLarge => return Err("its content stream is too large".to_string()),
    };
    ctx.bytes_left = ctx.bytes_left.saturating_sub(content.len());
    let operations = decode_content(&content)?;

    let mut scope = match dict.get(b"Resources") {
        Err(_) => None,
        Ok(resources) => {
            let resources =
                get_dict_or_resolve_ref("pattern /Resources", doc, resources, &mut Vec::new(), None)
                    .ok_or_else(|| "its /Resources is not a dictionary".to_string())?;
            Some(read_tiling_scope(doc, resources, page, warnings)?)
        }
    };
    let font_decode = match &scope {
        Some(scope) => scope
            .fonts
            .iter()
            .map(|(id, font)| (id.0.clone(), font.decode_kind()))
            .collect(),
        None => BTreeMap::new(),
    };
    let ops = convert_operations(
        page,
        &operations,
        &BTreeMap::new(),
        &font_decode,
        &BTreeSet::new(),
        warnings,
    )?;
    let fonts = match scope.as_mut() {
        Some(scope) => font_resources(std::mem::take(&mut scope.fonts).into_iter()),
        None => BTreeMap::new(),
    };
    check_content(&ops, scope.as_ref(), &fonts)?;

    // Resources without an entry are the same as no resources.
    let resources = scope
        .map(|scope| PdfResources {
            fonts: crate::PdfFontMap { map: fonts },
            extgstates: crate::ExtendedGraphicsStateMap {
                map: scope.extgstates,
            },
            shadings: crate::ShadingMap { map: scope.shadings },
            ..PdfResources::default()
        })
        .filter(|resources| *resources != PdfResources::default());
    let cost = content.len();
    Ok((
        Pattern::Tiling {
            paint_type,
            bbox,
            xstep,
            ystep,
            matrix,
            ops,
            resources,
        },
        cost,
    ))
}

/// The resources of a tiling pattern.
struct TilingScope {
    fonts: BTreeMap<FontId, super::ParsedOrBuiltinFont>,
    extgstates: BTreeMap<ExtendedGraphicsStateId, ExtendedGraphicsState>,
    shadings: BTreeMap<ShadingId, Shading>,
}

/// Reads `/Resources` of a tiling pattern, or says what in it is not modelled.
fn read_tiling_scope(
    doc: &LopdfDocument,
    resources: &LopdfDictionary,
    page: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<TilingScope, String> {
    check_keys(resources, TILING_RESOURCE_KEYS, "its /Resources")?;

    let fonts =
        parsefont::parse_fonts_enhanced(doc, resources, warnings, Some(page), &BTreeMap::new());

    // The problems of these two are reported by the reason that the pattern is kept as it is.
    let mut quiet = Vec::new();
    let extgstates = read_extgstates(doc, resources, page, &mut quiet).unwrap_or_default();
    if let Some(category) = category_dict(doc, resources, b"ExtGState") {
        if category.len() != extgstates.len() {
            return Err("it has a graphics state that is not a dictionary".to_string());
        }
        for (key, state) in &extgstates {
            let source = category
                .get(key.0.as_bytes())
                .ok()
                .and_then(|entry| dict_of(doc, entry));
            match source {
                Some(source) if extgstate_is_exact(source, state) => {}
                _ => return Err(format!("its graphics state {} has entries that are not modelled", key.0)),
            }
        }
    }

    let mut shadings = BTreeMap::new();
    read_shadings(doc, resources, page, &mut shadings, &mut quiet);
    if let Some(category) = category_dict(doc, resources, b"Shading") {
        if category.len() != shadings.len() {
            return Err("it has a shading that is not modelled".to_string());
        }
        for (key, shading) in &shadings {
            let source = category
                .get(key.0.as_bytes())
                .ok()
                .and_then(|entry| dict_of(doc, entry));
            match source {
                Some(source) if shading_is_exact(doc, source, shading) => {}
                _ => return Err(format!("its shading {} is not modelled exactly", key.0)),
            }
        }
    }

    Ok(TilingScope {
        fonts,
        extgstates,
        shadings,
    })
}

/// Checks that writing the ops of a tiling pattern again names only what the scope holds.
fn check_content(
    ops: &[Op],
    scope: Option<&TilingScope>,
    fonts: &BTreeMap<FontId, crate::font::PdfFont>,
) -> Result<(), String> {
    for op in ops {
        match op {
            Op::Unknown { .. } => return Err("its content has an operator that no Op models".to_string()),
            Op::BeginInlineImage | Op::BeginInlineImageData | Op::EndInlineImage => {
                return Err("its content has an inline image".to_string());
            }
            Op::UseXobject { .. } => return Err("its content draws an XObject".to_string()),
            Op::SetFillPattern { .. } | Op::SetOutlinePattern { .. } => {
                return Err("its content selects a pattern".to_string());
            }
            Op::BeginLayer { .. } | Op::BeginOptionalContent { .. } => {
                return Err("its content has optional content".to_string());
            }
            Op::BeginMarkedContentWithProperties {
                properties: DictItem::Name(_),
                ..
            } => return Err("its content names marked-content properties".to_string()),
            Op::SetColorSpaceFill { id } | Op::SetColorSpaceStroke { id } => {
                if !matches!(id.as_str(), "DeviceRGB" | "DeviceGray" | "DeviceCMYK") {
                    return Err(format!("its content selects the color space {id}"));
                }
            }
            Op::LoadGraphicsState { gs } => {
                if !scope.is_some_and(|s| s.extgstates.contains_key(gs)) {
                    return Err(format!("its content names the graphics state {}, which it has not", gs.0));
                }
            }
            Op::PaintShading { id } => {
                if !scope.is_some_and(|s| s.shadings.contains_key(id)) {
                    return Err(format!("its content names the shading {}, which it has not", id.0));
                }
            }
            Op::SetFont {
                font: crate::ops::PdfFontHandle::External(id),
                ..
            } => {
                if !fonts.contains_key(id) {
                    return Err(format!("it selects the font {}, which has no font program", id.0));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The shading pattern in `dict`, or why it is not modelled.
fn read_shading_pattern(
    doc: &LopdfDocument,
    dict: &LopdfDictionary,
    page: usize,
    warnings: &mut Vec<PdfWarnMsg>,
) -> Result<Pattern, String> {
    check_keys(dict, SHADING_KEYS, "its dictionary")?;
    let shading_object = dict
        .get(b"Shading")
        .map_err(|_| "it has no /Shading".to_string())?;
    let Object::Dictionary(shading_dict) = deref(doc, shading_object) else {
        return Err("its shading is not a dictionary (a mesh shading is a stream)".to_string());
    };
    let shading = parse_shading_dict(doc, shading_dict)?;
    if !shading_is_exact(doc, shading_dict, &shading) {
        return Err("its shading has entries that are not modelled".to_string());
    }
    let matrix = read_matrix(doc, dict)?;
    let ext_gstate = match dict.get(b"ExtGState") {
        Err(_) => None,
        Ok(entry) => {
            let source = get_dict_or_resolve_ref("pattern /ExtGState", doc, entry, warnings, Some(page))
                .ok_or_else(|| "its /ExtGState is not a dictionary".to_string())?;
            let state = super::extgstate::parse_extgstate(source);
            if !extgstate_is_exact(source, &state) {
                return Err("its /ExtGState has entries that are not modelled".to_string());
            }
            Some(state)
        }
    };
    Ok(Pattern::Shading {
        shading,
        matrix,
        ext_gstate,
    })
}

/// Whether writing `state` gives every entry of `source`.
fn extgstate_is_exact(source: &LopdfDictionary, state: &ExtendedGraphicsState) -> bool {
    let written = crate::graphics::extgstate_to_dict(state);
    source
        .iter()
        .all(|(key, _)| key.as_slice() == b"Type" || written.has(key))
}

/// Whether writing `shading` gives what `source` says: no entries beyond the ones that are
/// written, the color space DeviceRGB or DeviceGray, and functions that are read exactly.
fn shading_is_exact(doc: &LopdfDocument, source: &LopdfDictionary, shading: &Shading) -> bool {
    let written = shading.to_dict();
    if !source.iter().all(|(key, _)| written.has(key)) {
        return false;
    }
    let device_color = matches!(
        source.get(b"ColorSpace").ok().map(|o| deref(doc, o)),
        Some(Object::Name(name)) if name.as_slice() == b"DeviceRGB" || name.as_slice() == b"DeviceGray"
    );
    let function = source
        .get(b"Function")
        .ok()
        .and_then(|o| dict_of(doc, o))
        .is_some_and(|f| function_is_exact(doc, f, true));
    device_color && function
}

/// Whether the function `f` is one that [`Shading`] holds exactly: exponential with exponent 1
/// and one or three components on `[0 1]`, or a type 3 function of such functions over `[0 1]`.
fn function_is_exact(doc: &LopdfDocument, f: &LopdfDictionary, allow_type3: bool) -> bool {
    let unit = |key: &[u8]| match f.get(key).ok().map(|o| deref(doc, o)) {
        None => true,
        Some(Object::Array(a)) => {
            a.len() >= 2
                && a.iter().enumerate().all(|(i, v)| {
                    number(deref(doc, v)) == Some(if i % 2 == 0 { 0.0 } else { 1.0 })
                })
        }
        _ => false,
    };
    match integer(doc, f, b"FunctionType") {
        Some(2) => {
            let components = |key: &[u8]| match f.get(key).ok().map(|o| deref(doc, o)) {
                Some(Object::Array(a)) => Some(a.len()),
                _ => None,
            };
            let c0 = components(b"C0");
            let exponent = f.get(b"N").ok().and_then(|o| number(deref(doc, o)));
            unit(b"Domain")
                && exponent == Some(1.0)
                && c0.is_some()
                && c0 == components(b"C1")
                && matches!(c0, Some(1) | Some(3))
        }
        Some(3) if allow_type3 => {
            let Some(Object::Array(functions)) = f.get(b"Functions").ok().map(|o| deref(doc, o))
            else {
                return false;
            };
            unit(b"Domain")
                && unit(b"Encode")
                && !functions.is_empty()
                && functions
                    .iter()
                    .all(|sub| dict_of(doc, sub).is_some_and(|d| function_is_exact(doc, d, false)))
        }
        _ => false,
    }
}

/// `/Matrix` as six numbers, the identity when there is none.
fn read_matrix(doc: &LopdfDocument, dict: &LopdfDictionary) -> Result<[f32; 6], String> {
    let Ok(object) = dict.get(b"Matrix") else {
        return Ok(IDENTITY_MATRIX);
    };
    let Object::Array(entries) = deref(doc, object) else {
        return Err("its /Matrix is not an array".to_string());
    };
    let mut matrix = [0.0f32; 6];
    if entries.len() != matrix.len() {
        return Err("its /Matrix is not six numbers".to_string());
    }
    for (slot, entry) in matrix.iter_mut().zip(entries) {
        *slot = number(deref(doc, entry))
            .filter(|v| v.is_finite())
            .ok_or_else(|| "its /Matrix is not six finite numbers".to_string())?;
    }
    Ok(matrix)
}

/// Whether every key of `dict` is in `allowed`.
fn check_keys(dict: &LopdfDictionary, allowed: &[&[u8]], what: &str) -> Result<(), String> {
    match dict
        .iter()
        .map(|(key, _)| key)
        .find(|key| !allowed.contains(&key.as_slice()))
    {
        Some(key) => Err(format!(
            "{what} has the entry /{}, which is not modelled",
            String::from_utf8_lossy(key)
        )),
        None => Ok(()),
    }
}

/// The dictionary of the resource category `key`.
fn category_dict<'a>(
    doc: &'a LopdfDocument,
    resources: &'a LopdfDictionary,
    key: &[u8],
) -> Option<&'a LopdfDictionary> {
    dict_of(doc, resources.get(key).ok()?)
}

/// The dictionary behind `object`, which is a dictionary, a stream or a reference to one.
fn dict_of<'a>(doc: &'a LopdfDocument, object: &'a Object) -> Option<&'a LopdfDictionary> {
    match deref(doc, object) {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream(stream) => Some(&stream.dict),
        _ => None,
    }
}

/// The object `object` refers to, one level deep.
fn deref<'a>(doc: &'a LopdfDocument, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(object),
        other => other,
    }
}

/// An integer or real number.
fn number(object: &Object) -> Option<f32> {
    match object {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

/// The integer entry `key` of `dict`.
fn integer(doc: &LopdfDocument, dict: &LopdfDictionary, key: &[u8]) -> Option<i64> {
    dict.get(key)
        .ok()
        .and_then(|object| deref(doc, object).as_i64().ok())
}
