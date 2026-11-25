//! printpdf writes PDF/A-1b, PDF/A-2b and PDF/X-1a:2001 files that its own preflight accepts.
//!
//! The preflight is the oracle here (see `printpdf::preflight`). It is not a certification, so this
//! file also checks the pieces of the output by hand: the XMP packet, the output intent, the
//! embedded built-in fonts and the text that `pdftotext` reads back.

use lopdf::{Dictionary, Document, Object};
use printpdf::preflight::{Finding, PreflightProfile, Severity};
use printpdf::*;

const FONT: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

fn text_at(font: PdfFontHandle, y: f32, text: &str) -> Vec<Op> {
    vec![
        Op::StartTextSection,
        Op::SetTextCursor {
            pos: Point::new(Mm(10.0), Mm(y)),
        },
        Op::SetFont {
            font,
            size: Pt(14.0),
        },
        Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        },
        Op::EndTextSection,
    ]
}

/// A 2 x 2 image: DeviceRGB, or DeviceGray when `rgb` is false.
fn image(rgb: bool) -> RawImage {
    if !rgb {
        return RawImage {
            pixels: RawImageData::U8(vec![0, 85, 170, 255]),
            width: 2,
            height: 2,
            data_format: RawImageFormat::R8,
            tag: Vec::new(),
        };
    }
    RawImage {
        pixels: RawImageData::U8(vec![
            255, 0, 0, 0, 255, 0, //
            0, 0, 255, 255, 255, 0,
        ]),
        width: 2,
        height: 2,
        data_format: RawImageFormat::RGB8,
        tag: Vec::new(),
    }
}

fn rect(x: f32, color: Color) -> Vec<Op> {
    vec![
        Op::SetFillColor { col: color },
        Op::DrawRectangle {
            rectangle: Rect {
                mode: Some(PaintMode::Fill),
                ..Rect::from_xywh(Pt(x), Pt(400.0), Pt(100.0), Pt(50.0))
            },
        },
    ]
}

/// Text in a built-in font and in an embedded font, a grey rectangle, an image and a link. With
/// `rgb`, the image is RGB and there is also an RGB rectangle; without it nothing uses RGB.
fn document(conformance: PdfConformance, rgb: bool) -> PdfDocument {
    let mut doc = PdfDocument::new("PDF/A test");
    doc.metadata.info.conformance = conformance;
    doc.metadata.info.author = "A. Author".to_string();
    doc.metadata.info.creator = "A creator".to_string();
    doc.metadata.info.subject = "A subject".to_string();
    doc.metadata.info.producer = "printpdf tests".to_string();
    doc.metadata.info.keywords = vec!["one".to_string(), "two".to_string()];
    let font = ParsedFont::from_bytes(FONT, 0, &mut Vec::new()).expect("font");
    let id = doc.add_font(&font);
    let image_id = doc.add_image(&image(rgb));

    let mut ops = Vec::new();
    ops.extend(text_at(
        PdfFontHandle::Builtin(BuiltinFont::Helvetica),
        280.0,
        "Builtin Helvetica text",
    ));
    ops.extend(text_at(
        PdfFontHandle::External(id),
        260.0,
        "Embedded Roboto text",
    ));
    if rgb {
        ops.extend(rect(50.0, Color::Rgb(Rgb::new(0.9, 0.2, 0.1, None))));
    }
    ops.extend(rect(
        200.0,
        Color::Greyscale(Greyscale::new(0.5, None)),
    ));
    ops.push(Op::UseXobject {
        id: image_id,
        transform: XObjectTransform {
            translate_x: Some(Pt(50.0)),
            translate_y: Some(Pt(500.0)),
            dpi: Some(10.0),
            ..Default::default()
        },
    });
    ops.push(Op::LinkAnnotation {
        link: LinkAnnotation::new(
            Rect::from_xywh(Pt(50.0), Pt(300.0), Pt(100.0), Pt(20.0)),
            Actions::uri("https://example.com".to_string()),
            None,
            None,
            None,
        ),
    });
    doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    doc
}

fn errors(findings: &[Finding]) -> Vec<(&'static str, &str)> {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| (f.rule, f.message.as_str()))
        .collect()
}

fn save(doc: &PdfDocument) -> (Vec<u8>, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    (bytes, warnings)
}

fn load(bytes: &[u8]) -> Document {
    Document::load_mem(bytes).expect("the file loads")
}

fn catalog(doc: &Document) -> Dictionary {
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .expect("root");
    doc.get_dictionary(root).expect("catalog").clone()
}

fn xmp(doc: &Document) -> String {
    let id = catalog(doc)
        .get(b"Metadata")
        .and_then(Object::as_reference)
        .expect("metadata");
    let stream = doc.get_object(id).and_then(Object::as_stream).expect("stream");
    let bytes = stream.decompressed_content().expect("xmp bytes");
    String::from_utf8(bytes).expect("xmp is UTF-8")
}

fn dictionaries(doc: &Document) -> Vec<&Dictionary> {
    doc.objects
        .values()
        .filter_map(|o| match o {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        })
        .collect()
}

fn name_of<'a>(dict: &'a Dictionary, key: &[u8]) -> Option<&'a [u8]> {
    dict.get(key).ok().and_then(|o| o.as_name().ok())
}

/// The font dictionaries with this `/BaseFont` (ignoring a subset prefix).
fn fonts_named<'a>(doc: &'a Document, base: &str) -> Vec<&'a Dictionary> {
    dictionaries(doc)
        .into_iter()
        .filter(|d| name_of(d, b"Type") == Some(b"Font"))
        .filter(|d| {
            name_of(d, b"BaseFont")
                .map(|n| String::from_utf8_lossy(n).ends_with(base))
                .unwrap_or(false)
        })
        .collect()
}

// --- the preflight accepts the files -------------------------------------------------------

#[test]
fn a1b_document_has_no_preflight_errors() {
    let doc = document(PdfConformance::A1B_2005_PDF_1_4, true);
    let report = doc.preflight(PreflightProfile::PdfA1b);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

#[test]
fn a2b_document_has_no_preflight_errors() {
    let doc = document(PdfConformance::A2B_2011_PDF_1_7, true);
    let report = doc.preflight(PreflightProfile::PdfA2b);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

#[test]
fn x1a_2001_document_has_no_preflight_errors() {
    // No RGB: PDF/X-1a allows only gray, CMYK and spot colours.
    let doc = document(PdfConformance::X1A_2001_PDF_1_3, false);
    let report = doc.preflight(PreflightProfile::PdfX1a_2001);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

#[test]
fn x3_2002_document_has_no_preflight_errors_with_a_builtin_font() {
    let doc = document(PdfConformance::X3_2002_PDF_1_3, true);
    let report = doc.preflight(PreflightProfile::PdfX3_2002);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

#[test]
fn every_pdfa_variant_passes_the_profile_of_its_part() {
    for (conformance, profile) in [
        (PdfConformance::A1B_2005_PDF_1_4, PreflightProfile::PdfA1b),
        (PdfConformance::A1A_2005_PDF_1_4, PreflightProfile::PdfA1b),
        (PdfConformance::A2_2011_PDF_1_7, PreflightProfile::PdfA2b),
        (PdfConformance::A2A_2011_PDF_1_7, PreflightProfile::PdfA2b),
        (PdfConformance::A2B_2011_PDF_1_7, PreflightProfile::PdfA2b),
        (PdfConformance::A2U_2011_PDF_1_7, PreflightProfile::PdfA2b),
        (PdfConformance::A3_2012_PDF_1_7, PreflightProfile::PdfA2b),
    ] {
        let report = document(conformance.clone(), true).preflight(profile);
        // A-3 names part 3, which the A-2 profile reports as a mismatch of pdfaid:part only.
        let found: Vec<_> = errors(&report.findings)
            .into_iter()
            .filter(|(rule, _)| {
                !(conformance == PdfConformance::A3_2012_PDF_1_7 && *rule == "pdfa-id-mismatch")
            })
            .collect();
        assert_eq!(found, Vec::new(), "{conformance:?}: {:#?}", report.findings);
    }
}

// --- identification, metadata, output intent -----------------------------------------------

#[test]
fn xmp_identifies_the_part_and_level() {
    for (conformance, part, level) in [
        (PdfConformance::A1B_2005_PDF_1_4, "1", "B"),
        (PdfConformance::A2B_2011_PDF_1_7, "2", "B"),
        (PdfConformance::A2_2011_PDF_1_7, "2", "B"),
        (PdfConformance::A3_2012_PDF_1_7, "3", "B"),
    ] {
        let (bytes, _) = save(&document(conformance.clone(), true));
        let text = xmp(&load(&bytes));
        assert!(text.contains("http://www.aiim.org/pdfa/ns/id/"), "{text}");
        assert!(text.contains(&format!("<pdfaid:part>{part}</pdfaid:part>")), "{conformance:?}");
        assert!(
            text.contains(&format!("<pdfaid:conformance>{level}</pdfaid:conformance>")),
            "{conformance:?}"
        );
        assert!(!text.contains("pdfxid") && !text.contains("GTS_PDFXVersion"), "{text}");
    }
}

#[test]
fn the_a_variants_warn_and_claim_level_b() {
    for (conformance, part, word) in [
        (PdfConformance::A1A_2005_PDF_1_4, "1", "level a"),
        (PdfConformance::A2A_2011_PDF_1_7, "2", "level a"),
        (PdfConformance::A2U_2011_PDF_1_7, "2", "level u"),
    ] {
        let (bytes, warnings) = save(&document(conformance.clone(), true));
        assert!(
            warnings
                .iter()
                .any(|w| w.msg.contains(word) && w.msg.contains("claims level b")),
            "{conformance:?}: {warnings:#?}"
        );
        let text = xmp(&load(&bytes));
        assert!(text.contains("<pdfaid:conformance>B</pdfaid:conformance>"), "{text}");
        assert!(text.contains(&format!("<pdfaid:part>{part}</pdfaid:part>")), "{text}");
        assert!(!text.contains("<pdfaid:conformance>A<"), "{text}");
    }
    // The b variants make no such claim, so they do not warn about a level.
    for conformance in [PdfConformance::A1B_2005_PDF_1_4, PdfConformance::A2B_2011_PDF_1_7] {
        let (_, warnings) = save(&document(conformance.clone(), true));
        assert!(
            warnings.iter().all(|w| !w.msg.contains("tagged")),
            "{conformance:?}: {warnings:#?}"
        );
    }
}

#[test]
fn xmp_is_well_formed_with_a_hostile_title() {
    let mut doc = document(PdfConformance::A1B_2005_PDF_1_4, true);
    doc.metadata.info.document_title = "<&\">".to_string();
    doc.metadata.info.author = "A & B <c> \"d\" 'e'".to_string();
    doc.metadata.info.keywords = vec!["a&b".to_string(), "<x>".to_string()];
    doc.metadata.info.subject = "]]><!-- x -->".to_string();
    let (bytes, _) = save(&doc);
    let text = xmp(&load(&bytes));
    assert!(text.contains("&lt;&amp;&quot;&gt;"), "{text}");
    assert!(text.contains("A &amp; B &lt;c&gt;"), "{text}");
    assert!(!text.contains("<&\">"), "{text}");
    let report = preflight_bytes_a1b(&bytes);
    assert!(
        report.iter().all(|f| f.rule != "xmp-not-well-formed"),
        "{report:#?}"
    );
    assert_eq!(errors(&report), Vec::new(), "{report:#?}");
}

fn preflight_bytes_a1b(bytes: &[u8]) -> Vec<Finding> {
    printpdf::preflight::preflight_bytes(bytes, PreflightProfile::PdfA1b).findings
}

#[test]
fn xmp_repeats_the_values_of_the_info_dictionary() {
    let mut doc = document(PdfConformance::A2B_2011_PDF_1_7, true);
    doc.metadata.info.document_title = "Annual report".to_string();
    let (bytes, _) = save(&doc);
    let loaded = load(&bytes);
    let text = xmp(&loaded);
    assert!(text.contains(">Annual report</rdf:li>"), "{text}");
    assert!(text.contains("<rdf:li>A. Author</rdf:li>"), "{text}");
    assert!(text.contains("<xmp:CreatorTool>A creator</xmp:CreatorTool>"), "{text}");
    assert!(text.contains("<pdf:Producer>printpdf tests</pdf:Producer>"), "{text}");
    assert!(text.contains("<pdf:Keywords>one,two</pdf:Keywords>"), "{text}");
    assert!(text.contains(">A subject</rdf:li>"), "{text}");
    // The dates are ISO 8601, not PDF dates.
    assert!(text.contains("<xmp:CreateDate>1970-01-01T00:00:00+00:00</xmp:CreateDate>"), "{text}");
    assert!(!text.contains("D:1970"), "{text}");

    let info_id = loaded
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .expect("info");
    let info = loaded.get_dictionary(info_id).expect("info dictionary");
    assert!(info.get(b"Title").is_ok() && info.get(b"Author").is_ok());
    assert!(
        info.get(b"GTS_PDFXVersion").is_err(),
        "a PDF/A file has no /GTS_PDFXVersion"
    );
}

#[test]
fn the_output_intent_of_pdfa_is_srgb() {
    for conformance in [PdfConformance::A1B_2005_PDF_1_4, PdfConformance::A2B_2011_PDF_1_7] {
        let (bytes, warnings) = save(&document(conformance.clone(), true));
        let loaded = load(&bytes);
        let cat = catalog(&loaded);
        let intents = cat.get(b"OutputIntents").and_then(Object::as_array).expect("intents");
        assert_eq!(intents.len(), 1);
        let intent = intents[0].as_dict().expect("intent");
        assert_eq!(name_of(intent, b"S"), Some(&b"GTS_PDFA1"[..]));
        assert!(intent.get(b"OutputConditionIdentifier").is_ok());
        assert!(intent.get(b"Info").is_ok());
        let profile = intent
            .get(b"DestOutputProfile")
            .and_then(Object::as_reference)
            .expect("profile");
        let stream = loaded.get_object(profile).and_then(Object::as_stream).expect("stream");
        assert_eq!(stream.dict.get(b"N").and_then(Object::as_i64).ok(), Some(3));
        let data = stream.decompressed_content().expect("profile bytes");
        assert_eq!(data.get(36..40), Some(&b"acsp"[..]));
        assert_eq!(data.get(16..20), Some(&b"RGB "[..]));
        assert!(
            warnings.iter().all(|w| !w.msg.contains("DeviceCMYK")),
            "{warnings:#?}"
        );
    }
}

#[test]
fn a_cmyk_only_pdfa_document_gets_the_cmyk_profile() {
    let mut doc = document(PdfConformance::A1B_2005_PDF_1_4, false);
    if let Some(page) = doc.pages.first_mut() {
        page.ops.extend(rect(
            300.0,
            Color::Cmyk(Cmyk::new(0.1, 0.2, 0.3, 0.4, None)),
        ));
    }
    let report = doc.preflight(PreflightProfile::PdfA1b);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);

    let (bytes, _) = save(&doc);
    let loaded = load(&bytes);
    let cat = catalog(&loaded);
    let intent = cat
        .get(b"OutputIntents")
        .and_then(Object::as_array)
        .expect("intents")[0]
        .as_dict()
        .expect("intent")
        .clone();
    assert_eq!(name_of(&intent, b"S"), Some(&b"GTS_PDFA1"[..]));
    let profile = intent
        .get(b"DestOutputProfile")
        .and_then(Object::as_reference)
        .expect("profile");
    let stream = loaded.get_object(profile).and_then(Object::as_stream).expect("stream");
    assert_eq!(stream.dict.get(b"N").and_then(Object::as_i64).ok(), Some(4));
}

#[test]
fn rgb_and_cmyk_together_in_pdfa_warn() {
    let mut doc = document(PdfConformance::A2B_2011_PDF_1_7, true);
    if let Some(page) = doc.pages.first_mut() {
        page.ops.extend(rect(
            300.0,
            Color::Cmyk(Cmyk::new(0.1, 0.2, 0.3, 0.4, None)),
        ));
    }
    let (_, warnings) = save(&doc);
    assert!(
        warnings.iter().any(|w| w.msg.contains("DeviceCMYK")),
        "{warnings:#?}"
    );
}

#[test]
fn x1a_2001_writes_an_output_intent_and_warns_about_rgb() {
    let (bytes, warnings) = save(&document(PdfConformance::X1A_2001_PDF_1_3, false));
    let loaded = load(&bytes);
    let cat = catalog(&loaded);
    let intent = cat
        .get(b"OutputIntents")
        .and_then(Object::as_array)
        .expect("X-1a:2001 has an output intent")[0]
        .as_dict()
        .expect("intent")
        .clone();
    assert_eq!(name_of(&intent, b"S"), Some(&b"GTS_PDFX"[..]));
    assert!(intent.get(b"DestOutputProfile").is_ok());
    assert!(warnings.iter().all(|w| !w.msg.contains("DeviceRGB")), "{warnings:#?}");

    let (_, warnings) = save(&document(PdfConformance::X1A_2001_PDF_1_3, true));
    assert!(warnings.iter().any(|w| w.msg.contains("DeviceRGB")), "{warnings:#?}");
}

#[test]
fn header_version_trailer_id_and_binary_comment() {
    for (conformance, version) in [
        (PdfConformance::A1B_2005_PDF_1_4, "1.4"),
        (PdfConformance::A2B_2011_PDF_1_7, "1.7"),
        (PdfConformance::A3_2012_PDF_1_7, "1.7"),
        (PdfConformance::X1A_2001_PDF_1_3, "1.3"),
    ] {
        let (bytes, _) = save(&document(conformance.clone(), false));
        assert!(
            bytes.starts_with(format!("%PDF-{version}\n").as_bytes()),
            "{conformance:?}"
        );
        // The binary comment: a line of four bytes above 127 after the header.
        let second_line = bytes
            .split(|b| *b == b'\n')
            .nth(1)
            .expect("second line");
        assert!(
            second_line.starts_with(b"%") && second_line.iter().skip(1).filter(|b| **b > 127).count() >= 4,
            "{conformance:?}"
        );
        let loaded = load(&bytes);
        let id = loaded.trailer.get(b"ID").and_then(Object::as_array).expect("ID");
        assert_eq!(id.len(), 2);
    }
}

#[test]
fn a_saved_pdfa_file_reads_back_as_pdfa() {
    for conformance in [
        PdfConformance::A1B_2005_PDF_1_4,
        PdfConformance::A2B_2011_PDF_1_7,
        PdfConformance::A3_2012_PDF_1_7,
    ] {
        let (bytes, _) = save(&document(conformance.clone(), true));
        let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new())
            .expect("parse");
        assert_eq!(parsed.metadata.info.conformance, conformance);
        // The embedded built-in font reads back as a font with the same text.
        let text = parsed.extract_text().concat().concat();
        assert!(text.contains("Builtin Helvetica text"), "{text:?}");
    }
}

// --- built-in fonts --------------------------------------------------------------------------

fn builtin_document(conformance: PdfConformance, font: BuiltinFont, text: &str) -> PdfDocument {
    let mut doc = PdfDocument::new("Built-in fonts");
    doc.metadata.info.conformance = conformance;
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        text_at(PdfFontHandle::Builtin(font), 280.0, text),
    ));
    doc
}

#[test]
fn builtin_fonts_are_embedded_for_the_standards_that_need_it() {
    let text = "Hello Wörld";
    for (conformance, embedded) in [
        (PdfConformance::A1B_2005_PDF_1_4, true),
        (PdfConformance::A2B_2011_PDF_1_7, true),
        (PdfConformance::X1A_2001_PDF_1_3, true),
        (PdfConformance::X3_2002_PDF_1_3, true),
        (PdfConformance::default(), false),
    ] {
        let (bytes, _) = save(&builtin_document(
            conformance.clone(),
            BuiltinFont::TimesRoman,
            text,
        ));
        let loaded = load(&bytes);
        let fonts = fonts_named(&loaded, "Times-Roman");
        assert_eq!(fonts.len(), 1, "{conformance:?}");
        let font = fonts[0];
        if embedded {
            assert_eq!(name_of(font, b"Subtype"), Some(&b"TrueType"[..]), "{conformance:?}");
            assert_eq!(name_of(font, b"Encoding"), Some(&b"WinAnsiEncoding"[..]));
            let descriptor = font
                .get(b"FontDescriptor")
                .and_then(Object::as_reference)
                .and_then(|id| loaded.get_dictionary(id))
                .expect("descriptor");
            let program = descriptor
                .get(b"FontFile2")
                .and_then(Object::as_reference)
                .and_then(|id| loaded.get_object(id))
                .and_then(Object::as_stream)
                .expect("font program");
            let bytes = program.decompressed_content().expect("program bytes");
            assert_eq!(bytes.get(0..4), Some(&[0, 1, 0, 0][..]), "a TrueType program");
            // Only the glyphs of the text are kept when fonts can be subset (the bundled file
            // has more than 200 glyphs); without the `text_layout` feature the whole file is
            // embedded.
            if cfg!(feature = "text_layout") {
                assert!(bytes.len() < 30_000, "{} bytes", bytes.len());
            }
            // The widths cover the codes of the text.
            let first = font.get(b"FirstChar").and_then(Object::as_i64).expect("first");
            let last = font.get(b"LastChar").and_then(Object::as_i64).expect("last");
            let widths = font.get(b"Widths").and_then(Object::as_array).expect("widths");
            assert_eq!(widths.len() as i64, last - first + 1);
            assert_eq!(first, i64::from(b' '));
            assert_eq!(last, 0xF6); // ö
        } else {
            assert_eq!(name_of(font, b"Subtype"), Some(&b"Type1"[..]), "{conformance:?}");
            assert!(font.get(b"FontDescriptor").is_err());
        }
    }
}

#[test]
fn all_twelve_text_fonts_embed_and_pass_the_preflight() {
    use BuiltinFont::*;
    for font in [
        TimesRoman,
        TimesBold,
        TimesItalic,
        TimesBoldItalic,
        Helvetica,
        HelveticaBold,
        HelveticaOblique,
        HelveticaBoldOblique,
        Courier,
        CourierOblique,
        CourierBold,
        CourierBoldOblique,
    ] {
        let doc = builtin_document(PdfConformance::A2B_2011_PDF_1_7, font, "Quick brown fox 0123");
        let (_, warnings) = save(&doc);
        assert!(warnings.is_empty(), "{font:?}: {warnings:#?}");
        let report = doc.preflight(PreflightProfile::PdfA2b);
        assert_eq!(errors(&report.findings), Vec::new(), "{font:?}: {:#?}", report.findings);
    }
}

#[test]
fn symbol_and_zapfdingbats_stay_unembedded_and_warn() {
    for font in [BuiltinFont::Symbol, BuiltinFont::ZapfDingbats] {
        let doc = builtin_document(PdfConformance::A1B_2005_PDF_1_4, font, "abc");
        let (_, warnings) = save(&doc);
        assert!(
            warnings.iter().any(|w| w.msg.contains(font.get_id()) && w.msg.contains("not embedded")),
            "{font:?}: {warnings:#?}"
        );
        let report = doc.preflight(PreflightProfile::PdfA1b);
        assert!(
            errors(&report.findings).iter().any(|(rule, _)| *rule == "font-not-embedded"),
            "{font:?}"
        );
    }
}

#[test]
fn a_character_the_bundled_font_lacks_is_named_in_a_warning() {
    // The bundled Courier-BoldOblique has no euro sign.
    let doc = builtin_document(
        PdfConformance::A1B_2005_PDF_1_4,
        BuiltinFont::CourierBoldOblique,
        "5 \u{20AC}",
    );
    let (_, warnings) = save(&doc);
    assert!(
        warnings
            .iter()
            .any(|w| w.msg.contains("Courier-BoldOblique") && w.msg.contains("U+20AC")),
        "{warnings:#?}"
    );
}

#[test]
fn a_font_used_in_a_form_is_embedded() {
    let mut doc = PdfDocument::new("Form");
    doc.metadata.info.conformance = PdfConformance::A2B_2011_PDF_1_7;
    let form = FormXObject::new(
        Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(200.0), Pt(50.0)),
        text_at(PdfFontHandle::Builtin(BuiltinFont::Courier), 10.0, "in a form"),
    )
    .with_resources(PdfResources::default());
    let id = doc.add_form(&form);
    doc.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![Op::UseXobject {
            id,
            transform: XObjectTransform::default(),
        }],
    ));
    let report = doc.preflight(PreflightProfile::PdfA2b);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

/// `pdftotext` output of `bytes`, or `None` when the tool is not installed.
fn pdftotext(bytes: &[u8], name: &str) -> Option<String> {
    let path = std::env::temp_dir().join(format!("printpdf-pdfa-output-{}-{name}.pdf", std::process::id()));
    std::fs::write(&path, bytes).ok()?;
    let output = std::process::Command::new("pdftotext")
        .arg(&path)
        .arg("-")
        .output()
        .ok();
    let _ = std::fs::remove_file(&path);
    let output = output?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

#[test]
fn pdftotext_reads_the_same_text_before_and_after_embedding() {
    let text = "Gr\u{FC}\u{DF}e \u{2013} \u{201C}quoted\u{201D} caf\u{E9} \u{20AC}5 and \u{2026}";
    for font in [BuiltinFont::Helvetica, BuiltinFont::TimesItalic, BuiltinFont::Courier] {
        let plain = save(&builtin_document(PdfConformance::default(), font, text)).0;
        let embedded = save(&builtin_document(PdfConformance::A2B_2011_PDF_1_7, font, text)).0;
        let (Some(a), Some(b)) = (pdftotext(&plain, "plain"), pdftotext(&embedded, "embedded")) else {
            eprintln!("pdftotext is not installed or failed; skipping the text comparison");
            return;
        };
        assert!(a.contains("quoted"), "{font:?}: {a:?}");
        assert_eq!(a, b, "{font:?}");
        assert!(b.contains("caf\u{E9}") && b.contains("\u{20AC}5"), "{font:?}: {b:?}");
    }
}

// --- warnings about content that a standard does not allow -------------------------------------

#[test]
fn transparency_warns_in_pdfa1_and_not_in_pdfa2() {
    let build = |conformance: PdfConformance| {
        let mut doc = document(conformance, true);
        let gs = doc.add_graphics_state(ExtendedGraphicsState::default().with_current_fill_alpha(0.5));
        if let Some(page) = doc.pages.first_mut() {
            page.ops.insert(0, Op::LoadGraphicsState { gs });
        }
        doc
    };
    let (_, warnings) = save(&build(PdfConformance::A1B_2005_PDF_1_4));
    assert!(
        warnings.iter().any(|w| w.msg.contains("transparency")),
        "{warnings:#?}"
    );
    let report = build(PdfConformance::A1B_2005_PDF_1_4).preflight(PreflightProfile::PdfA1b);
    assert!(report.findings.iter().any(|f| f.rule == "transparency-alpha"));

    let (_, warnings) = save(&build(PdfConformance::A2B_2011_PDF_1_7));
    assert!(
        warnings.iter().all(|w| !w.msg.contains("transparency")),
        "{warnings:#?}"
    );
    let report = build(PdfConformance::A2B_2011_PDF_1_7).preflight(PreflightProfile::PdfA2b);
    assert_eq!(errors(&report.findings), Vec::new(), "{:#?}", report.findings);
}

#[test]
fn an_rgba_image_is_transparency_in_pdfa1() {
    let mut doc = document(PdfConformance::A1B_2005_PDF_1_4, true);
    let rgba = RawImage {
        pixels: RawImageData::U8(vec![255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 0]),
        width: 2,
        height: 2,
        data_format: RawImageFormat::RGBA8,
        tag: Vec::new(),
    };
    let id = doc.add_image(&rgba);
    if let Some(page) = doc.pages.first_mut() {
        page.ops.push(Op::UseXobject {
            id,
            transform: XObjectTransform::default(),
        });
    }
    let (_, warnings) = save(&doc);
    assert!(
        warnings.iter().any(|w| w.msg.contains("soft mask")),
        "{warnings:#?}"
    );
}

#[test]
fn layers_warn_in_pdfa1() {
    let mut doc = document(PdfConformance::A1B_2005_PDF_1_4, true);
    let layer = doc.add_layer(&Layer {
        name: "Notes".to_string(),
        creator: "tests".to_string(),
        intent: LayerIntent::View,
        usage: LayerSubtype::Artwork,
    });
    if let Some(page) = doc.pages.first_mut() {
        page.ops.insert(0, Op::BeginLayer { layer_id: layer });
        page.ops.push(Op::EndLayer);
    }
    let (_, warnings) = save(&doc);
    assert!(
        warnings.iter().any(|w| w.msg.contains("layers")),
        "{warnings:#?}"
    );
    let report = doc.preflight(PreflightProfile::PdfA1b);
    assert!(report.findings.iter().any(|f| f.rule == "optional-content"));
}

#[test]
fn the_default_conformance_is_unchanged() {
    // No XMP, no output intent, Type1 fonts, version 1.3, and no warning from the standards.
    let (bytes, warnings) = save(&document(PdfConformance::default(), true));
    assert!(bytes.starts_with(b"%PDF-1.3\n"));
    let loaded = load(&bytes);
    let cat = catalog(&loaded);
    assert!(cat.get(b"Metadata").is_err());
    assert!(cat.get(b"OutputIntents").is_err());
    assert!(warnings.is_empty(), "{warnings:#?}");
}

// --- other tools -------------------------------------------------------------------------------

#[test]
fn qpdf_accepts_the_files() {
    let Ok(version) = std::process::Command::new("qpdf").arg("--version").output() else {
        eprintln!("qpdf is not installed; skipping");
        return;
    };
    assert!(version.status.success());
    for (name, doc) in [
        ("a1b", document(PdfConformance::A1B_2005_PDF_1_4, true)),
        ("a2b", document(PdfConformance::A2B_2011_PDF_1_7, true)),
        ("x1a", document(PdfConformance::X1A_2001_PDF_1_3, false)),
    ] {
        let (bytes, _) = save(&doc);
        let path = std::env::temp_dir().join(format!("printpdf-pdfa-qpdf-{}-{name}.pdf", std::process::id()));
        std::fs::write(&path, &bytes).expect("write");
        let output = std::process::Command::new("qpdf")
            .arg("--check")
            .arg(&path)
            .output()
            .expect("run qpdf");
        let _ = std::fs::remove_file(&path);
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// veraPDF is the reference validator for PDF/A. The test runs it when `verapdf` is on the PATH
/// (`brew install verapdf`) and skips otherwise. The set `PRINTPDF_VERAPDF=1` makes a missing
/// validator a failure, for CI jobs that install it.
fn verapdf_report(flavour: &str, bytes: &[u8], name: &str) -> Option<(bool, String)> {
    let path = std::env::temp_dir().join(format!("printpdf-verapdf-{}-{name}.pdf", std::process::id()));
    std::fs::write(&path, bytes).expect("write the temporary PDF");
    let output = std::process::Command::new("verapdf")
        .args(["--flavour", flavour, "--format", "text"])
        .arg(&path)
        .output();
    // PRINTPDF_VERAPDF_KEEP=<dir> keeps a copy of each file, to run veraPDF by hand.
    if let Some(dir) = std::env::var_os("PRINTPDF_VERAPDF_KEEP") {
        let _ = std::fs::copy(&path, std::path::Path::new(&dir).join(format!("{name}.pdf")));
    }
    let _ = std::fs::remove_file(&path);
    match output {
        Ok(o) => Some((
            o.status.success(),
            format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
        )),
        Err(_) => {
            assert!(
                std::env::var_os("PRINTPDF_VERAPDF").is_none(),
                "PRINTPDF_VERAPDF is set but verapdf cannot be run"
            );
            eprintln!("verapdf is not installed; skipping");
            None
        }
    }
}

#[test]
fn verapdf_accepts_a2b() {
    let (bytes, _) = save(&document(PdfConformance::A2B_2011_PDF_1_7, true));
    if let Some((ok, text)) = verapdf_report("2b", &bytes, "a2b") {
        assert!(ok, "veraPDF rejects the PDF/A-2b file:\n{text}");
    }
}

#[test]
fn verapdf_accepts_a1b() {
    let (bytes, _) = save(&document(PdfConformance::A1B_2005_PDF_1_4, true));
    if let Some((ok, text)) = verapdf_report("1b", &bytes, "a1b") {
        assert!(ok, "veraPDF rejects the PDF/A-1b file:\n{text}");
    }
}
