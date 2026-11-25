//! Content streams and what they use: resources, fonts, images, forms, graphics states, colour.

use std::collections::HashSet;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

use super::colour::{self, Space};
use super::util::{decode, filter_names, look, name_text, resolve, Budget, Decoded, Look};
use super::{Loc, Run, Seen, Severity};

/// The largest content stream that is parsed. A parsed operator takes far more memory than its
/// bytes in the stream.
pub(crate) const MAX_PARSE_BYTES: usize = 16 * 1024 * 1024;

/// The 14 fonts that a PDF reader has without a font file.
const STANDARD_14: [&str; 14] = [
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
    "Symbol",
    "ZapfDingbats",
];

/// A resource looked up by category and name.
enum Res<'a> {
    /// The resources have no such category or no such name.
    NotFound,
    /// The entry is `null`.
    Null,
    /// The entry refers to an object that does not exist.
    Dangling,
    Found(Option<ObjectId>, &'a Object),
}

fn resource<'a>(
    doc: &'a Document,
    resources: Option<&'a Dictionary>,
    category: &str,
    name: &[u8],
) -> Res<'a> {
    let Some(res) = resources else {
        return Res::NotFound;
    };
    let Some(cat) = look(doc, res, category).dict() else {
        return Res::NotFound;
    };
    match cat.get(name) {
        Err(_) => Res::NotFound,
        Ok(o) => match resolve(doc, o) {
            Look::Absent => Res::Null,
            Look::Dangling(_) => Res::Dangling,
            Look::Found(id, o) => Res::Found(id, o),
        },
    }
}

/// The categories of a resource dictionary and the kind of object each entry must be.
const CATEGORIES: [&str; 7] = [
    "Font",
    "XObject",
    "ExtGState",
    "ColorSpace",
    "Pattern",
    "Shading",
    "Properties",
];

/// Checks what a resource dictionary declares: that every entry exists and has the right kind
/// of object, and that every colour space is well formed.
pub(crate) fn check_resources<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    res: &'a Dictionary,
    id: Option<ObjectId>,
) {
    match id {
        Some(id) => {
            if !run.first_visit(Seen::Resources, id) {
                return;
            }
        }
        // A resource dictionary written directly in a page tree node is inherited by every
        // page below it; it is the same dictionary, so it is read once.
        None => {
            if !run.seen_dicts.insert(std::ptr::from_ref(res) as usize) {
                return;
            }
        }
    }
    let doc = run.doc;
    let loc = Loc::at(page, id);
    for category in CATEGORIES {
        let cat = match look(doc, res, category) {
            Look::Absent => continue,
            Look::Dangling(missing) => {
                run.sink.error(
                    "resource-dangling",
                    loc,
                    format!(
                        "/{category} of the resources refers to object {} {}, which does not exist",
                        missing.0, missing.1
                    ),
                );
                continue;
            }
            Look::Found(_, Object::Dictionary(d)) => d,
            Look::Found(_, other) => {
                run.sink.error(
                    "resources-malformed",
                    loc,
                    format!(
                        "/{category} of the resources is {}, not a dictionary",
                        other.enum_variant()
                    ),
                );
                continue;
            }
        };
        for (name, value) in cat.iter() {
            if !run.visit() {
                return;
            }
            let shown = name_text(name);
            match resolve(doc, value) {
                Look::Dangling(missing) => run.sink.error(
                    "resource-dangling",
                    loc,
                    format!(
                        "/{category} /{shown} refers to object {} {}, which does not exist",
                        missing.0, missing.1
                    ),
                ),
                Look::Absent => run.sink.error(
                    "resource-dangling",
                    loc,
                    format!("/{category} /{shown} is null"),
                ),
                Look::Found(_, entry) => {
                    if category == "ColorSpace" {
                        match name.as_slice() {
                            b"DefaultGray" => run.default_spaces[0] = true,
                            b"DefaultRGB" => run.default_spaces[1] = true,
                            b"DefaultCMYK" => run.default_spaces[2] = true,
                            _ => {}
                        }
                        let cs_loc = Loc::at(page, id);
                        colour::analyse(run, value, cs_loc, false, 0);
                    } else {
                        let right_kind = match category {
                            "XObject" => matches!(entry, Object::Stream(_)),
                            "Font" | "ExtGState" | "Properties" => super::dict_of(entry).is_some(),
                            _ => super::dict_of(entry).is_some(),
                        };
                        if !right_kind {
                            run.sink.error(
                                "resources-malformed",
                                loc,
                                format!(
                                    "/{category} /{shown} is {}, not a dictionary or stream",
                                    entry.enum_variant()
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Decodes and walks one content stream.
pub(crate) fn check_content_data<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    data: &[u8],
    resources: Option<&'a Dictionary>,
    depth: usize,
    holder: Option<ObjectId>,
) {
    let loc = Loc::at(page, holder);
    if data.len() > MAX_PARSE_BYTES {
        run.sink.info(
            "content-not-checked",
            loc,
            format!(
                "a content stream of {} bytes is larger than the 16 MiB that is parsed; its operators were not checked",
                data.len()
            ),
        );
        return;
    }
    let content = match Content::decode(data) {
        Ok(c) => c,
        Err(e) => {
            run.sink.error(
                "content-stream-unreadable",
                loc,
                format!("the content stream cannot be parsed: {e}"),
            );
            return;
        }
    };

    let doc = run.doc;
    let mut graphics_depth = 0usize;
    let mut underflow = false;
    let mut in_text = false;
    let mut text_nesting = false;
    let mut stray_et = false;
    let mut outside_text = 0usize;
    let mut font: Option<(Option<ObjectId>, &'a Object)> = None;
    let mut font_stack: Vec<Option<(Option<ObjectId>, &'a Object)>> = Vec::new();
    let mut unsaved = 0usize;
    let mut done: HashSet<(&'static str, Vec<u8>)> = HashSet::new();

    for op in &content.operations {
        if run.operators_left == 0 {
            run.sink
                .limit("the limit of 20,000,000 content stream operators was reached");
            break;
        }
        run.operators_left -= 1;
        let first_name = match op.operands.first() {
            Some(Object::Name(n)) => Some(n.as_slice()),
            _ => None,
        };
        match op.operator.as_str() {
            "q" => {
                graphics_depth = graphics_depth.saturating_add(1);
                // The font is part of the graphics state. The saved fonts are kept for a
                // nesting of 4096 levels, which is more than any real file uses.
                if font_stack.len() < 4096 {
                    font_stack.push(font);
                } else {
                    unsaved = unsaved.saturating_add(1);
                }
            }
            "Q" => {
                if graphics_depth == 0 {
                    underflow = true;
                } else {
                    graphics_depth -= 1;
                    if unsaved > 0 {
                        unsaved -= 1;
                    } else {
                        font = font_stack.pop().flatten();
                    }
                }
            }
            "BT" => {
                if in_text {
                    text_nesting = true;
                }
                in_text = true;
            }
            "ET" => {
                if !in_text {
                    stray_et = true;
                }
                in_text = false;
            }
            "Tf" => {
                font = None;
                if let Some(name) = first_name {
                    match resource(doc, resources, "Font", name) {
                        Res::Found(id, o) => font = Some((id, o)),
                        Res::NotFound => missing(run, page, "Font", name),
                        Res::Null | Res::Dangling => {}
                    }
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if !in_text {
                    outside_text = outside_text.saturating_add(1);
                }
                if let Some((id, o)) = font {
                    check_font(run, page, id, o);
                }
            }
            "gs" => {
                if let Some(name) = first_name {
                    if done.insert(("gs", name.to_vec())) {
                        match resource(doc, resources, "ExtGState", name) {
                            Res::Found(_, o) => {
                                if let Some(d) = super::dict_of(o) {
                                    check_ext_gstate(run, page, d);
                                }
                            }
                            Res::NotFound => missing(run, page, "ExtGState", name),
                            Res::Null | Res::Dangling => {}
                        }
                    }
                }
            }
            "Do" => {
                if let Some(name) = first_name {
                    if done.insert(("Do", name.to_vec())) {
                        match resource(doc, resources, "XObject", name) {
                            Res::Found(id, Object::Stream(s)) => {
                                check_xobject(run, page, id, s, resources, depth);
                            }
                            Res::NotFound => missing(run, page, "XObject", name),
                            _ => {}
                        }
                    }
                }
            }
            "cs" | "CS" => {
                if let Some(name) = first_name {
                    colour_by_name(run, page, name, resources, &mut done);
                }
            }
            "scn" | "SCN" | "sc" | "SC" => {
                if let Some(Object::Name(name)) = op.operands.last() {
                    if done.insert(("Pattern", name.clone())) {
                        pattern(run, page, name, resources, depth);
                    }
                }
            }
            "sh" => {
                if let Some(name) = first_name {
                    if done.insert(("sh", name.to_vec())) {
                        match resource(doc, resources, "Shading", name) {
                            Res::Found(_, o) => {
                                if let Some(d) = super::dict_of(o) {
                                    shading(run, page, d);
                                }
                            }
                            Res::NotFound => missing(run, page, "Shading", name),
                            _ => {}
                        }
                    }
                }
            }
            "g" | "G" => colour::note_use(run, page, &Space::Gray),
            "rg" | "RG" => colour::note_use(run, page, &Space::Rgb),
            "k" | "K" => colour::note_use(run, page, &Space::Cmyk),
            "BDC" | "DP" => {
                if let Some(Object::Name(name)) = op.operands.get(1) {
                    if done.insert(("Properties", name.clone())) {
                        if let Res::NotFound = resource(doc, resources, "Properties", name) {
                            missing(run, page, "Properties", name);
                        }
                    }
                }
            }
            "BI" => {
                if let Some(Object::Stream(s)) = op.operands.first() {
                    inline_image(run, page, s, resources, &mut done);
                }
            }
            _ => {}
        }
    }

    if underflow {
        run.sink.warn(
            "graphics-state-unbalanced",
            loc,
            "a Q operator has no matching q".to_string(),
        );
    }
    if graphics_depth > 0 {
        run.sink.warn(
            "graphics-state-unbalanced",
            loc,
            format!("{graphics_depth} q operators have no matching Q at the end of the stream"),
        );
    }
    if text_nesting || stray_et || in_text {
        let what = if text_nesting {
            "a BT inside a text object"
        } else if stray_et {
            "an ET without BT"
        } else {
            "a BT without ET at the end of the stream"
        };
        run.sink.warn(
            "text-object-unbalanced",
            loc,
            format!("BT and ET do not match: {what}"),
        );
    }
    if outside_text > 0 {
        run.sink.warn(
            "text-outside-text-object",
            loc,
            format!("{outside_text} text-showing operators are outside BT ... ET"),
        );
    }
}

fn missing(run: &mut Run<'_>, page: Option<usize>, category: &str, name: &[u8]) {
    let shown = name_text(name);
    run.sink.once(
        Severity::Error,
        "resource-not-found",
        format!("{category}/{shown}"),
        Loc::at(page, None),
        format!("the content uses /{category} /{shown}, which the resources do not have"),
    );
}

fn colour_by_name<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    name: &[u8],
    resources: Option<&'a Dictionary>,
    done: &mut HashSet<(&'static str, Vec<u8>)>,
) {
    let space = match name {
        b"DeviceGray" => Some(Space::Gray),
        b"DeviceRGB" => Some(Space::Rgb),
        b"DeviceCMYK" => Some(Space::Cmyk),
        b"Pattern" => None,
        _ => {
            if !done.insert(("cs", name.to_vec())) {
                return;
            }
            match resource(run.doc, resources, "ColorSpace", name) {
                Res::Found(_, o) => {
                    let loc = Loc::at(page, None);
                    // Referenced through the resources, so it is read as the entry is.
                    Some(colour::analyse(run, o, loc, false, 0))
                }
                Res::NotFound => {
                    missing(run, page, "ColorSpace", name);
                    None
                }
                _ => None,
            }
        }
    };
    if let Some(sp) = space {
        colour::note_use(run, page, &sp);
    }
}

fn inline_image<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    s: &Stream,
    resources: Option<&'a Dictionary>,
    done: &mut HashSet<(&'static str, Vec<u8>)>,
) {
    let cs = s
        .dict
        .get(b"CS")
        .or_else(|_| s.dict.get(b"ColorSpace"))
        .ok();
    let Some(cs) = cs else { return };
    match cs {
        Object::Name(n)
            if !matches!(
                n.as_slice(),
                b"G" | b"RGB" | b"CMYK" | b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK"
            ) =>
        {
            colour_by_name(run, page, n, resources, done);
        }
        other => {
            let sp = colour::analyse(run, other, Loc::at(page, None), true, 0);
            colour::note_use(run, page, &sp);
        }
    }
}

fn pattern<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    name: &[u8],
    resources: Option<&'a Dictionary>,
    depth: usize,
) {
    let doc = run.doc;
    match resource(doc, resources, "Pattern", name) {
        Res::Found(id, o) => {
            let Some(d) = super::dict_of(o) else { return };
            match look(doc, d, "PatternType").integer() {
                Some(1) => {
                    if let Object::Stream(s) = o {
                        // A tiling pattern is drawn by a content stream, like a form.
                        check_form_like(run, page, id, s, resources, depth, false);
                    }
                }
                Some(2) => {
                    if let Some(sd) = look(doc, d, "Shading").dict() {
                        shading(run, page, sd);
                    }
                }
                _ => {}
            }
        }
        Res::NotFound => missing(run, page, "Pattern", name),
        _ => {}
    }
}

fn shading(run: &mut Run<'_>, page: Option<usize>, d: &Dictionary) {
    if let Ok(cs) = d.get(b"ColorSpace") {
        let sp = colour::analyse(run, cs, Loc::at(page, None), false, 0);
        colour::note_use(run, page, &sp);
    } else {
        run.sink.error(
            "colour-space-malformed",
            Loc::at(page, None),
            "a shading dictionary has no /ColorSpace".to_string(),
        );
    }
}

/// Whether the filter list of an image ends in JPEG 2000, which carries its own colour space.
fn is_jpx(doc: &Document, s: &Stream) -> bool {
    filter_names(doc, s).is_some_and(|f| f.iter().any(|n| n == b"JPXDecode"))
}

fn check_xobject<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    id: Option<ObjectId>,
    s: &'a Stream,
    resources: Option<&'a Dictionary>,
    depth: usize,
) {
    let doc = run.doc;
    let loc = Loc::at(page, id);
    match look(doc, &s.dict, "Subtype").name() {
        Some(b"Image") => check_image(run, page, id, s),
        Some(b"Form") => check_form_like(run, page, id, s, resources, depth, true),
        Some(b"PS") => {
            let sev = run.sink.forbidden(None);
            if let Some(sev) = sev {
                run.sink.push(
                    sev,
                    "postscript-xobject",
                    loc,
                    "a PostScript XObject is used".to_string(),
                );
            }
        }
        _ => run.sink.error(
            "xobject-subtype-invalid",
            loc,
            "an XObject has no /Subtype of Image, Form or PS".to_string(),
        ),
    }
}

fn check_image(run: &mut Run<'_>, page: Option<usize>, id: Option<ObjectId>, s: &Stream) {
    if let Some(id) = id {
        if !run.first_visit(Seen::Image, id) {
            return;
        }
    }
    let doc = run.doc;
    let loc = Loc::at(page, id);
    let d = &s.dict;
    let mask = look(doc, d, "ImageMask").boolean() == Some(true);
    let jpx = is_jpx(doc, s);

    let mut missing_keys = Vec::new();
    for key in ["Width", "Height"] {
        if look(doc, d, key).integer().is_none_or(|v| v <= 0) {
            missing_keys.push(key);
        }
    }
    if !mask && !jpx && look(doc, d, "BitsPerComponent").integer().is_none() {
        missing_keys.push("BitsPerComponent");
    }
    if !missing_keys.is_empty() {
        run.sink.error(
            "image-dict-incomplete",
            loc,
            format!("the image has no valid {}", missing_keys.join(", ")),
        );
    }

    match d.get(b"ColorSpace") {
        Ok(cs) if !mask => {
            let sp = colour::analyse(run, cs, loc, false, 0);
            colour::note_use(run, page, &sp);
        }
        Ok(_) => {}
        Err(_) => {
            if !mask && !jpx {
                run.sink.error(
                    "image-no-colour-space",
                    loc,
                    "the image is not a mask and has no /ColorSpace".to_string(),
                );
            }
        }
    }

    if run.profile().forbids_transparency() && !look(doc, d, "SMask").is_absent() {
        transparency(
            run,
            "transparency-smask",
            "image",
            loc,
            "an image has a soft mask (/SMask)",
        );
    }
}

/// A form XObject or a tiling pattern: a content stream with a resource dictionary.
fn check_form_like<'a>(
    run: &mut Run<'a>,
    page: Option<usize>,
    id: Option<ObjectId>,
    s: &'a Stream,
    parent: Option<&'a Dictionary>,
    depth: usize,
    is_form: bool,
) {
    if depth >= super::MAX_NESTING {
        run.sink
            .limit("forms and patterns nested more than 12 levels deep were not followed");
        return;
    }
    if let Some(id) = id {
        if !run.first_visit(Seen::Form, id) {
            return;
        }
    }
    if !run.visit() {
        return;
    }
    let doc = run.doc;
    let loc = Loc::at(page, id);
    let d = &s.dict;

    if is_form && d.get(b"BBox").ok().and_then(|b| run.rect(b)).is_none() {
        run.sink.error(
            "form-bbox-missing",
            loc,
            "a form XObject has no valid /BBox".to_string(),
        );
    }
    if is_form && run.profile().forbids_transparency() {
        if let Some(g) = look(doc, d, "Group").dict() {
            if look(doc, g, "S").name() == Some(b"Transparency") {
                transparency(
                    run,
                    "transparency-group",
                    "form",
                    loc,
                    "a form XObject is a transparency group",
                );
            }
        }
    }

    let own = look(doc, d, "Resources");
    let resources = match own {
        Look::Found(rid, Object::Dictionary(r)) => {
            check_resources(run, page, r, rid);
            Some(r)
        }
        _ => parent,
    };

    match decode(run, s, Budget::Content) {
        Decoded::Data(bytes) => {
            check_content_data(run, page, &bytes, resources, depth.saturating_add(1), id);
        }
        Decoded::Failed(e) => run.sink.error(
            "content-stream-unreadable",
            loc,
            format!("the content stream of a form cannot be decoded: {e}"),
        ),
        Decoded::Skipped => {}
    }
}

fn transparency(run: &mut Run<'_>, rule: &'static str, key: &str, loc: Loc, message: &str) {
    run.sink.once(
        Severity::Error,
        rule,
        key.to_string(),
        loc,
        format!("{message}, which {} does not allow", run.profile().label()),
    );
}

/// An appearance stream of an annotation, which is a form XObject.
pub(crate) fn check_appearance<'a>(
    run: &mut Run<'a>,
    page: usize,
    id: Option<ObjectId>,
    s: &'a Stream,
    page_resources: Option<&'a Dictionary>,
) {
    check_form_like(run, Some(page), id, s, page_resources, 0, true);
}

fn check_ext_gstate(run: &mut Run<'_>, page: Option<usize>, d: &Dictionary) {
    if !run.profile().forbids_transparency() {
        return;
    }
    let doc = run.doc;
    let loc = Loc::at(page, None);
    match look(doc, d, "SMask") {
        Look::Absent => {}
        Look::Found(_, Object::Name(n)) if n == b"None" => {}
        _ => transparency(
            run,
            "transparency-smask",
            "gs",
            loc,
            "a graphics state sets a soft mask (/SMask)",
        ),
    }
    for key in ["CA", "ca"] {
        if let Some(v) = look(doc, d, key).number() {
            if v < 1.0 {
                transparency(
                    run,
                    "transparency-alpha",
                    "gs",
                    loc,
                    &format!("a graphics state sets /{key} to {v}"),
                );
            }
        }
    }
    let blend = look(doc, d, "BM");
    let name = match blend {
        Look::Found(_, Object::Name(n)) => Some(n.clone()),
        Look::Found(_, Object::Array(a)) => match a.first() {
            Some(Object::Name(n)) => Some(n.clone()),
            _ => None,
        },
        _ => None,
    };
    if let Some(n) = name {
        if n != b"Normal" && n != b"Compatible" {
            transparency(
                run,
                "transparency-blend-mode",
                "gs",
                loc,
                &format!("a graphics state sets the blend mode /{}", name_text(&n)),
            );
        }
    }
}

/// The name of a font without the `ABCDEF+` prefix of a subset.
fn plain_font_name(name: &str) -> &str {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|b| b.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

/// The signature check of a font file. `None` is fine.
fn font_file_problem(run: &mut Run<'_>, key: &str, s: &Stream) -> Option<(Severity, String)> {
    let data = match decode(run, s, Budget::Validation) {
        Decoded::Data(d) => d,
        Decoded::Failed(e) => {
            return Some((Severity::Error, format!("/{key} cannot be decoded: {e}")));
        }
        Decoded::Skipped => return None,
    };
    if data.is_empty() {
        return Some((Severity::Error, format!("/{key} is empty")));
    }
    let head = data.get(..4).unwrap_or(&[]);
    let sfnt = head == [0, 1, 0, 0] || head == b"true" || head == b"OTTO" || head == b"ttcf";
    let ok = match key {
        "FontFile" => matches!(data.first(), Some(b'%' | 0x80)),
        "FontFile2" => sfnt,
        _ => {
            let subtype = look(run.doc, &s.dict, "Subtype").name().map(<[u8]>::to_vec);
            match subtype.as_deref() {
                Some(b"OpenType") => sfnt,
                Some(b"Type1C" | b"CIDFontType0C") => data.first() == Some(&1),
                _ => true,
            }
        }
    };
    (!ok).then(|| {
        (
            Severity::Warning,
            format!("/{key} does not start like a font file of that kind"),
        )
    })
}

/// Checks a font that a content stream uses to show text.
fn check_font(run: &mut Run<'_>, page: Option<usize>, id: Option<ObjectId>, o: &Object) {
    if let Some(id) = id {
        if !run.first_visit(Seen::Font, id) {
            return;
        }
    }
    if !run.visit() {
        return;
    }
    let doc = run.doc;
    let loc = Loc::at(page, id);
    let Some(font) = super::dict_of(o) else {
        run.sink.error(
            "font-subtype-invalid",
            loc,
            "a font is not a dictionary".to_string(),
        );
        return;
    };
    let subtype = look(doc, font, "Subtype").name().map(<[u8]>::to_vec);
    let base_font = look(doc, font, "BaseFont")
        .name()
        .map(name_text)
        .unwrap_or_else(|| "(no name)".to_string());
    let general = !run.profile().is_standard();

    // The dictionary that has the font descriptor: the descendant for a Type0 font.
    let (holder, simple): (&Dictionary, bool) = match subtype.as_deref() {
        Some(b"Type3") => {
            if general && look(doc, font, "ToUnicode").is_absent() {
                run.sink.warn(
                    "font-no-tounicode",
                    loc,
                    format!("the Type 3 font {base_font} has no /ToUnicode, so its text cannot be extracted"),
                );
            }
            return;
        }
        Some(b"Type0") => {
            let descendant = look(doc, font, "DescendantFonts")
                .array()
                .and_then(|a| a.first())
                .and_then(|o| resolve(doc, o).dict());
            let Some(desc) = descendant else {
                run.sink.error(
                    "font-subtype-invalid",
                    loc,
                    format!("the Type0 font {base_font} has no descendant font"),
                );
                return;
            };
            if general && look(doc, font, "ToUnicode").is_absent() {
                let identity = match look(doc, font, "Encoding") {
                    Look::Found(_, Object::Name(n)) => n == b"Identity-H" || n == b"Identity-V",
                    Look::Found(_, Object::Stream(_)) => true,
                    _ => false,
                };
                if identity {
                    run.sink.warn(
                        "font-no-tounicode",
                        loc,
                        format!("the font {base_font} uses glyph ids as character codes and has no /ToUnicode, so its text cannot be extracted"),
                    );
                }
            }
            (desc, false)
        }
        Some(b"Type1" | b"MMType1" | b"TrueType") => {
            if general && look(doc, font, "ToUnicode").is_absent() {
                let differences = look(doc, font, "Encoding")
                    .dict()
                    .is_some_and(|e| e.has(b"Differences"));
                if differences {
                    run.sink.warn(
                        "font-no-tounicode",
                        loc,
                        format!("the font {base_font} has a /Differences encoding and no /ToUnicode, so its text may not be extractable"),
                    );
                }
            }
            (font, true)
        }
        _ => {
            run.sink.error(
                "font-subtype-invalid",
                loc,
                format!("the font {base_font} has no /Subtype of Type1, TrueType, Type0 or Type3"),
            );
            return;
        }
    };

    let is_std = simple && STANDARD_14.contains(&plain_font_name(&base_font));
    let descriptor = look(doc, holder, "FontDescriptor");
    let desc = descriptor.dict();

    if desc.is_none() {
        if !is_std {
            run.sink.error(
                "font-descriptor-missing",
                loc,
                format!("the font {base_font} has no /FontDescriptor and is not one of the 14 standard fonts"),
            );
        }
        report_not_embedded(run, loc, &base_font, is_std);
        return;
    }
    let Some(desc) = desc else { return };

    let mut embedded = false;
    for key in ["FontFile", "FontFile2", "FontFile3"] {
        match look(doc, desc, key) {
            Look::Absent => {}
            Look::Found(_, Object::Stream(s)) => {
                embedded = true;
                if let Some((sev, msg)) = font_file_problem(run, key, s) {
                    run.sink.push(
                        sev,
                        "font-program-invalid",
                        loc,
                        format!("the font {base_font}: {msg}"),
                    );
                }
            }
            _ => {
                run.sink.error(
                    "font-program-invalid",
                    loc,
                    format!("the font {base_font}: /{key} is not a stream or refers to a missing object"),
                );
            }
        }
    }
    if !embedded {
        report_not_embedded(run, loc, &base_font, is_std);
    }
}

fn report_not_embedded(run: &mut Run<'_>, loc: Loc, name: &str, standard_14: bool) {
    let general = !run.profile().is_standard();
    let sev = if general {
        if standard_14 {
            Severity::Info
        } else {
            Severity::Warning
        }
    } else {
        Severity::Error
    };
    let note = if standard_14 {
        " (one of the 14 standard fonts, which readers substitute)"
    } else {
        ""
    };
    run.sink.push(
        sev,
        "font-not-embedded",
        loc,
        format!("the font {name} is used but its font program is not embedded{note}"),
    );
}
