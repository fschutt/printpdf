//! Colour spaces: the shape of the arrays and dictionaries that name them, and which families
//! a document uses.

use lopdf::{Object, ObjectId};

use super::util::{decode, look, name_text, resolve, Budget, Decoded, Look};
use super::{Loc, Run, Seen, Severity};

/// A colour space, as far as the preflight looks at it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Space {
    Gray,
    Rgb,
    Cmyk,
    CalGray,
    CalRgb,
    Lab,
    /// ICC-based, with the number of components.
    Icc(i64),
    Indexed(Box<Space>),
    Separation,
    DeviceN,
    Pattern,
    /// Reported as a finding already.
    Invalid,
}

impl Space {
    /// The number of colour components, when it is known.
    fn components(&self) -> Option<usize> {
        match self {
            Space::Gray | Space::CalGray | Space::Indexed(_) | Space::Separation => Some(1),
            Space::Rgb | Space::CalRgb | Space::Lab => Some(3),
            Space::Cmyk => Some(4),
            Space::Icc(n) => usize::try_from(*n).ok(),
            _ => None,
        }
    }
}

/// The three device colour spaces, as an index into `Run::device_use`.
fn device_index(sp: &Space) -> Option<usize> {
    match sp {
        Space::Gray => Some(0),
        Space::Rgb => Some(1),
        Space::Cmyk => Some(2),
        _ => None,
    }
}

const DEVICE_NAMES: [&str; 3] = ["DeviceGray", "DeviceRGB", "DeviceCMYK"];

fn malformed(run: &mut Run<'_>, loc: Loc, what: String) -> Space {
    run.sink.error("colour-space-malformed", loc, what);
    Space::Invalid
}

/// Reads a colour space and reports it when it is malformed.
///
/// `inline` allows the abbreviated names of inline images. A colour space that is an indirect
/// object is read once.
pub(crate) fn analyse(
    run: &mut Run<'_>,
    cs: &Object,
    loc: Loc,
    inline: bool,
    depth: usize,
) -> Space {
    if depth > super::MAX_NESTING {
        run.sink
            .limit("colour spaces nested more than 12 levels deep were not followed");
        return Space::Invalid;
    }
    if let Object::Reference(id) = cs {
        if let Some(cached) = run.cs_cache.get(id) {
            return cached.clone();
        }
        let loc = Loc::at(loc.page, Some(*id));
        let space = analyse_resolved(run, cs, loc, inline, depth);
        run.cs_cache.insert(*id, space.clone());
        return space;
    }
    analyse_resolved(run, cs, loc, inline, depth)
}

fn analyse_resolved(run: &mut Run<'_>, cs: &Object, loc: Loc, inline: bool, depth: usize) -> Space {
    match run.resolve(cs) {
        Look::Absent => malformed(run, loc, "the colour space is null".to_string()),
        Look::Dangling(id) => malformed(
            run,
            loc,
            format!(
                "the colour space refers to object {} {}, which does not exist",
                id.0, id.1
            ),
        ),
        Look::Found(_, Object::Name(n)) => named(run, n, loc, inline),
        Look::Found(id, Object::Array(items)) => {
            let loc = Loc::at(loc.page, id.or(loc.obj));
            array(run, items, loc, inline, depth)
        }
        Look::Found(_, other) => malformed(
            run,
            loc,
            format!(
                "a colour space must be a name or an array, found {}",
                other.enum_variant()
            ),
        ),
    }
}

fn named(run: &mut Run<'_>, n: &[u8], loc: Loc, inline: bool) -> Space {
    match n {
        b"DeviceGray" => Space::Gray,
        b"DeviceRGB" => Space::Rgb,
        b"DeviceCMYK" => Space::Cmyk,
        b"Pattern" => Space::Pattern,
        b"G" if inline => Space::Gray,
        b"RGB" if inline => Space::Rgb,
        b"CMYK" if inline => Space::Cmyk,
        b"CalGray" | b"CalRGB" | b"Lab" | b"ICCBased" | b"Indexed" | b"Separation" | b"DeviceN" => {
            malformed(
                run,
                loc,
                format!(
                    "the colour space /{} must be an array, not a bare name",
                    name_text(n)
                ),
            )
        }
        _ => {
            run.sink.error(
                "colour-space-unknown",
                loc,
                format!("/{} is not a colour space that PDF defines", name_text(n)),
            );
            Space::Invalid
        }
    }
}

fn array(run: &mut Run<'_>, items: &[Object], loc: Loc, inline: bool, depth: usize) -> Space {
    let Some(first) = items.first() else {
        return malformed(run, loc, "the colour space array is empty".to_string());
    };
    let Look::Found(_, Object::Name(family)) = run.resolve(first) else {
        return malformed(
            run,
            loc,
            "the first element of a colour space array is not a name".to_string(),
        );
    };
    let rest = items.get(1..).unwrap_or(&[]);
    let fam = name_text(family);
    let want = |run: &mut Run<'_>, ok: bool, what: &str| -> bool {
        if !ok {
            run.sink.error(
                "colour-space-malformed",
                loc,
                format!("the /{fam} colour space {what}"),
            );
        }
        ok
    };

    match family.as_slice() {
        b"DeviceGray" | b"G" => Space::Gray,
        b"DeviceRGB" | b"RGB" => Space::Rgb,
        b"DeviceCMYK" | b"CMYK" => Space::Cmyk,
        b"ICCBased" => {
            if !want(
                run,
                items.len() == 2,
                "needs exactly two elements: the name and the profile stream",
            ) {
                return Space::Invalid;
            }
            match rest.first().map(|o| run.resolve(o)) {
                Some(Look::Found(id, Object::Stream(s))) => icc(run, s, id, loc),
                Some(Look::Dangling(id)) => malformed(
                    run,
                    loc,
                    format!(
                        "the /ICCBased profile is object {} {}, which does not exist",
                        id.0, id.1
                    ),
                ),
                _ => malformed(
                    run,
                    loc,
                    "the second element of /ICCBased is not a profile stream".to_string(),
                ),
            }
        }
        b"CalGray" | b"CalRGB" | b"Lab" => {
            if !want(
                run,
                items.len() == 2,
                "needs exactly two elements: the name and a dictionary",
            ) {
                return Space::Invalid;
            }
            let Some(d) = rest.first().and_then(|o| run.resolve(o).dict()) else {
                return malformed(
                    run,
                    loc,
                    format!("the second element of /{fam} is not a dictionary"),
                );
            };
            let triple = |run: &mut Run<'_>, key: &str, n: usize, required: bool| -> bool {
                match look(run.doc, d, key) {
                    Look::Absent => {
                        if required {
                            run.sink.error(
                                "colour-space-malformed",
                                loc,
                                format!("the /{fam} dictionary has no /{key}"),
                            );
                        }
                        !required
                    }
                    Look::Found(_, Object::Array(a))
                        if a.len() == n
                            && a.iter().all(|x| resolve(run.doc, x).number().is_some()) =>
                    {
                        true
                    }
                    _ => {
                        run.sink.error(
                            "colour-space-malformed",
                            loc,
                            format!("/{key} of /{fam} is not an array of {n} numbers"),
                        );
                        false
                    }
                }
            };
            let mut ok = triple(run, "WhitePoint", 3, true);
            match family.as_slice() {
                b"CalGray" => {}
                b"CalRGB" => {
                    ok &= triple(run, "Gamma", 3, false);
                    ok &= triple(run, "Matrix", 9, false);
                }
                _ => ok &= triple(run, "Range", 4, false),
            }
            if !ok {
                return Space::Invalid;
            }
            match family.as_slice() {
                b"CalGray" => Space::CalGray,
                b"CalRGB" => Space::CalRgb,
                _ => Space::Lab,
            }
        }
        b"Indexed" | b"I" => indexed(run, rest, loc, depth, items.len()),
        b"Separation" => {
            if !want(run, items.len() == 4, "needs four elements: the name, a colorant name, an alternate space and a tint function") {
                return Space::Invalid;
            }
            let mut ok = matches!(
                rest.first().map(|o| run.resolve(o)),
                Some(Look::Found(_, Object::Name(_)))
            );
            if !ok {
                malformed(
                    run,
                    loc,
                    "the colorant of /Separation is not a name".to_string(),
                );
            }
            ok &= alternate(run, rest.get(1), loc, depth, "Separation");
            ok &= function(run, rest.get(2), loc, "Separation");
            if ok {
                Space::Separation
            } else {
                Space::Invalid
            }
        }
        b"DeviceN" => {
            if !want(run, items.len() == 4 || items.len() == 5, "needs four or five elements: the name, the colorant names, an alternate space, a tint function and an optional dictionary") {
                return Space::Invalid;
            }
            let names_ok = match rest.first().map(|o| run.resolve(o)) {
                Some(Look::Found(_, Object::Array(a))) => {
                    !a.is_empty()
                        && a.iter()
                            .all(|x| matches!(run.resolve(x), Look::Found(_, Object::Name(_))))
                }
                _ => false,
            };
            if !names_ok {
                malformed(
                    run,
                    loc,
                    "the colorant names of /DeviceN are not a non-empty array of names".to_string(),
                );
            }
            let mut ok = names_ok;
            ok &= alternate(run, rest.get(1), loc, depth, "DeviceN");
            ok &= function(run, rest.get(2), loc, "DeviceN");
            if ok {
                Space::DeviceN
            } else {
                Space::Invalid
            }
        }
        b"Pattern" => {
            if !want(run, items.len() <= 2, "has more than two elements") {
                return Space::Invalid;
            }
            if let Some(base) = rest.first() {
                let b = analyse(run, base, loc, inline, depth.saturating_add(1));
                if b == Space::Pattern {
                    return malformed(
                        run,
                        loc,
                        "the base of a /Pattern space is a /Pattern space".to_string(),
                    );
                }
            }
            Space::Pattern
        }
        _ => {
            run.sink.error(
                "colour-space-unknown",
                loc,
                format!("/{fam} is not a colour space family that PDF defines"),
            );
            Space::Invalid
        }
    }
}

/// The alternate space of a Separation or DeviceN space.
fn alternate(run: &mut Run<'_>, alt: Option<&Object>, loc: Loc, depth: usize, fam: &str) -> bool {
    let Some(alt) = alt else {
        return false;
    };
    match analyse(run, alt, loc, false, depth.saturating_add(1)) {
        Space::Invalid => false,
        Space::Pattern => {
            run.sink.error(
                "colour-space-malformed",
                loc,
                format!("the alternate space of /{fam} is /Pattern"),
            );
            false
        }
        _ => true,
    }
}

/// The tint transform: a function dictionary or a function stream.
fn function(run: &mut Run<'_>, f: Option<&Object>, loc: Loc, fam: &str) -> bool {
    let ok = f.is_some_and(|o| {
        run.resolve(o)
            .dict()
            .is_some_and(|d| d.has(b"FunctionType"))
    });
    if !ok {
        run.sink.error(
            "colour-space-malformed",
            loc,
            format!("the tint transform of /{fam} is not a function (a dictionary or stream with /FunctionType)"),
        );
    }
    ok
}

fn indexed(run: &mut Run<'_>, rest: &[Object], loc: Loc, depth: usize, len: usize) -> Space {
    if len != 4 {
        return malformed(
            run,
            loc,
            "the /Indexed colour space needs four elements: the name, a base space, the highest index and a lookup table".to_string(),
        );
    }
    let base = match rest.first() {
        Some(b) => analyse(run, b, loc, false, depth.saturating_add(1)),
        None => Space::Invalid,
    };
    match base {
        Space::Invalid => return Space::Invalid,
        Space::Pattern | Space::Indexed(_) => {
            return malformed(
                run,
                loc,
                "the base of an /Indexed space is /Pattern or /Indexed".to_string(),
            )
        }
        _ => {}
    }
    let hival = rest.get(1).and_then(|o| run.resolve(o).integer());
    let Some(hival) = hival.filter(|h| (0..=255).contains(h)) else {
        return malformed(
            run,
            loc,
            "the highest index of an /Indexed space is not an integer from 0 to 255".to_string(),
        );
    };
    let lookup_len = match rest.get(2).map(|o| run.resolve(o)) {
        Some(Look::Found(_, Object::String(s, _))) => Some(s.len()),
        Some(Look::Found(_, Object::Stream(s))) => {
            // Only a lookup table without a filter has a length that can be compared.
            if s.dict.has(b"Filter") {
                None
            } else {
                Some(s.content.len())
            }
        }
        _ => {
            return malformed(
                run,
                loc,
                "the lookup table of an /Indexed space is not a string or a stream".to_string(),
            )
        }
    };
    if let (Some(have), Some(comps)) = (lookup_len, base.components()) {
        let need = usize::try_from(hival)
            .unwrap_or(0)
            .saturating_add(1)
            .saturating_mul(comps);
        if have < need {
            return malformed(
                run,
                loc,
                format!(
                    "the lookup table of an /Indexed space has {have} bytes, but {need} are needed"
                ),
            );
        }
    }
    Space::Indexed(Box::new(base))
}

/// The colour space signature at byte 16 of an ICC profile, as the component count it implies.
fn header_components(sig: &[u8]) -> Option<i64> {
    match sig {
        b"GRAY" => Some(1),
        b"RGB " => Some(3),
        b"CMYK" => Some(4),
        _ => None,
    }
}

fn icc(run: &mut Run<'_>, s: &lopdf::Stream, id: Option<ObjectId>, loc: Loc) -> Space {
    let n = look(run.doc, &s.dict, "N").integer();
    let n = match n {
        Some(n @ (1 | 3 | 4)) => n,
        _ => {
            run.sink.error(
                "icc-profile-invalid",
                loc,
                "the /N of an ICC profile stream is missing or not 1, 3 or 4".to_string(),
            );
            return Space::Invalid;
        }
    };
    // Each profile object is read once.
    let first = id.is_none_or(|i| run.first_visit(Seen::Profile, i));
    if first {
        if let Some(problem) = profile_problem(run, s, n) {
            run.sink.error("icc-profile-invalid", loc, problem);
            return Space::Invalid;
        }
    }
    Space::Icc(n)
}

/// What is wrong with the header of an ICC profile, if anything.
pub(super) fn profile_problem(run: &mut Run<'_>, s: &lopdf::Stream, n: i64) -> Option<String> {
    let data = match decode(run, s, Budget::Validation) {
        Decoded::Data(d) => d,
        Decoded::Failed(e) => {
            return Some(format!("the ICC profile stream cannot be decoded: {e}"))
        }
        Decoded::Skipped => return None,
    };
    if data.len() < 128 {
        return Some(format!(
            "the ICC profile has {} bytes, fewer than its 128 byte header",
            data.len()
        ));
    }
    if data.get(36..40) != Some(b"acsp") {
        return Some("the ICC profile header does not carry the signature \"acsp\"".to_string());
    }
    let implied = data.get(16..20).and_then(header_components);
    match implied {
        Some(m) if m != n => Some(format!(
            "/N is {n}, but the header of the ICC profile describes a space with {m} components"
        )),
        _ => None,
    }
}

/// Records the use of a colour space for the rules about device colour and PDF/X-1a. `page` is
/// the page that uses it.
pub(crate) fn note_use(run: &mut Run<'_>, page: Option<usize>, sp: &Space) {
    let profile = run.profile();
    let loc = Loc::at(page, None);
    match sp {
        Space::Indexed(base) => note_use(run, page, base),
        Space::Gray | Space::Rgb | Space::Cmyk => {
            if let Some(i) = device_index(sp) {
                if let Some(slot) = run.device_use.get_mut(i) {
                    if slot.is_none() {
                        *slot = Some(page);
                    }
                }
            }
            if profile == super::PreflightProfile::PdfX1a_2001 && *sp == Space::Rgb {
                run.sink.once(
                    Severity::Error,
                    "rgb-colour-in-pdfx1a",
                    String::new(),
                    loc,
                    "DeviceRGB is used; PDF/X-1a allows only gray, CMYK and spot colours"
                        .to_string(),
                );
            }
        }
        Space::CalGray | Space::CalRgb | Space::Lab | Space::Icc(_) => {
            if profile == super::PreflightProfile::PdfX1a_2001 {
                let what = match sp {
                    Space::CalGray => "CalGray",
                    Space::CalRgb => "CalRGB",
                    Space::Lab => "Lab",
                    _ => "ICCBased",
                };
                run.sink.once(
                    Severity::Error,
                    "colour-space-not-allowed",
                    what.to_string(),
                    loc,
                    format!("a {what} colour space is used; PDF/X-1a allows only device colour and spot colours"),
                );
            }
        }
        _ => {}
    }
}

/// After the walk: the device colour spaces that PDF/A needs an output intent or a default
/// colour space for.
pub(crate) fn finish_device_colour(run: &mut Run<'_>) {
    if !run.profile().is_pdfa() {
        return;
    }
    for (i, first_page) in run.device_use.into_iter().enumerate() {
        let Some(first_page) = first_page else {
            continue;
        };
        let name = DEVICE_NAMES.get(i).copied().unwrap_or("Device");
        if run.default_spaces.get(i).copied().unwrap_or(false) {
            continue;
        }
        let wanted: Option<i64> = match i {
            1 => Some(3),
            2 => Some(4),
            _ => None,
        };
        let covered = match wanted {
            Some(n) => run.intent_components.contains(&n),
            None => run.has_output_intent,
        };
        if covered {
            continue;
        }
        let need = match wanted {
            Some(n) => format!("an output intent with a {n}-component profile"),
            None => "an output intent".to_string(),
        };
        run.sink.once(
            Severity::Error,
            "device-colour-without-output-intent",
            name.to_string(),
            Loc::at(first_page, None),
            format!(
                "{name} is used, but the file has neither {need} nor a Default{} colour space",
                &name.get(6..).unwrap_or("")
            ),
        );
    }
}
