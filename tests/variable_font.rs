use allsorts::{binary::read::ReadScope, font_data::FontData, tables::FontTableProvider, tag};
use printpdf::{
    font_variation_axes, instantiate_variable_font_bytes, FontVariationSettings, FontVariationTag,
    VariableFontError,
};

const ADWAITA_SANS_VARIABLE: &[u8] =
    include_bytes!("assets/variable-fonts/AdwaitaSans-Regular.ttf");
const CANTARELL_VARIABLE: &[u8] = include_bytes!("assets/variable-fonts/Cantarell-VF.otf");

fn has_table(bytes: &[u8], table: u32) -> bool {
    let scope = ReadScope::new(bytes);
    let font_file = scope.read::<FontData<'_>>().unwrap();
    let provider = font_file.table_provider(0).unwrap();
    provider.has_table(table)
}

#[test]
fn exposes_axes_and_validates_tags() {
    let axes = font_variation_axes(ADWAITA_SANS_VARIABLE, 0).unwrap();
    let weight = axes
        .iter()
        .find(|axis| axis.tag == FontVariationTag::WGHT)
        .expect("Adwaita Sans must expose its weight axis");

    assert!(weight.min_value < weight.default_value);
    assert!(weight.default_value < weight.max_value);
    assert!(weight.name.is_some());
    assert_eq!(
        "wght".parse::<FontVariationTag>().unwrap(),
        FontVariationTag::WGHT
    );
    assert!("weight".parse::<FontVariationTag>().is_err());
    assert!("1bad".parse::<FontVariationTag>().is_err());
    assert!("a b ".parse::<FontVariationTag>().is_err());
}

#[test]
fn rejects_unknown_axes_and_non_finite_values() {
    let unknown = "TEST".parse::<FontVariationTag>().unwrap();
    let settings = FontVariationSettings::new().with(unknown, 1.0);
    let error =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &settings, &mut Vec::new())
            .unwrap_err();
    assert_eq!(error, VariableFontError::UnknownAxis(unknown));

    let settings = FontVariationSettings::new().with(FontVariationTag::WGHT, f32::NAN);
    assert!(matches!(
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &settings, &mut Vec::new(),),
        Err(VariableFontError::NonFiniteValue { .. })
    ));
}

#[test]
fn creates_distinct_static_truetype_instances() {
    let light = FontVariationSettings::new().with(FontVariationTag::WGHT, 300.0);
    let bold = FontVariationSettings::new().with(FontVariationTag::WGHT, 800.0);
    let (light_bytes, light_coordinates) =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &light, &mut Vec::new()).unwrap();
    let (bold_bytes, bold_coordinates) =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &bold, &mut Vec::new()).unwrap();

    assert_ne!(light_bytes, bold_bytes);
    assert_eq!(light_coordinates[&FontVariationTag::WGHT], 300.0);
    assert_eq!(bold_coordinates[&FontVariationTag::WGHT], 800.0);
    assert!(has_table(&light_bytes, tag::GLYF));
    assert!(!has_table(&light_bytes, tag::FVAR));
    assert!(!has_table(&light_bytes, tag::GVAR));
    assert!(!has_table(&light_bytes, tag::HVAR));
    assert!(!has_table(&light_bytes, tag::MVAR));
}

#[test]
fn default_instances_are_deterministic_and_static() {
    let settings = FontVariationSettings::new();
    let first =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &settings, &mut Vec::new())
            .unwrap();
    let second =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &settings, &mut Vec::new())
            .unwrap();

    assert_eq!(first, second);
    assert!(first.1.contains_key(&FontVariationTag::WGHT));
    assert!(!has_table(&first.0, tag::FVAR));
}

#[test]
fn rejects_static_fonts_and_invalid_collection_indices() {
    let static_font = instantiate_variable_font_bytes(
        ADWAITA_SANS_VARIABLE,
        0,
        &FontVariationSettings::new(),
        &mut Vec::new(),
    )
    .unwrap()
    .0;
    assert_eq!(
        font_variation_axes(&static_font, 0).unwrap_err(),
        VariableFontError::NotVariable
    );
    assert!(matches!(
        font_variation_axes(ADWAITA_SANS_VARIABLE, 1),
        Err(VariableFontError::InvalidCollectionIndex { index: 1 })
    ));
}

#[test]
fn clamps_coordinates_and_reports_the_effective_value() {
    let axes = font_variation_axes(ADWAITA_SANS_VARIABLE, 0).unwrap();
    let weight = axes
        .iter()
        .find(|axis| axis.tag == FontVariationTag::WGHT)
        .unwrap();
    let settings =
        FontVariationSettings::new().with(FontVariationTag::WGHT, weight.max_value + 10_000.0);
    let mut warnings = Vec::new();
    let (_, coordinates) =
        instantiate_variable_font_bytes(ADWAITA_SANS_VARIABLE, 0, &settings, &mut warnings)
            .unwrap();

    assert_eq!(coordinates[&FontVariationTag::WGHT], weight.max_value);
    assert!(warnings
        .iter()
        .any(|warning| warning.msg.contains("clamped")));
}

#[test]
fn converts_cff2_to_static_cff1() {
    let axes = font_variation_axes(CANTARELL_VARIABLE, 0).unwrap();
    assert!(axes.iter().any(|axis| axis.tag == FontVariationTag::WGHT));

    let settings = FontVariationSettings::new().with(FontVariationTag::WGHT, 700.0);
    let (bytes, coordinates) =
        instantiate_variable_font_bytes(CANTARELL_VARIABLE, 0, &settings, &mut Vec::new()).unwrap();

    assert_eq!(coordinates[&FontVariationTag::WGHT], 700.0);
    assert!(has_table(&bytes, tag::CFF));
    assert!(!has_table(&bytes, tag::CFF2));
    assert!(!has_table(&bytes, tag::FVAR));
    assert!(!has_table(&bytes, tag::HVAR));
    assert!(!has_table(&bytes, tag::MVAR));
}

#[cfg(feature = "text_layout")]
#[test]
fn registers_two_instances_and_embeds_static_fonts() {
    use printpdf::{
        FontType, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfParseOptions, PdfSaveOptions, Pt,
        TextItem,
    };

    let mut document = PdfDocument::new("Variable font test");
    let mut warnings = Vec::new();
    let light_id = document
        .add_variable_font(
            ADWAITA_SANS_VARIABLE,
            0,
            &FontVariationSettings::new().with(FontVariationTag::WGHT, 300.0),
            &mut warnings,
        )
        .unwrap();
    let bold_id = document
        .add_variable_font(
            ADWAITA_SANS_VARIABLE,
            0,
            &FontVariationSettings::new().with(FontVariationTag::WGHT, 800.0),
            &mut warnings,
        )
        .unwrap();

    assert_ne!(
        document.resources.fonts.map[&light_id]
            .parsed_font
            .original_bytes,
        document.resources.fonts.map[&bold_id]
            .parsed_font
            .original_bytes
    );
    assert!(matches!(
        document.resources.fonts.map[&light_id]
            .parsed_font
            .font_type,
        FontType::TrueType
    ));

    document.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(light_id),
                size: Pt(18.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Light".to_string())],
            },
            Op::SetFont {
                font: PdfFontHandle::External(bold_id),
                size: Pt(18.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Bold".to_string())],
            },
            Op::EndTextSection,
        ],
    ));

    let pdf = document.save(&PdfSaveOptions::default(), &mut warnings);
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let reparsed = PdfDocument::parse(&pdf, &PdfParseOptions::default(), &mut warnings).unwrap();
    let extracted = reparsed.extract_text().into_iter().flatten().collect::<String>();
    assert!(extracted.contains("Light"));
    assert!(extracted.contains("Bold"));
    let embedded_sfnt = parsed
        .objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|stream| {
            stream.content.starts_with(&[0, 1, 0, 0])
                || stream.content.starts_with(b"true")
                || stream.content.starts_with(b"OTTO")
        })
        .collect::<Vec<_>>();

    assert_eq!(embedded_sfnt.len(), 2);
    for stream in embedded_sfnt {
        assert!(!has_table(&stream.content, tag::FVAR));
        assert!(!has_table(&stream.content, tag::GVAR));
    }
}

#[cfg(feature = "text_layout")]
#[test]
fn cff2_document_instance_uses_opentype_font_stream() {
    use printpdf::{
        FontType, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt, TextItem,
    };

    let mut document = PdfDocument::new("CFF2 variable font test");
    let mut warnings = Vec::new();
    let font_id = document
        .add_variable_font(
            CANTARELL_VARIABLE,
            0,
            &FontVariationSettings::new().with(FontVariationTag::WGHT, 650.0),
            &mut warnings,
        )
        .unwrap();
    assert!(matches!(
        document.resources.fonts.map[&font_id].parsed_font.font_type,
        FontType::OpenTypeCFF(_)
    ));
    document.pages.push(PdfPage::new(
        Mm(210.0),
        Mm(297.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font_id),
                size: Pt(18.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Cantarell".to_string())],
            },
            Op::EndTextSection,
        ],
    ));

    let pdf = document.save(&PdfSaveOptions::default(), &mut warnings);
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    assert!(parsed.objects.values().any(|object| {
        let Ok(stream) = object.as_stream() else {
            return false;
        };
        stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            == Some(b"OpenType".as_slice())
            && stream.content.starts_with(b"OTTO")
    }));
}

#[cfg(feature = "text_layout")]
#[test]
fn full_embedding_also_uses_only_static_instance_bytes() {
    use printpdf::{Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt, TextItem};

    let mut document = PdfDocument::new("Full variable font embedding");
    let mut warnings = Vec::new();
    let font_id = document
        .add_variable_font(
            ADWAITA_SANS_VARIABLE,
            0,
            &FontVariationSettings::new().with(FontVariationTag::WGHT, 550.0),
            &mut warnings,
        )
        .unwrap();
    document.pages.push(PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font_id),
                size: Pt(12.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Static".into())],
            },
            Op::EndTextSection,
        ],
    ));

    let pdf = document.save(
        &PdfSaveOptions {
            subset_fonts: false,
            ..Default::default()
        },
        &mut warnings,
    );
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let embedded = parsed
        .objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .find(|stream| {
            stream.content.starts_with(&[0, 1, 0, 0])
                || stream.content.starts_with(b"true")
                || stream.content.starts_with(b"OTTO")
        })
        .expect("embedded font stream");
    assert!(!has_table(&embedded.content, tag::FVAR));
    assert!(!has_table(&embedded.content, tag::GVAR));
}

#[cfg(feature = "text_layout")]
#[test]
fn legacy_font_registration_refuses_unresolved_variable_bytes() {
    use printpdf::{
        Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt, TextItem,
    };

    let mut parse_warnings = Vec::new();
    let parsed_font = ParsedFont::from_bytes(ADWAITA_SANS_VARIABLE, 0, &mut parse_warnings)
        .expect("fixture parses");
    let mut document = PdfDocument::new("Reject unresolved font");
    let font_id = document.add_font(&parsed_font);
    document.pages.push(PdfPage::new(
        Mm(100.0),
        Mm(100.0),
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font_id),
                size: Pt(12.0),
            },
            Op::ShowText {
                items: vec![TextItem::Text("Variable".into())],
            },
            Op::EndTextSection,
        ],
    ));

    let mut warnings = Vec::new();
    let pdf = document.save(&PdfSaveOptions::default(), &mut warnings);
    assert!(warnings
        .iter()
        .any(|warning| warning.msg.contains("Refusing to embed font")));
    assert!(!pdf
        .windows(ADWAITA_SANS_VARIABLE.len())
        .any(|window| window == ADWAITA_SANS_VARIABLE));
}
