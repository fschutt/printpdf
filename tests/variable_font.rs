//! Variable fonts: the axes of a font, static instances created from it, and their embedding in a PDF.
//!
//! PDF has no variable fonts: a viewer reads an embedded font program as its default instance.
//! printpdf therefore creates a static font at chosen axis values before it embeds anything. These
//! tests look at the bytes of the instances and at the font programs inside the saved PDF with
//! allsorts and lopdf, so that printpdf's own reader does not check printpdf's own writer.
//!
//! Fixtures (`tests/assets/variable-fonts`, both under the SIL Open Font License):
//!
//! - `RedHatMono-VariableFont_wght.ttf`: TrueType outlines (`glyf` + `gvar`), `wght` 300..700.
//! - `Cantarell-VF.otf`: CFF2 outlines, `wght` 100..800. A PDF cannot hold CFF2, so this one
//!   covers the conversion to CFF1.
//!
//! The poppler checks (`pdffonts`, `pdftotext`) are skipped when poppler is not installed.

#![cfg(feature = "text_layout")]

use std::process::Command;

use allsorts::{binary::read::ReadScope, font_data::FontData, tables::FontTableProvider, tag};
use lopdf::{Document, Object};
use printpdf::{
    font_variation_axes, instantiate_variable_font_bytes, FontId, FontVariationSettings,
    FontVariationTag, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfParseErrorSeverity, PdfParseOptions, PdfSaveOptions, PdfWarnMsg, Point, Pt, TextItem,
    VariableFontError,
};

const RED_HAT_MONO: &[u8] = include_bytes!("assets/variable-fonts/RedHatMono-VariableFont_wght.ttf");
const CANTARELL: &[u8] = include_bytes!("assets/variable-fonts/Cantarell-VF.otf");
const ROBOTO: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// Text that two instances of a font both show and that both must give back on extraction.
const SAMPLE: &str = "Hamburgefonstiv 0123";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn wght(value: f32) -> FontVariationSettings {
    FontVariationSettings::new().with(FontVariationTag::WGHT, value)
}

/// The tags of the tables of the first face of an sfnt, as four-character strings.
fn tables_of(bytes: &[u8]) -> Vec<String> {
    let font_file = ReadScope::new(bytes)
        .read::<FontData<'_>>()
        .expect("font file reads");
    let provider = font_file.table_provider(0).expect("first face");
    provider
        .table_tags()
        .expect("table tags")
        .into_iter()
        .map(|t| String::from_utf8_lossy(&t.to_be_bytes()).into_owned())
        .collect()
}

fn has_table(bytes: &[u8], table: &str) -> bool {
    tables_of(bytes).iter().any(|t| t == table)
}

/// The variation tables (and `CFF2`, which only a variable font needs) that `bytes` still has.
fn variation_tables_in(bytes: &[u8]) -> Vec<String> {
    const VARIATION_TABLES: [&str; 8] = ["fvar", "gvar", "avar", "cvar", "HVAR", "MVAR", "VVAR", "CFF2"];
    tables_of(bytes)
        .into_iter()
        .filter(|t| VARIATION_TABLES.contains(&t.as_str()))
        .collect()
}

/// One text section with a line for each `(font, text)`, 20 mm apart.
fn one_page(lines: &[(&FontId, &str)]) -> PdfPage {
    let mut ops = vec![Op::StartTextSection];
    for (n, (font, text)) in lines.iter().enumerate() {
        // `Td` moves from the start of the previous line; the first move is the page position.
        let pos = if n == 0 {
            Point::new(Mm(20.0), Mm(260.0))
        } else {
            Point::new(Mm(0.0), Mm(-20.0))
        };
        ops.push(Op::SetTextCursor { pos });
        ops.push(Op::SetFont {
            font: PdfFontHandle::External((*font).clone()),
            size: Pt(24.0),
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text((*text).to_string())],
        });
    }
    ops.push(Op::EndTextSection);
    PdfPage::new(Mm(210.0), Mm(297.0), ops)
}

fn save(doc: &PdfDocument, subset_fonts: bool) -> (Vec<u8>, Vec<PdfWarnMsg>) {
    let mut warnings = Vec::new();
    let opts = PdfSaveOptions {
        subset_fonts,
        ..Default::default()
    };
    let pdf = doc.save(&opts, &mut warnings);
    (pdf, warnings)
}

fn assert_no_errors(warnings: &[PdfWarnMsg], what: &str) {
    let errors: Vec<&str> = warnings
        .iter()
        .filter(|w| w.severity == PdfParseErrorSeverity::Error)
        .map(|w| w.msg.as_str())
        .collect();
    assert!(errors.is_empty(), "{what}: save reported errors: {errors:#?}");
}

/// A font program as it sits in a saved PDF.
struct EmbeddedProgram {
    base_font: String,
    /// `FontFile2` (TrueType) or `FontFile3` (CFF, bare or in an OpenType wrapper).
    file_key: String,
    program: Vec<u8>,
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).expect("reference resolves"),
        other => other,
    }
}

/// The font programs of a saved PDF, found through lopdf: every Type0 font, its descendant, its
/// descriptor and the decoded `/FontFile*` stream.
fn embedded_programs(pdf: &[u8]) -> Vec<EmbeddedProgram> {
    let doc = Document::load_mem(pdf).expect("saved PDF loads in lopdf");
    let mut out = Vec::new();
    for object in doc.objects.values() {
        let Ok(font) = object.as_dict() else { continue };
        if font.get(b"Subtype").ok().and_then(|s| s.as_name().ok()) != Some(b"Type0".as_slice()) {
            continue;
        }
        let base_font = font
            .get(b"BaseFont")
            .ok()
            .and_then(|n| n.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .expect("a Type0 font has a /BaseFont");
        let descendants = resolve(&doc, font.get(b"DescendantFonts").expect("/DescendantFonts"))
            .as_array()
            .expect("an array");
        let descendant = resolve(&doc, descendants.first().expect("one descendant"))
            .as_dict()
            .expect("a descendant dictionary");
        let descriptor = resolve(&doc, descendant.get(b"FontDescriptor").expect("/FontDescriptor"))
            .as_dict()
            .expect("a descriptor dictionary");
        for key in ["FontFile2", "FontFile3"] {
            if let Ok(file) = descriptor.get(key.as_bytes()) {
                let stream = resolve(&doc, file).as_stream().expect("a font file stream");
                let program = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                out.push(EmbeddedProgram {
                    base_font: base_font.clone(),
                    file_key: key.to_string(),
                    program,
                });
            }
        }
    }
    out
}

/// What printpdf's reader extracts from the pages of `pdf`, all text sections joined.
fn extracted_text(pdf: &[u8]) -> String {
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(pdf, &PdfParseOptions::default(), &mut warnings)
        .expect("the saved PDF parses");
    doc.extract_text()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs a poppler tool on `pdf` (`args` come after the file name) and returns its stdout and
/// stderr, or `None` when poppler is not installed. The version banner is checked because some CI
/// images have an unrelated program of the same name.
fn poppler(tool: &str, args: &[&str], pdf: &[u8], name: &str) -> Option<(String, String)> {
    let banner = Command::new(tool).arg("-v").output().ok()?;
    let banner = format!(
        "{}{}",
        String::from_utf8_lossy(&banner.stdout),
        String::from_utf8_lossy(&banner.stderr)
    )
    .to_lowercase();
    if !banner.contains("poppler") {
        return None;
    }
    let path = std::env::temp_dir().join(format!(
        "printpdf-variable-font-{}-{name}.pdf",
        std::process::id()
    ));
    std::fs::write(&path, pdf).expect("write the PDF for poppler");
    let out = Command::new(tool).arg(&path).args(args).output();
    let _ = std::fs::remove_file(&path);
    let out = out.ok()?;
    Some((
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// Poppler reads the PDF without a syntax error and lists `lines` fonts, all embedded, and
/// `pdftotext` gives back `SAMPLE` once for each of the `lines` lines. Skipped unless both
/// `pdffonts` and `pdftotext` are poppler's.
fn assert_poppler_accepts(pdf: &[u8], name: &str, lines: usize) {
    let (Some((fonts, fonts_errors)), Some((text, _))) = (
        poppler("pdffonts", &[], pdf, name),
        poppler("pdftotext", &["-"], pdf, name),
    ) else {
        eprintln!("{name}: poppler is not installed, skipping the poppler checks");
        return;
    };
    assert!(
        !fonts_errors.contains("Syntax Error") && !fonts_errors.contains("may be invalid"),
        "{name}: pdffonts rejected the PDF:\n{fonts_errors}\n{fonts}"
    );
    // Columns: name type encoding emb sub uni object generation. Count from the right, because
    // the type has spaces ("CID TrueType").
    let rows: Vec<Vec<&str>> = fonts
        .lines()
        .skip_while(|l| !l.starts_with("---"))
        .skip(1)
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|cols| !cols.is_empty())
        .collect();
    assert_eq!(rows.len(), lines, "{name}: pdffonts lists one font per line:\n{fonts}");
    for cols in &rows {
        let embedded = cols.len().checked_sub(5).and_then(|i| cols.get(i));
        assert_eq!(embedded, Some(&"yes"), "{name}: a font is not embedded:\n{fonts}");
    }

    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(text.matches(SAMPLE).count(), lines, "{name}: pdftotext gave {text:?}");
}

// ---------------------------------------------------------------------------
// Axes and tags
// ---------------------------------------------------------------------------

#[test]
fn lists_the_axes_with_names_and_ranges() {
    for (bytes, min, max) in [(RED_HAT_MONO, 300.0, 700.0), (CANTARELL, 100.0, 800.0)] {
        let axes = font_variation_axes(bytes, 0).expect("axes");
        assert_eq!(axes.len(), 1);
        let axis = &axes[0];
        assert_eq!(axis.tag, FontVariationTag::WGHT);
        assert_eq!(axis.name.as_deref(), Some("Weight"));
        assert_eq!((axis.min, axis.default, axis.max), (min, 400.0, max));
    }
}

#[test]
fn a_static_font_has_no_axes_and_cannot_be_instantiated() {
    assert_eq!(font_variation_axes(ROBOTO, 0), Ok(Vec::new()));
    assert_eq!(
        instantiate_variable_font_bytes(ROBOTO, 0, &FontVariationSettings::new()),
        Err(VariableFontError::NotVariable)
    );
    let mut doc = PdfDocument::new("static");
    assert_eq!(
        doc.add_variable_font(ROBOTO, 0, &FontVariationSettings::new()),
        Err(VariableFontError::NotVariable)
    );
    assert!(doc.resources.fonts.map.is_empty());

    // An instance is a static font too.
    let (instance, _) = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(500.0)).unwrap();
    assert_eq!(font_variation_axes(&instance, 0), Ok(Vec::new()));
}

#[test]
fn tags_are_validated() {
    assert_eq!("wght".parse::<FontVariationTag>(), Ok(FontVariationTag::WGHT));
    assert_eq!(FontVariationTag::try_from("opsz"), Ok(FontVariationTag::OPSZ));
    assert_eq!(FontVariationTag::new(*b"wdth"), Ok(FontVariationTag::WDTH));
    assert_eq!(FontVariationTag::WGHT.to_string(), "wght");
    assert_eq!(FontVariationTag::SLNT.as_bytes(), b"slnt");
    assert_eq!(FontVariationTag::ITAL.as_u32(), u32::from_be_bytes(*b"ital"));
    // a custom axis has upper case letters, and a tag may be padded with spaces
    assert!("XTRA".parse::<FontVariationTag>().is_ok());
    assert!("AB  ".parse::<FontVariationTag>().is_ok());

    for bad in [
        "", "wgh", "weight", "1bad", "a b ", " abc", "ab c", "wg\u{e9}", "wgh-", "\0\0\0\0",
    ] {
        assert_eq!(
            bad.parse::<FontVariationTag>(),
            Err(VariableFontError::InvalidTag(bad.to_string())),
            "{bad:?} is not a tag"
        );
    }
    assert!(FontVariationTag::new(*b"w\x7fht").is_err());
}

#[test]
fn settings_round_trip_through_json() {
    let settings = wght(650.5).with(FontVariationTag::OPSZ, 14.0);
    let json = serde_json::to_string(&settings).unwrap();
    assert_eq!(json, r#"{"opsz":14.0,"wght":650.5}"#);
    assert_eq!(serde_json::from_str::<FontVariationSettings>(&json).unwrap(), settings);
    assert!(serde_json::from_str::<FontVariationSettings>(r#"{"weight":1.0}"#).is_err());
    assert_eq!(settings.get(FontVariationTag::WGHT), Some(650.5));
    assert_eq!(settings.get(FontVariationTag::WDTH), None);
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

#[test]
fn unknown_axes_and_non_finite_values_are_errors() {
    // `wdth` is a valid tag, but Red Hat Mono has no such axis.
    let unknown = FontVariationSettings::new().with(FontVariationTag::WDTH, 100.0);
    assert_eq!(
        instantiate_variable_font_bytes(RED_HAT_MONO, 0, &unknown),
        Err(VariableFontError::UnknownAxis(FontVariationTag::WDTH))
    );
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(value)),
            Err(VariableFontError::NonFiniteValue {
                tag: FontVariationTag::WGHT
            }),
            "{value}"
        );
    }
    // add_variable_font runs the same checks, and registers nothing when they fail.
    let mut doc = PdfDocument::new("errors");
    assert!(doc.add_variable_font(RED_HAT_MONO, 0, &unknown).is_err());
    assert!(doc.add_variable_font(RED_HAT_MONO, 0, &wght(f32::NAN)).is_err());
    assert!(doc.resources.fonts.map.is_empty());
}

#[test]
fn values_are_clamped_and_the_effective_value_is_reported() {
    let axes = font_variation_axes(RED_HAT_MONO, 0).unwrap();
    let (min, max) = (axes[0].min, axes[0].max);
    let effective = |requested: f32| {
        let (bytes, resolved) =
            instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(requested)).unwrap();
        assert_eq!(resolved.coordinates.len(), 1);
        (bytes, resolved.get(FontVariationTag::WGHT).unwrap())
    };

    let (too_heavy, value) = effective(max + 10_000.0);
    assert_eq!(value, max);
    assert_eq!(too_heavy, effective(max).0, "a value past the range is the end of the range");

    let (too_light, value) = effective(-5.0);
    assert_eq!(value, min);
    assert_eq!(too_light, effective(min).0);

    assert_eq!(effective(450.5).1, 450.5);

    // An axis value is stored as 16.16 fixed point, and that is the value that is reported.
    let (_, value) = effective(400.1);
    assert!((value - 400.1).abs() < 1e-4, "{value}");
    assert_eq!(value, (400.1_f64 * 65536.0).round() as f32 / 65536.0);
}

#[test]
fn the_resolved_settings_list_every_axis() {
    // No axis is set: every axis of the font is at its default, and the result says so.
    let (default, resolved) =
        instantiate_variable_font_bytes(RED_HAT_MONO, 0, &FontVariationSettings::new()).unwrap();
    assert_eq!(resolved, wght(400.0));
    // The resolved settings are valid input and create the same font.
    let (again, _) = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &resolved).unwrap();
    assert_eq!(default, again);
}

#[test]
fn two_instances_of_one_font_differ_and_are_static() {
    let (light, light_settings) = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(300.0)).unwrap();
    let (bold, bold_settings) = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(700.0)).unwrap();
    assert_ne!(light, bold);
    assert_eq!(light_settings.get(FontVariationTag::WGHT), Some(300.0));
    assert_eq!(bold_settings.get(FontVariationTag::WGHT), Some(700.0));

    for bytes in [&light, &bold] {
        assert_eq!(variation_tables_in(bytes), Vec::<String>::new());
        assert!(has_table(bytes, "glyf"));
        // The tables that shaping reads survive.
        assert!(has_table(bytes, "GSUB") && has_table(bytes, "GPOS") && has_table(bytes, "cmap"));
    }
    // The weight of the instance is in the OS/2 weight class (a u16 at offset 4 of the table).
    let weight_class = |bytes: &[u8]| {
        let font_file = ReadScope::new(bytes).read::<FontData<'_>>().unwrap();
        let provider = font_file.table_provider(0).unwrap();
        let os2 = provider.read_table_data(tag::OS_2).unwrap().into_owned();
        u16::from_be_bytes([os2[4], os2[5]])
    };
    assert_eq!((weight_class(&light), weight_class(&bold)), (300, 700));
}

#[test]
fn the_default_instance_is_deterministic() {
    let first = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &FontVariationSettings::new()).unwrap();
    let second = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &FontVariationSettings::new()).unwrap();
    assert_eq!(first, second);
    // Naming the default is the same as leaving the axis out.
    assert_eq!(first, instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(400.0)).unwrap());
    assert_eq!(variation_tables_in(&first.0), Vec::<String>::new());
}

#[test]
fn a_face_index_that_does_not_exist_is_an_error() {
    let missing = VariableFontError::InvalidFaceIndex { index: 1 };
    assert_eq!(font_variation_axes(RED_HAT_MONO, 1), Err(missing.clone()));
    assert_eq!(
        instantiate_variable_font_bytes(RED_HAT_MONO, 1, &FontVariationSettings::new()),
        Err(missing.clone())
    );
    assert_eq!(
        PdfDocument::new("face").add_variable_font(RED_HAT_MONO, 1, &FontVariationSettings::new()),
        Err(missing)
    );
}

/// A font collection (`ttcf`) whose two faces are the same font, `sfnt`.
fn two_face_collection(sfnt: &[u8]) -> Vec<u8> {
    let header_len = 12 + 4 * 2; // 'ttcf', version, number of fonts, two offsets
    let mut font = sfnt.to_vec();
    // The offsets of the table records count from the start of the file, so they move by the
    // length of the header. A table record is a tag, a checksum, an offset and a length.
    let num_tables = usize::from(u16::from_be_bytes([font[4], font[5]]));
    for table in 0..num_tables {
        let at = 12 + 16 * table + 8;
        let offset = u32::from_be_bytes(font[at..at + 4].try_into().unwrap()) + header_len as u32;
        font[at..at + 4].copy_from_slice(&offset.to_be_bytes());
    }
    let mut collection = b"ttcf".to_vec();
    collection.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
    collection.extend_from_slice(&2_u32.to_be_bytes());
    collection.extend_from_slice(&(header_len as u32).to_be_bytes());
    collection.extend_from_slice(&(header_len as u32).to_be_bytes());
    collection.extend_from_slice(&font);
    collection
}

#[test]
fn a_face_of_a_collection_is_selected_by_its_index() {
    let collection = two_face_collection(RED_HAT_MONO);
    let axes = font_variation_axes(RED_HAT_MONO, 0).unwrap();
    for face in [0, 1] {
        assert_eq!(font_variation_axes(&collection, face).unwrap(), axes, "face {face}");
    }
    let missing = VariableFontError::InvalidFaceIndex { index: 2 };
    assert_eq!(font_variation_axes(&collection, 2), Err(missing.clone()));

    let single = instantiate_variable_font_bytes(RED_HAT_MONO, 0, &wght(650.0)).unwrap();
    assert_eq!(instantiate_variable_font_bytes(&collection, 1, &wght(650.0)).unwrap(), single);
    assert_eq!(instantiate_variable_font_bytes(&collection, 2, &wght(650.0)), Err(missing));
    assert!(PdfDocument::new("collection").add_variable_font(&collection, 1, &wght(650.0)).is_ok());
}

#[test]
fn damaged_font_files_are_errors() {
    assert!(matches!(
        font_variation_axes(b"not a font", 0),
        Err(VariableFontError::InvalidFont(_))
    ));
    assert!(matches!(font_variation_axes(&[], 0), Err(VariableFontError::InvalidFont(_))));
    // A cut-off file: the table directory is incomplete, or tables point past the end. allsorts
    // treats a table that does not fit as missing, so such a file is a font without axes.
    for len in [12, 100, 5_000] {
        let cut = &RED_HAT_MONO[..len];
        assert!(
            font_variation_axes(cut, 0).map_or(true, |axes| axes.is_empty()),
            "axes of the first {len} bytes"
        );
        assert!(
            instantiate_variable_font_bytes(cut, 0, &FontVariationSettings::new()).is_err(),
            "instance of the first {len} bytes"
        );
    }
    // Cut inside the gvar table: the axes are intact, the glyph variations are gone.
    let cut = &RED_HAT_MONO[..40_000];
    assert_eq!(font_variation_axes(cut, 0).unwrap().len(), 1);
    assert!(instantiate_variable_font_bytes(cut, 0, &FontVariationSettings::new()).is_err());
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

#[test]
fn two_instances_embed_in_one_pdf_and_extract_the_same_text() {
    let mut doc = PdfDocument::new("two weights");
    let light = doc.add_variable_font(RED_HAT_MONO, 0, &wght(300.0)).expect("light");
    let bold = doc.add_variable_font(RED_HAT_MONO, 0, &wght(700.0)).expect("bold");
    assert_ne!(light, bold);

    // The fonts of the document are the static instances, under different names.
    let name_of = |id: &FontId| {
        doc.resources.fonts.map[id]
            .parsed_font
            .font_name
            .clone()
            .expect("an instance has a font name")
    };
    assert_ne!(name_of(&light), name_of(&bold));
    for id in [&light, &bold] {
        let font = &doc.resources.fonts.map[id].parsed_font;
        let source = font.source_bytes().expect("source bytes");
        assert_eq!(variation_tables_in(source.as_slice()), Vec::<String>::new());
        assert!(font.gsub().is_some() && font.gpos().is_some(), "the layout tables are kept");
    }

    doc.pages.push(one_page(&[(&light, SAMPLE), (&bold, SAMPLE)]));
    for subset in [true, false] {
        let (pdf, warnings) = save(&doc, subset);
        assert_no_errors(&warnings, "two instances");

        let programs = embedded_programs(&pdf);
        assert_eq!(programs.len(), 2, "both instances are embedded (subset: {subset})");
        assert_ne!(programs[0].program, programs[1].program);
        assert_ne!(programs[0].base_font, programs[1].base_font);
        for program in &programs {
            assert_eq!(program.file_key, "FontFile2");
            assert_eq!(variation_tables_in(&program.program), Vec::<String>::new());
        }

        let text = extracted_text(&pdf);
        assert_eq!(text.matches(SAMPLE).count(), 2, "both lines extract: {text:?}");
        assert_poppler_accepts(&pdf, "two-instances", 2);
    }
}

#[test]
fn a_cff2_instance_is_cff1_and_embeds() {
    let (bytes, resolved) = instantiate_variable_font_bytes(CANTARELL, 0, &wght(700.0)).unwrap();
    assert_eq!(resolved, wght(700.0));
    assert_eq!(variation_tables_in(&bytes), Vec::<String>::new(), "no CFF2, no variation tables");
    assert!(bytes.starts_with(b"OTTO") && has_table(&bytes, "CFF "));
    // Glyphs keep their ids, so the layout tables of the source still apply.
    assert!(has_table(&bytes, "GSUB") && has_table(&bytes, "GPOS") && has_table(&bytes, "cmap"));

    let mut doc = PdfDocument::new("cff2");
    let heavy = doc.add_variable_font(CANTARELL, 0, &wght(700.0)).expect("heavy");
    let thin = doc.add_variable_font(CANTARELL, 0, &wght(100.0)).expect("thin");
    for id in [&heavy, &thin] {
        let font = &doc.resources.fonts.map[id].parsed_font;
        assert!(font.gsub().is_some() && font.gpos().is_some(), "the layout tables are kept");
    }
    doc.pages.push(one_page(&[(&heavy, SAMPLE), (&thin, SAMPLE)]));

    for subset in [true, false] {
        let (pdf, warnings) = save(&doc, subset);
        assert_no_errors(&warnings, "cff2");
        let programs = embedded_programs(&pdf);
        assert_eq!(programs.len(), 2, "subset: {subset}");
        assert_ne!(programs[0].program, programs[1].program);
        for program in &programs {
            assert_eq!(program.file_key, "FontFile3");
            if program.program.starts_with(b"OTTO") {
                assert_eq!(variation_tables_in(&program.program), Vec::<String>::new());
                assert!(has_table(&program.program, "CFF "));
            } else {
                // A bare CID-keyed CFF: the header starts with the major version, 1 (CFF2 is 2).
                assert_eq!(program.program.first(), Some(&1), "{} is not CFF1", program.base_font);
            }
        }
        let text = extracted_text(&pdf);
        assert_eq!(text.matches(SAMPLE).count(), 2, "both lines extract: {text:?}");
        assert_poppler_accepts(&pdf, "cff2", 2);
    }
}

// ---------------------------------------------------------------------------
// The warning for a variable font that was not instantiated
// ---------------------------------------------------------------------------

/// The messages of `save` about a variable font that is embedded at its default instance.
fn unresolved_variable_font_warnings(warnings: &[PdfWarnMsg]) -> Vec<&PdfWarnMsg> {
    warnings
        .iter()
        .filter(|w| w.msg.contains("add_variable_font"))
        .collect()
}

#[test]
fn add_font_with_a_variable_font_warns_on_save() {
    for (bytes, outlines) in [(RED_HAT_MONO, "glyf and gvar"), (CANTARELL, "CFF2")] {
        let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("the font parses");
        let mut doc = PdfDocument::new("unresolved");
        let id = doc.add_font(&font);
        doc.pages.push(one_page(&[(&id, SAMPLE)]));
        for subset in [true, false] {
            let (pdf, warnings) = save(&doc, subset);
            let found = unresolved_variable_font_warnings(&warnings);
            assert_eq!(found.len(), 1, "{outlines}, subset: {subset}: {warnings:#?}");
            assert_eq!(found[0].severity, PdfParseErrorSeverity::Warning);
            assert!(found[0].msg.contains("default instance"), "{}", found[0].msg);
            // It is a warning and not a refusal: the font is still written.
            assert_eq!(embedded_programs(&pdf).len(), 1, "{outlines}, subset: {subset}");
        }
    }
}

#[test]
fn a_baked_instance_a_static_font_and_an_unused_font_do_not_warn() {
    let parse = |bytes: &[u8]| ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).expect("parses");
    let mut doc = PdfDocument::new("quiet");
    let glyf = doc.add_variable_font(RED_HAT_MONO, 0, &wght(500.0)).unwrap();
    let cff2 = doc.add_variable_font(CANTARELL, 0, &wght(500.0)).unwrap();
    let static_font = doc.add_font(&parse(ROBOTO));
    // A variable font that no text uses is not embedded, so there is nothing to warn about.
    let _unused = doc.add_font(&parse(RED_HAT_MONO));
    doc.pages.push(one_page(&[(&glyf, SAMPLE), (&cff2, SAMPLE), (&static_font, SAMPLE)]));
    for subset in [true, false] {
        let (_, warnings) = save(&doc, subset);
        let found = unresolved_variable_font_warnings(&warnings);
        assert!(found.is_empty(), "subset: {subset}: {found:#?}");
    }
}
