#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

/// Regression test for the "every custom font-family is UNRESOLVED" bug
/// (issue #220 finding F3, wasm-demo finding II.2).
///
/// Fonts supplied via `from_html`'s fonts map went into the FcFontCache but
/// azul's family resolution only consults `FontManager::memory_families`,
/// which nothing populated — so a specific family (especially multi-word,
/// which additionally arrives quote-wrapped at the resolver) always fell back
/// and the supplied font never reached the PDF. `register_input_fonts`
/// (src/html/mod.rs) fixes that; this asserts the supplied face is really the
/// embedded one.
#[test]
fn multi_word_font_family_from_fonts_map_is_embedded() {
    let mut fonts = BTreeMap::new();
    fonts.insert(
        "Roboto Medium".to_string(),
        Base64OrRaw::Raw(std::fs::read("examples/assets/fonts/RobotoMedium.ttf").unwrap()),
    );
    let html = r#"<html><body>
        <p style="font-family: 'Roboto Medium'; font-size: 24px;">Hello Probe Font</p>
    </body></html>"#;

    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("from_html");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    let parsed = lopdf::Document::load_mem(&bytes).expect("output parses");
    let mut base_fonts = Vec::new();
    for (_, obj) in parsed.objects.iter() {
        if let lopdf::Object::Dictionary(d) = obj {
            let is_font = d
                .get(b"Type")
                .and_then(|o| o.as_name())
                .map(|n| n == b"Font")
                .unwrap_or(false);
            if is_font {
                if let Ok(bf) = d.get(b"BaseFont").and_then(|o| o.as_name()) {
                    base_fonts.push(String::from_utf8_lossy(bf).to_string());
                }
            }
        }
    }
    assert!(
        base_fonts.iter().any(|f| f.contains("Roboto")),
        "the supplied 'Roboto Medium' must be the embedded font, found: {base_fonts:?}"
    );
}
