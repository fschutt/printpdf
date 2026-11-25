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

/// The sRGB profile of PDF/A-2 and PDF/A-3 files (ICC version 4.2). A copy of `sRGB-v4.icc` of
/// svg2pdf 0.13.0, licensed CC0 (see `sRGB-v4.icc.LICENSE.txt` next to it). PDF/A-1 needs a
/// version 2 profile, which [`srgb_v2_profile`] builds.
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
        let intent = output_intent(doc, profile, conformance.pdfa_part());
        catalog.set("OutputIntents", Array(vec![Dictionary(intent)]));
    }

    if conformance.must_have_xmp_metadata() {
        let xmp = Stream(LoStream::new(
            LoDictionary::from_iter(vec![("Type", "Metadata".into()), ("Subtype", "XML".into())]),
            pdf.metadata.xmp_metadata_string().as_bytes().to_vec(),
        )
        // PDF/A: the metadata stream has no filter.
        .with_compression(false));
        let metadata_id = doc.add_object(xmp);
        catalog.set("Metadata", Reference(metadata_id));
    }

    if conformance.pdfa_part() == Some(1) {
        add_cid_sets(doc);
    }
    if pdfa_profile == Some(PdfaProfile::Fogra39) && drop_rgb_annotation_colors(doc) {
        warnings.push(PdfWarnMsg::warning(
            0,
            0,
            format!(
                "{}: the output intent is a CMYK profile, so the RGB colour (/C) of link \
                 annotations is left out; PDF/A allows an annotation colour only with an RGB \
                 output intent.",
                conformance.get_identifier_string()
            ),
        ));
    }

    warn(pdf, conformance, &usage, pdfa_profile, warnings);
}

/// Adds a `/CIDSet` to the descriptor of every subset TrueType CID font. PDF/A-1 asks for it: a
/// bitmap with a bit for each CID of the embedded font, and the CIDs are the glyph ids.
fn add_cid_sets(doc: &mut Document) {
    // The descriptors of CIDFontType2 fonts, which sit in the `/DescendantFonts` of Type0 fonts.
    let mut descriptors = Vec::new();
    for object in doc.objects.values() {
        let Dictionary(font) = object else {
            continue;
        };
        let Ok(Array(descendants)) = font.get(b"DescendantFonts") else {
            continue;
        };
        for descendant in descendants {
            let Dictionary(cid_font) = descendant else {
                continue;
            };
            if matches!(cid_font.get(b"Subtype"), Ok(Name(n)) if n == b"CIDFontType2") {
                if let Ok(Reference(id)) = cid_font.get(b"FontDescriptor") {
                    descriptors.push(*id);
                }
            }
        }
    }
    for id in descriptors {
        let Some(Dictionary(descriptor)) = doc.objects.get(&id) else {
            continue;
        };
        let subset = matches!(
            descriptor.get(b"FontName"),
            Ok(Name(n)) if n.get(6) == Some(&b'+')
        );
        if !subset || descriptor.has(b"CIDSet") {
            continue;
        }
        let Ok(Reference(file)) = descriptor.get(b"FontFile2") else {
            continue;
        };
        let glyphs = match doc.objects.get(file) {
            Some(Stream(program)) => program
                .decompressed_content()
                .ok()
                .and_then(|bytes| glyph_count(&bytes)),
            _ => None,
        };
        let Some(glyphs) = glyphs else {
            continue;
        };
        let mut bits = vec![0u8; glyphs.div_ceil(8)];
        for cid in 0..glyphs {
            if let Some(byte) = bits.get_mut(cid / 8) {
                *byte |= 0x80 >> (cid % 8);
            }
        }
        let set_id = doc.add_object(Stream(LoStream::new(LoDictionary::new(), bits)));
        if let Some(Dictionary(descriptor)) = doc.objects.get_mut(&id) {
            descriptor.set("CIDSet", Reference(set_id));
        }
    }
}

/// Removes the `/C` entry of annotations when it holds three numbers (an RGB colour). Returns
/// whether it removed one.
fn drop_rgb_annotation_colors(doc: &mut Document) -> bool {
    let mut removed = false;
    for object in doc.objects.values_mut() {
        let Dictionary(page) = object else {
            continue;
        };
        if !matches!(page.get(b"Type"), Ok(Name(n)) if n == b"Page") {
            continue;
        }
        let Ok(Array(annotations)) = page.get_mut(b"Annots") else {
            continue;
        };
        for annotation in annotations.iter_mut() {
            let Dictionary(annotation) = annotation else {
                continue;
            };
            if matches!(annotation.get(b"C"), Ok(Array(c)) if c.len() == 3) {
                annotation.remove(b"C");
                removed = true;
            }
        }
    }
    removed
}

/// The number of glyphs of a TrueType font file (`maxp`).
fn glyph_count(font: &[u8]) -> Option<usize> {
    let tables = super::builtin_font::read_tables(font)?;
    let (_, maxp) = tables.iter().find(|(tag, _)| tag == b"maxp")?;
    super::builtin_font::be16(maxp, 4).map(usize::from)
}

/// The output intent dictionary: `/GTS_PDFA1` with sRGB or FOGRA39 for PDF/A, `/GTS_PDFX` with
/// FOGRA39 for the rest. The profile is a new object of `doc`.
fn output_intent(doc: &mut Document, pdfa: Option<PdfaProfile>, part: Option<u8>) -> LoDictionary {
    let registry = LoString("http://www.color.org".into(), Literal);
    if pdfa == Some(PdfaProfile::Srgb) {
        let (bytes, info) = if part == Some(1) {
            (srgb_v2_profile(), "sRGB (ICC profile version 2)")
        } else {
            (SRGB_PROFILE.to_vec(), "sRGB (ICC profile version 4)")
        };
        let profile = IccProfile::new(bytes, IccProfileType::Rgb)
            .with_alternate_profile(false)
            .with_range(false);
        let profile_id = doc.add_object(Stream(icc_to_stream(&profile)));
        return LoDictionary::from_iter(vec![
            ("Type", Name("OutputIntent".into())),
            ("S", Name("GTS_PDFA1".into())),
            ("OutputConditionIdentifier", LoString("sRGB".into(), Literal)),
            ("OutputCondition", LoString("sRGB".into(), Literal)),
            ("RegistryName", registry),
            ("Info", LoString(info.into(), Literal)),
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

/// An sRGB profile of ICC version 2.1 for PDF/A-1, which is based on ICC version 2: a matrix and
/// tone curve profile with the D50-adapted sRGB colorants and the sRGB transfer function as a
/// table of 1024 entries. It is written byte by byte here, so there is no file to copy.
fn srgb_v2_profile() -> Vec<u8> {
    fn s15(v: f64) -> [u8; 4] {
        ((v * 65536.0).round() as i32).to_be_bytes()
    }
    fn xyz(x: f64, y: f64, z: f64) -> Vec<u8> {
        let mut t = b"XYZ \0\0\0\0".to_vec();
        for v in [x, y, z] {
            t.extend_from_slice(&s15(v));
        }
        t
    }
    fn text_description(text: &str) -> Vec<u8> {
        let mut t = b"desc\0\0\0\0".to_vec();
        t.extend_from_slice(&((text.len() + 1) as u32).to_be_bytes());
        t.extend_from_slice(text.as_bytes());
        t.push(0);
        t.extend_from_slice(&[0; 8]); // no Unicode description
        t.extend_from_slice(&[0; 3]); // no ScriptCode description
        t.extend_from_slice(&[0; 67]);
        t
    }
    fn text(value: &str) -> Vec<u8> {
        let mut t = b"text\0\0\0\0".to_vec();
        t.extend_from_slice(value.as_bytes());
        t.push(0);
        t
    }

    // The sRGB transfer function, from encoded values to linear light.
    let mut curve = b"curv\0\0\0\0".to_vec();
    curve.extend_from_slice(&1024u32.to_be_bytes());
    for i in 0..1024u32 {
        let v = f64::from(i) / 1023.0;
        let linear = if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        };
        curve.extend_from_slice(&((linear * 65535.0).round() as u16).to_be_bytes());
    }

    // (signature, data); the three curves share one table. Sorted by signature.
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"bTRC", curve.clone()),
        (b"bXYZ", xyz(0.1430804, 0.0606169, 0.7141733)),
        (b"cprt", text("No copyright, use freely")),
        (b"desc", text_description("sRGB")),
        (b"gTRC", curve.clone()),
        (b"gXYZ", xyz(0.3850649, 0.7168786, 0.0971045)),
        (b"rTRC", curve),
        (b"rXYZ", xyz(0.4360747, 0.2225045, 0.0139322)),
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
    ];

    let table_len = 4 + 12 * tags.len();
    let mut table = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    table.extend_from_slice(&(tags.len() as u32).to_be_bytes());
    // The curve is stored once and the three curve tags point at it.
    let mut curve_at: Option<(usize, usize)> = None;
    for (signature, data) in &tags {
        let is_curve = signature.ends_with(b"TRC");
        let (at, len) = match (is_curve, curve_at) {
            (true, Some(known)) => known,
            _ => {
                let at = 128 + table_len + body.len();
                body.extend_from_slice(data);
                while body.len() & 3 != 0 {
                    body.push(0);
                }
                if is_curve {
                    curve_at = Some((at, data.len()));
                }
                (at, data.len())
            }
        };
        table.extend_from_slice(*signature);
        table.extend_from_slice(&(at as u32).to_be_bytes());
        table.extend_from_slice(&(len as u32).to_be_bytes());
    }

    let total = 128 + table.len() + body.len();
    let mut header = Vec::with_capacity(128);
    header.extend_from_slice(&(total as u32).to_be_bytes());
    header.extend_from_slice(&[0; 4]); // preferred CMM
    header.extend_from_slice(&[2, 0x10, 0, 0]); // version 2.1.0
    header.extend_from_slice(b"mntr");
    header.extend_from_slice(b"RGB ");
    header.extend_from_slice(b"XYZ ");
    for part in [2000u16, 1, 1, 0, 0, 0] {
        header.extend_from_slice(&part.to_be_bytes()); // 2000-01-01 00:00:00
    }
    header.extend_from_slice(b"acsp");
    header.extend_from_slice(&[0; 4]); // platform
    header.extend_from_slice(&[0; 4]); // flags
    header.extend_from_slice(&[0; 4]); // device manufacturer
    header.extend_from_slice(&[0; 4]); // device model
    header.extend_from_slice(&[0; 8]); // device attributes
    header.extend_from_slice(&[0; 4]); // rendering intent: perceptual
    for v in [0.9642, 1.0, 0.8249] {
        header.extend_from_slice(&s15(v)); // the D50 illuminant
    }
    header.extend_from_slice(&[0; 4]); // creator
    header.resize(128, 0); // profile id (none) and reserved bytes

    let mut out = header;
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    out
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
