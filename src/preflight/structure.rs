//! The document level: trailer, catalog, Info dictionary, output intents, metadata, the page
//! tree, and a walk over every reachable object for streams, actions and annotations.

use std::collections::HashSet;

use lopdf::{Dictionary, Object, ObjectId, Stream};

use super::colour;
use super::content::{self, check_content_data};
use super::file;
use super::util::{
    decode, filter_names, look, name_text, resolve, text_string, Budget, Decoded, Look,
};
use super::xmp;
use super::{Loc, PreflightProfile, Run, Severity};

/// The filter names that PDF defines for streams.
const KNOWN_FILTERS: [&[u8]; 10] = [
    b"FlateDecode",
    b"LZWDecode",
    b"ASCII85Decode",
    b"ASCIIHexDecode",
    b"RunLengthDecode",
    b"CCITTFaxDecode",
    b"DCTDecode",
    b"JBIG2Decode",
    b"JPXDecode",
    b"Crypt",
];

/// The abbreviations of filter names that only inline images may use.
const ABBREVIATED_FILTERS: [&[u8]; 8] = [
    b"Fl", b"LZW", b"A85", b"AHx", b"RL", b"CCF", b"DCT", b"JBIG2",
];

/// Page boxes larger than this are beyond the limit of Acrobat.
const MAX_PAGE_UNITS: f64 = 14_400.0;
/// Page boxes smaller than this are beyond the limit of Acrobat.
const MIN_PAGE_UNITS: f64 = 3.0;
/// The most annotations of a page whose appearance streams are walked.
const MAX_ANNOTATIONS: usize = 10_000;
/// The most dangling references that are reported one by one.
const MAX_DANGLING_REPORTED: usize = 25;

pub(super) fn check_document(run: &mut Run<'_>, bytes: &[u8]) {
    file::check_loaded(run, bytes);
    let doc = run.doc;

    let root = look(doc, &doc.trailer, "Root");
    let catalog = match root {
        Look::Found(_, Object::Dictionary(c)) => Some((root.id(), c)),
        _ => {
            run.sink.error(
                "trailer-root-missing",
                Loc::default(),
                "the trailer has no /Root that refers to a catalog dictionary".to_string(),
            );
            None
        }
    };

    check_trailer(run);
    check_info(run);
    if let Some((id, catalog)) = catalog {
        check_catalog(run, id, catalog);
    }
    scan_objects(run);
    if let Some((_, catalog)) = catalog {
        walk_pages(run, catalog);
    }
    colour::finish_device_colour(run);
}

fn check_trailer(run: &mut Run<'_>) {
    let doc = run.doc;
    let profile = run.profile();

    let max_number = doc.objects.keys().map(|k| k.0).max().unwrap_or(0);
    if let Some(size) = look(doc, &doc.trailer, "Size").integer() {
        if size <= i64::from(max_number) {
            run.sink.warn(
                "trailer-size-wrong",
                Loc::default(),
                format!(
                    "the trailer /Size is {size}, but the file has an object numbered {max_number}"
                ),
            );
        }
    }

    if look(doc, &doc.trailer, "ID").is_absent() {
        let sev = if profile.is_pdfa() {
            Some(Severity::Error)
        } else if profile.is_pdfx() {
            Some(Severity::Warning)
        } else {
            None
        };
        if let Some(sev) = sev {
            run.sink.push(
                sev,
                "trailer-id-missing",
                Loc::default(),
                format!("the trailer has no /ID, which {} asks for", profile.label()),
            );
        }
    }
}

fn check_info(run: &mut Run<'_>) {
    let profile = run.profile();
    let Some(want) = profile.pdfx_identifier() else {
        return;
    };
    let doc = run.doc;
    let info = look(doc, &doc.trailer, "Info");
    let loc = Loc::at(None, info.id());
    let d = info.dict();
    let get = |key: &str| d.map(|d| look(doc, d, key)).unwrap_or(Look::Absent);

    match get("GTS_PDFXVersion")
        .string()
        .map(text_string)
        .filter(|v| !v.trim().is_empty())
    {
        None => run.sink.error(
            "pdfx-version-missing",
            loc,
            format!("the Info dictionary has no /GTS_PDFXVersion; it must say {want}"),
        ),
        Some(v) if v.trim() != want => run.sink.error(
            "pdfx-version-mismatch",
            loc,
            format!("/GTS_PDFXVersion is \"{v}\", but the profile is {want}"),
        ),
        Some(_) => {}
    }
    if get("Title").string().is_none_or(|t| t.is_empty()) {
        run.sink.error(
            "info-title-missing",
            loc,
            "the Info dictionary has no /Title".to_string(),
        );
    }
    for key in ["CreationDate", "ModDate"] {
        if get(key).string().is_none() {
            run.sink.error(
                "info-date-missing",
                loc,
                format!("the Info dictionary has no /{key}"),
            );
        }
    }
    match get("Trapped").name() {
        Some(b"True" | b"False") => {}
        other => run.sink.error(
            "trapped-missing",
            loc,
            match other {
                Some(n) => format!(
                    "/Trapped is /{}, but PDF/X needs /True or /False",
                    name_text(n)
                ),
                None => "the Info dictionary has no /Trapped of /True or /False".to_string(),
            },
        ),
    }
}

fn check_catalog(run: &mut Run<'_>, id: Option<ObjectId>, catalog: &Dictionary) {
    let doc = run.doc;
    let profile = run.profile();
    let loc = Loc::at(None, id);

    if look(doc, catalog, "Type").name() != Some(b"Catalog") {
        run.sink.warn(
            "catalog-type-wrong",
            loc,
            "the catalog has no /Type /Catalog".to_string(),
        );
    }

    check_output_intents(run, catalog, loc);
    check_metadata(run, catalog, loc);

    if profile.forbids_layers() && !look(doc, catalog, "OCProperties").is_absent() {
        run.sink.error(
            "optional-content",
            loc,
            format!(
                "the catalog has /OCProperties (layers), which {} does not allow",
                profile.label()
            ),
        );
    }

    if let Some(names) = look(doc, catalog, "Names").dict() {
        if !look(doc, names, "JavaScript").is_absent() {
            active(
                run,
                "javascript",
                "names",
                loc,
                "the catalog has a JavaScript name tree".to_string(),
            );
        }
        if !look(doc, names, "EmbeddedFiles").is_absent() {
            embedded_file(run, loc, "the catalog has an embedded files name tree");
        }
    }
}

fn check_output_intents(run: &mut Run<'_>, catalog: &Dictionary, loc: Loc) {
    let doc = run.doc;
    let profile = run.profile();

    let intents = match look(doc, catalog, "OutputIntents") {
        Look::Found(_, Object::Array(a)) => a.as_slice(),
        Look::Absent => &[],
        _ => {
            run.sink.error(
                "output-intent-invalid",
                loc,
                "/OutputIntents of the catalog is not an array".to_string(),
            );
            &[]
        }
    };

    let mut pdfx_intent = false;
    for item in intents {
        let r = resolve(doc, item);
        let iloc = Loc::at(None, r.id().or(loc.obj));
        let Some(d) = r.dict() else {
            run.sink.error(
                "output-intent-invalid",
                iloc,
                "an output intent is not a dictionary".to_string(),
            );
            continue;
        };
        run.has_output_intent = true;
        let subtype = look(doc, d, "S").name().map(<[u8]>::to_vec);
        if profile.is_pdfa() && subtype.as_deref() != Some(b"GTS_PDFA1") {
            run.sink.error(
                "output-intent-subtype",
                iloc,
                format!(
                    "the output intent has /S {}, but PDF/A needs /GTS_PDFA1",
                    subtype
                        .as_deref()
                        .map_or("(none)".to_string(), |n| format!("/{}", name_text(n)))
                ),
            );
        }
        if profile.is_pdfx() && subtype.as_deref() == Some(b"GTS_PDFX") {
            pdfx_intent = true;
        }
        if (profile.is_pdfa() || (profile.is_pdfx() && subtype.as_deref() == Some(b"GTS_PDFX")))
            && look(doc, d, "OutputConditionIdentifier").string().is_none()
        {
            run.sink.error(
                "output-intent-invalid",
                iloc,
                "the output intent has no /OutputConditionIdentifier".to_string(),
            );
        }

        match look(doc, d, "DestOutputProfile") {
            Look::Found(pid, Object::Stream(s)) => {
                let n = look(doc, &s.dict, "N").integer();
                match n {
                    Some(n @ (1 | 3 | 4)) => {
                        run.intent_components.push(n);
                        let first = pid.is_none_or(|p| run.first_visit(super::Seen::Profile, p));
                        if first {
                            if let Some(problem) = colour::profile_problem(run, s, n) {
                                run.sink.error(
                                    "icc-profile-invalid",
                                    Loc::at(None, pid.or(iloc.obj)),
                                    problem,
                                );
                            }
                        }
                    }
                    _ => run.sink.error(
                        "icc-profile-invalid",
                        Loc::at(None, pid.or(iloc.obj)),
                        "the /N of the destination profile is missing or not 1, 3 or 4".to_string(),
                    ),
                }
            }
            Look::Absent => {
                let sev = if profile.is_pdfa() {
                    Some(Severity::Error)
                } else if profile == PreflightProfile::PdfX3_2002 {
                    Some(Severity::Warning)
                } else {
                    None
                };
                if let Some(sev) = sev {
                    run.sink.push(
                        sev,
                        "output-intent-profile-missing",
                        iloc,
                        "the output intent has no /DestOutputProfile".to_string(),
                    );
                }
            }
            _ => run.sink.error(
                "output-intent-invalid",
                iloc,
                "/DestOutputProfile is not a stream that exists".to_string(),
            ),
        }
    }

    if profile.is_pdfx() && !pdfx_intent {
        run.sink.error(
            "output-intent-missing",
            loc,
            if intents.is_empty() {
                format!(
                    "the catalog has no output intent, which {} requires",
                    profile.label()
                )
            } else {
                "no output intent has /S /GTS_PDFX".to_string()
            },
        );
    }
}

fn check_metadata(run: &mut Run<'_>, catalog: &Dictionary, loc: Loc) {
    let doc = run.doc;
    let profile = run.profile();
    let pdfa = profile.is_pdfa();
    let bad = if pdfa {
        Severity::Error
    } else {
        Severity::Warning
    };

    let (id, stream) = match look(doc, catalog, "Metadata") {
        Look::Absent => {
            if pdfa {
                run.sink.error(
                    "xmp-missing",
                    loc,
                    format!(
                        "the catalog has no /Metadata (XMP), which {} requires",
                        profile.label()
                    ),
                );
            }
            return;
        }
        Look::Found(id, Object::Stream(s)) => (id, s),
        _ => {
            run.sink.push(
                bad,
                "xmp-stream-invalid",
                loc,
                "/Metadata of the catalog is not a stream that exists".to_string(),
            );
            return;
        }
    };
    let mloc = Loc::at(None, id);
    if look(doc, &stream.dict, "Subtype").name() != Some(b"XML") {
        run.sink.push(
            bad,
            "xmp-stream-invalid",
            mloc,
            "the metadata stream has no /Subtype /XML".to_string(),
        );
    }
    let data = match decode(run, stream, Budget::Validation) {
        Decoded::Data(d) => d,
        Decoded::Failed(e) => {
            run.sink.push(
                bad,
                "xmp-stream-invalid",
                mloc,
                format!("the metadata stream cannot be decoded: {e}"),
            );
            return;
        }
        Decoded::Skipped => return,
    };
    if data.starts_with(&[0xFE, 0xFF]) || data.starts_with(&[0xFF, 0xFE]) {
        run.sink.info(
            "content-not-checked",
            mloc,
            "the XMP metadata is UTF-16, which the preflight does not read".to_string(),
        );
        return;
    }
    let Ok(text) = std::str::from_utf8(&data) else {
        run.sink.push(
            bad,
            "xmp-not-well-formed",
            mloc,
            "the XMP metadata is not valid UTF-8".to_string(),
        );
        return;
    };
    if let Err(e) = xmp::check_well_formed(&data) {
        run.sink.push(
            bad,
            "xmp-not-well-formed",
            mloc,
            format!("the XMP metadata is not well-formed XML: {e}"),
        );
        return;
    }
    if pdfa {
        check_pdfa_id(run, text, mloc);
    }
}

fn check_pdfa_id(run: &mut Run<'_>, text: &str, loc: Loc) {
    let profile = run.profile();
    let (part, conformance) = xmp::pdfa_id(text);
    let (want_part, levels): (&str, &[&str]) = match profile {
        PreflightProfile::PdfA1b => ("1", &["A", "B"]),
        _ => ("2", &["A", "B", "U"]),
    };
    match part {
        None => run.sink.error(
            "pdfa-id-missing",
            loc,
            "the XMP metadata has no pdfaid:part".to_string(),
        ),
        Some(p) if p != want_part => run.sink.error(
            "pdfa-id-mismatch",
            loc,
            format!("pdfaid:part is {p}, but the profile is {}", profile.label()),
        ),
        Some(_) => {}
    }
    match conformance {
        None => run.sink.error(
            "pdfa-id-missing",
            loc,
            "the XMP metadata has no pdfaid:conformance".to_string(),
        ),
        Some(c) if !levels.contains(&c.as_str()) => run.sink.error(
            "pdfa-id-mismatch",
            loc,
            format!(
                "pdfaid:conformance is {c}, but {} needs one of {}",
                profile.label(),
                levels.join(", ")
            ),
        ),
        Some(_) => {}
    }
}

/// Reports active content that a standard forbids. `kind` makes one finding per kind.
fn active(run: &mut Run<'_>, rule: &'static str, kind: &str, loc: Loc, message: String) {
    let allowed = match rule {
        "javascript" => run.profile().allows_javascript(),
        _ => false,
    };
    if allowed {
        return;
    }
    let sev = run.sink.forbidden(Some(Severity::Warning));
    if let Some(sev) = sev {
        run.sink.once(sev, rule, kind.to_string(), loc, message);
    }
}

fn media(run: &mut Run<'_>, rule: &'static str, loc: Loc, what: &str) {
    let allowed = match rule {
        "3d-content" => run.profile().allows_3d(),
        "video-content" => run.profile().allows_video(),
        "audio-content" => run.profile().allows_audio(),
        _ => false,
    };
    if allowed {
        return;
    }
    let sev = run
        .sink
        .forbidden(Some(Severity::Info))
        .unwrap_or(Severity::Info);
    run.sink.once(
        sev,
        rule,
        what.to_string(),
        loc,
        format!(
            "{what} is used, which {} does not allow",
            run.profile().label()
        ),
    );
}

fn embedded_file(run: &mut Run<'_>, loc: Loc, what: &str) {
    let sev = match run.profile() {
        PreflightProfile::General => Severity::Info,
        PreflightProfile::PdfA2b => Severity::Warning,
        _ => Severity::Error,
    };
    let note = if run.profile() == PreflightProfile::PdfA2b {
        "; PDF/A-2 allows only embedded files that are PDF/A themselves"
    } else {
        ""
    };
    run.sink.once(
        sev,
        "embedded-file",
        String::new(),
        loc,
        format!("{what}{note}"),
    );
}

/// Looks at one dictionary of the file for actions, annotations and file specifications.
fn inspect_dict(run: &mut Run<'_>, holder: Option<ObjectId>, d: &Dictionary) {
    let doc = run.doc;
    let loc = Loc::at(None, holder);
    if d.has_type(b"StructElem") {
        return;
    }

    if let Ok(Object::Name(s)) = d.get(b"S") {
        match s.as_slice() {
            b"JavaScript" => active(
                run,
                "javascript",
                "action",
                loc,
                "a JavaScript action".to_string(),
            ),
            b"Launch" => active(
                run,
                "launch-action",
                "action",
                loc,
                "a Launch action".to_string(),
            ),
            b"Named" => {
                let n = look(doc, d, "N").name().map(<[u8]>::to_vec);
                let ok = matches!(
                    n.as_deref(),
                    Some(b"NextPage" | b"PrevPage" | b"FirstPage" | b"LastPage")
                );
                if !ok && run.profile().is_pdfa() {
                    run.sink.once(
                        Severity::Error,
                        "named-action-not-allowed",
                        String::new(),
                        loc,
                        format!(
                            "a named action /{} is used; PDF/A allows only NextPage, PrevPage, FirstPage and LastPage",
                            n.as_deref().map(name_text).unwrap_or_default()
                        ),
                    );
                }
            }
            b"GoToR" => run.sink.once(
                Severity::Warning,
                "remote-goto-action",
                String::new(),
                loc,
                "a go-to action opens another file".to_string(),
            ),
            b"URI" => {
                let uri = look(doc, d, "URI")
                    .string()
                    .map(text_string)
                    .unwrap_or_default();
                if uri
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("javascript:")
                {
                    active(
                        run,
                        "uri-javascript",
                        "action",
                        loc,
                        "a URI action with the javascript: scheme".to_string(),
                    );
                }
            }
            b"Sound" => media(run, "audio-content", loc, "a sound action"),
            b"Movie" => media(run, "video-content", loc, "a movie action"),
            b"Rendition" | b"RichMediaExecute" => media(
                run,
                "multimedia-content",
                loc,
                "a rendition or rich media action",
            ),
            b"GoTo3DView" => media(run, "3d-content", loc, "a 3D view action"),
            _ => {}
        }
    }

    let is_annot = d.has_type(b"Annot") || d.has(b"Rect");
    if is_annot {
        match look(doc, d, "Subtype").name() {
            Some(b"3D") => media(run, "3d-content", loc, "a 3D annotation"),
            Some(b"Movie") => media(run, "video-content", loc, "a movie annotation"),
            Some(b"Sound") => media(run, "audio-content", loc, "a sound annotation"),
            Some(b"Screen" | b"RichMedia") => media(
                run,
                "multimedia-content",
                loc,
                "a screen or rich media annotation",
            ),
            Some(b"FileAttachment") => embedded_file(run, loc, "a file attachment annotation"),
            _ => {}
        }
    }
    if d.has(b"EF") {
        embedded_file(run, loc, "a file specification with an embedded file");
    }
}

/// Looks at one stream: its length, its filters, whether it decodes.
fn inspect_stream(run: &mut Run<'_>, holder: Option<ObjectId>, s: &Stream) {
    let doc = run.doc;
    let profile = run.profile();
    let loc = Loc::at(None, holder);
    let d = &s.dict;

    if s.dict.has_type(b"XRef") || s.dict.has_type(b"ObjStm") {
        // Read by the loader.
        return;
    }
    if profile.is_pdfa() && (d.has(b"F") || d.has(b"FFilter")) {
        run.sink.error(
            "stream-external-file",
            loc,
            "a stream keeps its data in an external file (/F)".to_string(),
        );
    }

    let Some(names) = filter_names(doc, s) else {
        run.sink.error(
            "stream-filter-unknown",
            loc,
            "/Filter is not a name or an array of names".to_string(),
        );
        return;
    };
    for n in &names {
        let shown = name_text(n);
        if n == b"LZWDecode" && profile.is_pdfa() {
            run.sink.once(
                Severity::Error,
                "lzw-filter",
                String::new(),
                loc,
                "a stream uses the LZWDecode filter, which PDF/A does not allow".to_string(),
            );
        }
        let newer = (n == b"JPXDecode"
            && (profile == PreflightProfile::PdfA1b || profile.is_pdfx()))
            || (n == b"JBIG2Decode" && profile.is_pdfx())
            || (n == b"BrotliDecode" && profile.is_standard());
        if newer {
            run.sink.once(
                Severity::Error,
                "filter-not-in-base-version",
                shown.clone(),
                loc,
                format!(
                    "a stream uses the {shown} filter, which {} does not have",
                    profile.label()
                ),
            );
        }
        if ABBREVIATED_FILTERS.contains(&n.as_slice()) {
            run.sink.error(
                "stream-filter-unknown",
                loc,
                format!(
                    "the filter name /{shown} is an abbreviation, which only inline images may use"
                ),
            );
        } else if !KNOWN_FILTERS.contains(&n.as_slice()) && n != b"BrotliDecode" {
            run.sink.error(
                "stream-filter-unknown",
                loc,
                format!("/{shown} is not a filter that PDF defines"),
            );
        }
    }

    let failed =
        if names.first().is_some_and(|n| n == b"FlateDecode") && doc.encryption_state.is_none() {
            // lopdf accepts damaged Flate data and returns what it can read, so the first Flate
            // layer is checked with the decoder directly.
            match (strict_flate(run, s), names.len()) {
                (Err(e), _) => Some(e),
                (Ok(()), 1) => None,
                (Ok(()), _) => match decode(run, s, Budget::Validation) {
                    Decoded::Failed(e) => Some(e),
                    _ => None,
                },
            }
        } else {
            match decode(run, s, Budget::Validation) {
                Decoded::Failed(e) => Some(e),
                _ => None,
            }
        };
    if let Some(e) = failed {
        if let Some(id) = holder {
            run.decode_failed.insert(id);
        }
        run.sink.error(
            "stream-decode-failed",
            loc,
            format!("the stream cannot be decoded: {e}"),
        );
    }
}

/// Decodes the Flate data of `s` without keeping it. Fails on damaged data, a missing end or
/// a wrong checksum.
fn strict_flate(run: &mut Run<'_>, s: &Stream) -> Result<(), String> {
    use std::io::Read;

    let cap = run.validation_left.min(super::MAX_STREAM_BYTES);
    let mut decoder = flate2::read::ZlibDecoder::new(s.content.as_slice());
    let mut buf = [0u8; 16 * 1024];
    let mut total = 0usize;
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total = total.saturating_add(n);
                if total > cap {
                    run.sink
                        .limit("the limit of decoded stream bytes was reached");
                    run.validation_left = run.validation_left.saturating_sub(total);
                    return Ok(());
                }
            }
            Err(e) => return Err(format!("the Flate data is damaged: {e}")),
        }
    }
    run.validation_left = run.validation_left.saturating_sub(total);
    Ok(())
}

/// Walks every object that is reachable from the trailer.
fn scan_objects(run: &mut Run<'_>) {
    let doc = run.doc;
    let mut seen: HashSet<ObjectId> = HashSet::new();
    let mut stack: Vec<(Option<ObjectId>, &Object)> =
        doc.trailer.iter().map(|(_, v)| (None, v)).collect();
    let mut dangling = 0usize;

    while let Some((holder, obj)) = stack.pop() {
        if !run.visit() {
            break;
        }
        match obj {
            Object::Reference(r) => match doc.objects.get(r) {
                Some(target) => {
                    if seen.insert(*r) {
                        stack.push((Some(*r), target));
                    }
                }
                None => {
                    dangling = dangling.saturating_add(1);
                    if dangling <= MAX_DANGLING_REPORTED {
                        run.sink.warn(
                            "dangling-reference",
                            Loc::at(None, holder),
                            format!(
                                "a reference to object {} {}, which does not exist (it counts as null)",
                                r.0, r.1
                            ),
                        );
                    }
                }
            },
            Object::Array(items) => stack.extend(items.iter().map(|o| (holder, o))),
            Object::Dictionary(d) => {
                inspect_dict(run, holder, d);
                stack.extend(d.iter().map(|(_, v)| (holder, v)));
            }
            Object::Stream(s) => {
                inspect_dict(run, holder, &s.dict);
                inspect_stream(run, holder, s);
                stack.extend(s.dict.iter().map(|(_, v)| (holder, v)));
            }
            _ => {}
        }
    }
    if dangling > MAX_DANGLING_REPORTED {
        run.sink.warn(
            "dangling-reference",
            Loc::default(),
            format!(
                "{} more references to objects that do not exist",
                dangling - MAX_DANGLING_REPORTED
            ),
        );
    }
}

/// Values that a page inherits from its ancestors.
#[derive(Clone, Copy, Default)]
struct Inherited<'a> {
    resources: Option<(&'a Dictionary, Option<ObjectId>)>,
    media_box: Option<&'a Object>,
    crop_box: Option<&'a Object>,
    rotate: Option<&'a Object>,
}

impl<'a> Inherited<'a> {
    fn with(self, run: &Run<'a>, node: &'a Dictionary) -> Inherited<'a> {
        let doc = run.doc;
        let mut next = self;
        if let Look::Found(id, Object::Dictionary(r)) = look(doc, node, "Resources") {
            next.resources = Some((r, id));
        }
        if let Ok(o) = node.get(b"MediaBox") {
            next.media_box = Some(o);
        }
        if let Ok(o) = node.get(b"CropBox") {
            next.crop_box = Some(o);
        }
        if let Ok(o) = node.get(b"Rotate") {
            next.rotate = Some(o);
        }
        next
    }
}

fn walk_pages<'a>(run: &mut Run<'a>, catalog: &'a Dictionary) {
    let doc = run.doc;
    let root = look(doc, catalog, "Pages");
    let Look::Found(root_id, Object::Dictionary(root_dict)) = root else {
        run.sink.error(
            "pages-tree-missing",
            Loc::default(),
            "the catalog has no /Pages that refers to a page tree dictionary".to_string(),
        );
        return;
    };

    let mut visited: HashSet<ObjectId> = HashSet::new();
    let mut stack: Vec<(Option<ObjectId>, &'a Dictionary, Inherited<'a>, usize)> =
        vec![(root_id, root_dict, Inherited::default(), 0)];
    let mut count = 0usize;
    let mut truncated = false;

    while let Some((id, node, inherited, depth)) = stack.pop() {
        if depth > super::MAX_TREE_DEPTH {
            run.sink
                .limit("a page tree nested more than 64 levels deep was not followed");
            truncated = true;
            continue;
        }
        if let Some(id) = id {
            if !visited.insert(id) {
                run.sink.error(
                    "page-tree-cycle",
                    Loc::at(None, Some(id)),
                    "the page tree contains this node more than once or contains itself"
                        .to_string(),
                );
                continue;
            }
        }
        if !run.visit() {
            truncated = true;
            break;
        }
        let inherited = inherited.with(run, node);
        let kids = look(doc, node, "Kids").array();
        let is_pages = match look(doc, node, "Type").name() {
            Some(b"Pages") => true,
            Some(b"Page") => false,
            _ => kids.is_some(),
        };

        if is_pages {
            let Some(kids) = kids else {
                run.sink.error(
                    "page-tree-node-invalid",
                    Loc::at(None, id),
                    "a /Pages node has no /Kids array".to_string(),
                );
                continue;
            };
            for kid in kids.iter().rev() {
                match resolve(doc, kid) {
                    Look::Found(kid_id, Object::Dictionary(d)) => {
                        stack.push((kid_id, d, inherited, depth.saturating_add(1)));
                    }
                    _ => run.sink.error(
                        "page-tree-node-invalid",
                        Loc::at(None, id),
                        "an entry of /Kids is not a dictionary or refers to an object that does not exist".to_string(),
                    ),
                }
            }
        } else {
            if count >= super::MAX_PAGES {
                run.sink.limit("the limit of 100,000 pages was reached");
                truncated = true;
                break;
            }
            count += 1;
            run.sink.pages = count;
            check_page(run, count, id, node, inherited);
        }
    }

    if !truncated {
        if count == 0 {
            run.sink.warn(
                "no-pages",
                Loc::default(),
                "the document has no pages".to_string(),
            );
        }
        if let Some(declared) = look(doc, root_dict, "Count").integer() {
            if usize::try_from(declared).ok() != Some(count) {
                run.sink.warn(
                    "page-count-mismatch",
                    Loc::at(None, root_id),
                    format!(
                        "/Count of the page tree is {declared}, but the tree has {count} pages"
                    ),
                );
            }
        }
    }
}

fn contains(outer: [f64; 4], inner: [f64; 4]) -> bool {
    let eps = 0.01;
    inner[0] >= outer[0] - eps
        && inner[1] >= outer[1] - eps
        && inner[2] <= outer[2] + eps
        && inner[3] <= outer[3] + eps
}

fn check_page<'a>(
    run: &mut Run<'a>,
    n: usize,
    id: Option<ObjectId>,
    page: &'a Dictionary,
    inh: Inherited<'a>,
) {
    let doc = run.doc;
    let profile = run.profile();
    let loc = Loc::at(Some(n), id);

    // MediaBox
    let media_obj = page.get(b"MediaBox").ok().or(inh.media_box);
    let media = match media_obj.map(|o| resolve(doc, o)) {
        None | Some(Look::Absent) => {
            run.sink.error(
                "mediabox-missing",
                loc,
                "the page has no /MediaBox, own or inherited".to_string(),
            );
            None
        }
        Some(_) => match media_obj.and_then(|o| run.rect(o)) {
            None => {
                run.sink.error(
                    "mediabox-invalid",
                    loc,
                    "/MediaBox is not an array of four numbers".to_string(),
                );
                None
            }
            Some(r) => {
                let (w, h) = (r[2] - r[0], r[3] - r[1]);
                if w <= 0.0 || h <= 0.0 {
                    run.sink
                        .error("mediabox-invalid", loc, "/MediaBox has no area".to_string());
                } else if w > MAX_PAGE_UNITS || h > MAX_PAGE_UNITS {
                    run.sink.warn(
                        "page-size-huge",
                        loc,
                        format!(
                            "the page is {w:.0} x {h:.0} units; Acrobat supports at most 14400"
                        ),
                    );
                } else if w < MIN_PAGE_UNITS || h < MIN_PAGE_UNITS {
                    run.sink.warn(
                        "page-size-tiny",
                        loc,
                        format!("the page is {w:.2} x {h:.2} units; Acrobat supports at least 3"),
                    );
                }
                Some(r)
            }
        },
    };

    // The other boxes
    let boxes: [(&str, Option<&Object>); 4] = [
        ("CropBox", page.get(b"CropBox").ok().or(inh.crop_box)),
        ("BleedBox", page.get(b"BleedBox").ok()),
        ("TrimBox", page.get(b"TrimBox").ok()),
        ("ArtBox", page.get(b"ArtBox").ok()),
    ];
    for (key, obj) in boxes {
        let Some(obj) = obj else { continue };
        if resolve(doc, obj).is_absent() {
            continue;
        }
        match run.rect(obj) {
            None => run.sink.error(
                "page-box-invalid",
                loc,
                format!("/{key} is not an array of four numbers"),
            ),
            Some(r) => {
                if let Some(m) = media {
                    if !contains(m, r) {
                        let sev = if profile.is_pdfx() && key != "CropBox" {
                            Severity::Error
                        } else {
                            Severity::Warning
                        };
                        run.sink.push(
                            sev,
                            "page-box-outside-mediabox",
                            loc,
                            format!("/{key} is not inside the /MediaBox"),
                        );
                    }
                }
            }
        }
    }

    if let Some(rot) = page.get(b"Rotate").ok().or(inh.rotate) {
        let value = resolve(doc, rot);
        if !value.is_absent() && value.integer().is_none_or(|v| v % 90 != 0) {
            run.sink.warn(
                "page-rotate-invalid",
                loc,
                "/Rotate is not an integer multiple of 90".to_string(),
            );
        }
    }

    if profile.is_pdfx() {
        let trim = !look(doc, page, "TrimBox").is_absent();
        let art = !look(doc, page, "ArtBox").is_absent();
        if !trim && !art {
            run.sink.error(
                "trimbox-missing",
                loc,
                "the page has neither /TrimBox nor /ArtBox, one of which PDF/X requires"
                    .to_string(),
            );
        }
        if trim && art {
            run.sink.error(
                "trimbox-and-artbox",
                loc,
                "the page has both /TrimBox and /ArtBox; PDF/X allows only one".to_string(),
            );
        }
        if let Some(g) = look(doc, page, "Group").dict() {
            if look(doc, g, "S").name() == Some(b"Transparency") {
                run.sink.once(
                    Severity::Error,
                    "transparency-group",
                    "page".to_string(),
                    loc,
                    format!(
                        "the page has a transparency group, which {} does not allow",
                        profile.label()
                    ),
                );
            }
        }
    }

    let resources = inh.resources.map(|(r, _)| r);
    if let Some((r, rid)) = inh.resources {
        content::check_resources(run, Some(n), r, rid);
    }

    check_page_contents(run, n, id, page, resources);
    check_annotations(run, n, page, resources);
}

fn check_page_contents<'a>(
    run: &mut Run<'a>,
    n: usize,
    id: Option<ObjectId>,
    page: &'a Dictionary,
    resources: Option<&'a Dictionary>,
) {
    let doc = run.doc;
    let loc = Loc::at(Some(n), id);
    let bad = |run: &mut Run<'_>, what: &str| {
        run.sink
            .error("contents-invalid", loc, format!("/Contents {what}"));
    };

    let mut streams: Vec<(Option<ObjectId>, &'a Stream)> = Vec::new();
    match look(doc, page, "Contents") {
        Look::Absent => return,
        Look::Dangling(_) => {
            bad(run, "refers to an object that does not exist");
            return;
        }
        Look::Found(sid, Object::Stream(s)) => streams.push((sid, s)),
        Look::Found(_, Object::Array(items)) => {
            for item in items {
                match resolve(doc, item) {
                    Look::Found(sid, Object::Stream(s)) => streams.push((sid, s)),
                    _ => bad(
                        run,
                        "has an array entry that is not a stream or does not exist",
                    ),
                }
            }
        }
        Look::Found(_, _) => {
            bad(run, "is not a stream or an array of streams");
            return;
        }
    }

    let mut joined: Vec<u8> = Vec::new();
    for (sid, s) in streams {
        match decode(run, s, Budget::Content) {
            Decoded::Data(d) => {
                joined.extend_from_slice(&d);
                joined.push(b'\n');
            }
            Decoded::Failed(e) => {
                let reported = sid.is_some_and(|i| run.decode_failed.contains(&i));
                if !reported {
                    run.sink.error(
                        "content-stream-unreadable",
                        Loc::at(Some(n), sid.or(id)),
                        format!("the content stream cannot be decoded: {e}"),
                    );
                }
            }
            Decoded::Skipped => {}
        }
    }
    check_content_data(run, Some(n), &joined, resources, 0, id);
}

fn check_annotations<'a>(
    run: &mut Run<'a>,
    n: usize,
    page: &'a Dictionary,
    resources: Option<&'a Dictionary>,
) {
    let doc = run.doc;
    let Some(annots) = look(doc, page, "Annots").array() else {
        return;
    };
    for annot in annots.iter().take(MAX_ANNOTATIONS) {
        let Some(d) = resolve(doc, annot).dict() else {
            continue;
        };
        let Some(ap) = look(doc, d, "AP").dict() else {
            continue;
        };
        let normal = look(doc, ap, "N");
        match normal {
            Look::Found(id, Object::Stream(s)) => {
                content::check_appearance(run, n, id, s, resources)
            }
            Look::Found(_, Object::Dictionary(states)) => {
                for (_, v) in states.iter().take(64) {
                    if let Look::Found(id, Object::Stream(s)) = resolve(doc, v) {
                        content::check_appearance(run, n, id, s, resources);
                    }
                }
            }
            _ => {}
        }
    }
}
