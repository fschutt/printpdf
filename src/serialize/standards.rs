//! What a PDF standard (PDF/A, PDF/X) adds to the file, and what it warns about.
//!
//! [`apply`] runs once the pages, forms, fonts and patterns are written, and before the catalog
//! is stored. It looks at the objects that were written (not at the model, so forms, patterns and
//! images are covered the same way) to find out which device colour spaces, transparency features,
//! filters and active content the file uses. Then it
//!
//! - sets the header version of PDF/A files,
//! - adds the output intent: the FOGRA39 CMYK profile with `/S /GTS_PDFX` for PDF/X, and for PDF/A
//!   an sRGB profile (or the FOGRA39 profile when only CMYK is used) with `/S /GTS_PDFA1`,
//! - adds the XMP metadata stream,
//! - and pushes a warning for everything the standard does not allow. Content is never changed.

use std::collections::BTreeSet;

use lopdf::{
    content::Content,
    Dictionary as LoDictionary,
    Document, Object,
    Object::{Array, Dictionary, Integer, Name, Reference, Stream, String as LoString},
    ObjectId, Stream as LoStream,
    StringFormat::Literal,
};

use super::icc_to_stream;
use crate::{
    color::IccProfile, IccProfileType, PdfConformance, PdfDocument, PdfWarnMsg,
};

/// The sRGB profile of PDF/A files. A copy of `sRGB-v4.icc` of svg2pdf 0.13.0, licensed CC0 (see
/// `sRGB-v4.icc.LICENSE.txt` next to it).
const SRGB_PROFILE: &[u8] = include_bytes!("../res/sRGB-v4.icc");
/// The CMYK profile "Coated FOGRA39".
const FOGRA39_PROFILE: &[u8] = include_bytes!("../res/CoatedFOGRA39.icc");
const FOGRA39_LICENSE: &str = include_str!("../res/CoatedFOGRA39.icc.LICENSE.txt");

/// The most nesting that the scan follows in one object.
const MAX_DEPTH: usize = 32;
/// The most decoded bytes of one content stream that the scan reads.
const MAX_CONTENT: usize = 256 * 1024 * 1024;

/// What the written objects use, as far as the standards care.
#[derive(Debug, Default)]
pub(super) struct Usage {
    rgb: bool,
    cmyk: bool,
    alpha: bool,
    soft_mask: bool,
    blend_mode: bool,
    group: bool,
    lzw: bool,
    jpx: bool,
    jbig2: bool,
    javascript: bool,
    launch: bool,
    embedded_file: bool,
    layers: bool,
}

/// The ids of the content streams of the pages.
fn page_content_ids(doc: &Document) -> BTreeSet<ObjectId> {
    let mut ids = BTreeSet::new();
    for object in doc.objects.values() {
        let Object::Dictionary(page) = object else {
            continue;
        };
        if !matches!(page.get(b"Type"), Ok(Name(n)) if n == b"Page") {
            continue;
        }
        match page.get(b"Contents") {
            Ok(Reference(id)) => {
                ids.insert(*id);
            }
            Ok(Array(items)) => {
                for item in items {
                    if let Reference(id) = item {
                        ids.insert(*id);
                    }
                }
            }
            _ => {}
        }
    }
    ids
}

/// Scans every object of `doc`.
fn scan(doc: &Document) -> Usage {
    let mut usage = Usage::default();
    let contents = page_content_ids(doc);
    for (id, object) in &doc.objects {
        match object {
            Stream(stream) => {
                scan_dictionary(doc, &stream.dict, &mut usage, 0);
                let subtype = stream.dict.get(b"Subtype").ok();
                let is_form = matches!(subtype, Some(Name(n)) if n == b"Form");
                let is_tiling = matches!(stream.dict.get(b"PatternType"), Ok(Integer(1)));
                if contents.contains(id) || is_form || is_tiling {
                    scan_content(stream, &mut usage);
                }
            }
            other => scan_object(doc, other, None, &mut usage, 0),
        }
    }
    usage
}

fn scan_object(doc: &Document, object: &Object, key: Option<&[u8]>, usage: &mut Usage, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    match object {
        Name(name) => scan_name(name, key, usage),
        Array(items) => {
            for item in items {
                scan_object(doc, item, key, usage, depth.saturating_add(1));
            }
        }
        Dictionary(dict) => scan_dictionary(doc, dict, usage, depth.saturating_add(1)),
        _ => {}
    }
}

fn number(object: &Object) -> Option<f32> {
    match object {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

fn scan_dictionary(doc: &Document, dict: &LoDictionary, usage: &mut Usage, depth: usize) {
    for (key, value) in dict.iter() {
        let key = key.as_slice();
        match key {
            b"CA" | b"ca" => {
                if number(value).is_some_and(|v| v < 1.0) {
                    usage.alpha = true;
                }
            }
            b"SMask" => {
                if !matches!(value, Name(n) if n == b"None") {
                    usage.soft_mask = true;
                }
            }
            b"BM" => {
                let first = match value {
                    Name(n) => Some(n.as_slice()),
                    Array(items) => match items.first() {
                        Some(Name(n)) => Some(n.as_slice()),
                        _ => None,
                    },
                    _ => None,
                };
                if first.is_some_and(|n| n != b"Normal" && n != b"Compatible") {
                    usage.blend_mode = true;
                }
            }
            b"Group" => {
                let group = match value {
                    Dictionary(d) => Some(d),
                    Reference(id) => match doc.objects.get(id) {
                        Some(Dictionary(d)) => Some(d),
                        _ => None,
                    },
                    _ => None,
                };
                if group.is_some_and(|g| matches!(g.get(b"S"), Ok(Name(n)) if n == b"Transparency")) {
                    usage.group = true;
                }
            }
            b"JS" => usage.javascript = true,
            b"OC" | b"OCProperties" => usage.layers = true,
            b"EF" | b"EmbeddedFiles" => usage.embedded_file = true,
            _ => {}
        }
        scan_object(doc, value, Some(key), usage, depth);
    }
}

fn scan_name(name: &[u8], key: Option<&[u8]>, usage: &mut Usage) {
    let is = |want: &[u8]| key == Some(want);
    match name {
        // The alternate of an ICC profile is not a use of the device space. Gray needs no flag:
        // every output intent covers it.
        b"DeviceRGB" if !is(b"Alternate") => usage.rgb = true,
        b"DeviceCMYK" if !is(b"Alternate") => usage.cmyk = true,
        b"LZWDecode" => usage.lzw = true,
        b"JPXDecode" => usage.jpx = true,
        b"JBIG2Decode" => usage.jbig2 = true,
        b"JavaScript" if is(b"S") => usage.javascript = true,
        b"Launch" if is(b"S") => usage.launch = true,
        b"OCG" | b"OCMD" if is(b"Type") => usage.layers = true,
        b"EmbeddedFile" if is(b"Type") => usage.embedded_file = true,
        _ => {}
    }
}

/// The colour operators and the colour space names of a content stream.
fn scan_content(stream: &LoStream, usage: &mut Usage) {
    let Ok(bytes) = stream.decompressed_content_with_limit(MAX_CONTENT) else {
        return;
    };
    let Ok(content) = Content::decode(&bytes) else {
        return;
    };
    for operation in &content.operations {
        match operation.operator.as_str() {
            "rg" | "RG" => usage.rgb = true,
            "k" | "K" => usage.cmyk = true,
            _ => {}
        }
        for operand in &operation.operands {
            if let Name(name) = operand {
                scan_name(name, None, usage);
            }
        }
    }
}

/// Which profile the output intent of a PDF/A file carries.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PdfaProfile {
    Srgb,
    Fogra39,
}

/// The output intent and the XMP metadata of the catalog, the header version, and the warnings
/// about content that the standard does not allow. See the module docs.
pub(super) fn apply(
    pdf: &PdfDocument,
    doc: &mut Document,
    catalog: &mut LoDictionary,
    warnings: &mut Vec<PdfWarnMsg>,
) {
    let conformance = &pdf.metadata.info.conformance;
    // Reading every content stream costs time that a document without a standard does not
    // need to spend.
    let needs_scan = conformance.is_pdfa()
        || !conformance.is_transparency_allowed()
        || !conformance.is_layering_allowed();
    let usage = if needs_scan { scan(doc) } else { Usage::default() };
    doc.version = conformance.header_version().to_string();

    let mut pdfa_profile = None;
    if conformance.must_have_icc_profile() {
        let profile = if conformance.is_pdfa() {
            // One output intent can name one profile. RGB and gray are the common case; a
            // document that only uses CMYK gets the CMYK profile.
            let profile = if usage.cmyk && !usage.rgb {
                PdfaProfile::Fogra39
            } else {
                PdfaProfile::Srgb
            };
            pdfa_profile = Some(profile);
            Some(profile)
        } else {
            None
        };
        let intent = output_intent(doc, profile);
        catalog.set("OutputIntents", Array(vec![Dictionary(intent)]));
    }

    if conformance.must_have_xmp_metadata() {
        let xmp = Stream(LoStream::new(
            LoDictionary::from_iter(vec![("Type", "Metadata".into()), ("Subtype", "XML".into())]),
            pdf.metadata.xmp_metadata_string().as_bytes().to_vec(),
        ));
        let metadata_id = doc.add_object(xmp);
        catalog.set("Metadata", Reference(metadata_id));
    }

    warn(pdf, conformance, &usage, pdfa_profile, warnings);
}

/// The output intent dictionary: `/GTS_PDFA1` with sRGB or FOGRA39 for PDF/A, `/GTS_PDFX` with
/// FOGRA39 for the rest. The profile is a new object of `doc`.
fn output_intent(doc: &mut Document, pdfa: Option<PdfaProfile>) -> LoDictionary {
    let registry = LoString("http://www.color.org".into(), Literal);
    if pdfa == Some(PdfaProfile::Srgb) {
        let profile = IccProfile::new(SRGB_PROFILE.to_vec(), IccProfileType::Rgb)
            .with_alternate_profile(false)
            .with_range(false);
        let profile_id = doc.add_object(Stream(icc_to_stream(&profile)));
        return LoDictionary::from_iter(vec![
            ("Type", Name("OutputIntent".into())),
            ("S", Name("GTS_PDFA1".into())),
            ("OutputConditionIdentifier", LoString("sRGB".into(), Literal)),
            ("OutputCondition", LoString("sRGB".into(), Literal)),
            ("RegistryName", registry),
            ("Info", LoString("sRGB (ICC profile version 4)".into(), Literal)),
            ("DestOutputProfile", Reference(profile_id)),
        ]);
    }

    let description = "Commercial and special offset print acccording to ISO \
                       12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                       offset paper, 115 g/m2), screen ruling 60/cm";
    let profile = IccProfile::new(FOGRA39_PROFILE.to_vec(), IccProfileType::Cmyk)
        .with_alternate_profile(false)
        .with_range(true);
    let profile_id = doc.add_object(Stream(icc_to_stream(&profile)));
    let subtype = if pdfa.is_some() { "GTS_PDFA1" } else { "GTS_PDFX" };
    LoDictionary::from_iter(vec![
        ("S", Name(subtype.into())),
        ("OutputCondition", LoString(description.into(), Literal)),
        ("License", LoString(FOGRA39_LICENSE.into(), Literal)),
        ("Type", Name("OutputIntent".into())),
        ("OutputConditionIdentifier", LoString("FOGRA39".into(), Literal)),
        ("RegistryName", registry),
        ("Info", LoString("Coated FOGRA39 (ISO 12647-2:2004)".into(), Literal)),
        ("DestOutputProfile", Reference(profile_id)),
    ])
}

/// Warnings about what the file uses and the standard does not allow.
fn warn(
    pdf: &PdfDocument,
    conformance: &PdfConformance,
    usage: &Usage,
    pdfa_profile: Option<PdfaProfile>,
    warnings: &mut Vec<PdfWarnMsg>,
) {
    let label = conformance.get_identifier_string();
    let mut push = |text: String| warnings.push(PdfWarnMsg::warning(0, 0, format!("{label}: {text}")));

    match conformance.pdfa_level() {
        Some('A') => push(
            "level a needs tagged document structure (/StructTreeRoot and /MarkInfo), which \
             printpdf does not write. The file claims level b (pdfaid:conformance B)."
                .to_string(),
        ),
        Some('U') => push(
            "level u needs a Unicode mapping for all text, which printpdf does not check. The \
             file claims level b (pdfaid:conformance B)."
                .to_string(),
        ),
        _ => {}
    }

    if !conformance.is_transparency_allowed() {
        let mut found = Vec::new();
        if usage.alpha {
            found.push("an alpha value below 1 (/CA or /ca)");
        }
        if usage.soft_mask {
            found.push("a soft mask (/SMask)");
        }
        if usage.blend_mode {
            found.push("a blend mode");
        }
        if usage.group {
            found.push("a transparency group");
        }
        if !found.is_empty() {
            push(format!(
                "the document uses transparency ({}), which this standard does not allow. It is \
                 written as it is.",
                found.join(", ")
            ));
        }
    }

    if !conformance.is_layering_allowed() && (usage.layers || !pdf.resources.layers.map.is_empty())
    {
        push(
            "the document has layers (optional content), which this standard does not allow. \
             They are written as they are."
                .to_string(),
        );
    }

    if conformance.is_pdfa() {
        if usage.lzw {
            push("a stream uses the LZW filter, which PDF/A does not allow.".to_string());
        }
        if conformance.pdfa_part() == Some(1) && (usage.jpx || usage.jbig2) {
            push(
                "a stream uses the JPEG 2000 or JBIG2 filter, which PDF/A-1 does not allow."
                    .to_string(),
            );
        }
        if usage.javascript {
            push("the document has JavaScript, which PDF/A does not allow.".to_string());
        }
        if usage.launch {
            push("the document has a launch action, which PDF/A does not allow.".to_string());
        }
        if usage.embedded_file && conformance.pdfa_part() != Some(3) {
            push(
                "the document has an embedded file, which only PDF/A-3 allows.".to_string(),
            );
        }
        if usage.cmyk && pdfa_profile == Some(PdfaProfile::Srgb) {
            push(
                "the document uses DeviceCMYK and DeviceRGB, but the file has one output intent, \
                 the sRGB profile, which does not cover DeviceCMYK. Use one of the two colour \
                 models."
                    .to_string(),
            );
        }
    }

    if matches!(
        *conformance,
        PdfConformance::X1A_2001_PDF_1_3 | PdfConformance::X1A_2003_PDF_1_4
    ) && usage.rgb
    {
        push("DeviceRGB is used, but PDF/X-1a allows only gray, CMYK and spot colours.".to_string());
    }
}
