#![cfg(feature = "html")]
//! Regression test for the page -> SVG preview renderer (`PdfPage::to_svg`).
//!
//! Guards against two bugs that made preview text invisible / mis-rendered:
//!   1. The PDF(bottom-left) -> SVG(top-left) Y-flip double-applied the glyph
//!      position (once as the `x`/`y` attribute, once in the `transform` matrix),
//!      pushing every glyph hundreds of points above the page.
//!   2. `SetFont` never recorded the current font, so text fell back to the
//!      builtin Times family instead of the embedded (external) font, and the
//!      `@font-face` family did not match the `<text>` `font-family`.
//!
//! This test fails against the pre-fix renderer and passes after the fix.

use printpdf::*;
use std::collections::BTreeMap;

/// Minimal `<text ...>content</text>` view extracted from the SVG.
struct SvgText {
    font_family: String,
    y_attr: f32,
    matrix: [f32; 6],
    content: String,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{}=\"", name);
    let start = tag.find(&key)? + key.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn parse_matrix(tag: &str) -> Option<[f32; 6]> {
    let start = tag.find("transform=\"matrix(")? + "transform=\"matrix(".len();
    let rest = &tag[start..];
    let end = rest.find(')')?;
    let nums: Vec<f32> = rest[..end]
        .split_whitespace()
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    if nums.len() == 6 {
        Some([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]])
    } else {
        None
    }
}

fn parse_texts(svg: &str) -> Vec<SvgText> {
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(open_rel) = svg[cursor..].find("<text") {
        let open = cursor + open_rel;
        let tag_end = match svg[open..].find('>') {
            Some(i) => open + i,
            None => break,
        };
        let tag = &svg[open..tag_end];
        let close = match svg[tag_end..].find("</text>") {
            Some(i) => tag_end + i,
            None => break,
        };
        let content = &svg[tag_end + 1..close];
        if let (Some(fam), Some(y), Some(m)) = (
            attr(tag, "font-family"),
            attr(tag, "y").and_then(|v| v.parse::<f32>().ok()),
            parse_matrix(tag),
        ) {
            out.push(SvgText {
                font_family: fam.to_string(),
                y_attr: y,
                matrix: m,
                content: content.to_string(),
            });
        }
        cursor = close + "</text>".len();
    }
    out
}

/// The `@font-face` family declared in the `<style>` block (first rule).
fn font_face_family(svg: &str) -> Option<String> {
    let start = svg.find("@font-face")?;
    let rest = &svg[start..];
    let key = "font-family: \"";
    let fstart = rest.find(key)? + key.len();
    let frest = &rest[fstart..];
    let end = frest.find('"')?;
    Some(frest[..end].to_string())
}

fn font_face_has_base64(svg: &str) -> bool {
    // The embedded font must actually carry bytes so the browser can render it.
    if let Some(i) = svg.find("base64,") {
        let after = &svg[i + "base64,".len()..];
        let len: usize = after.chars().take_while(|c| *c != '"').count();
        len > 1000
    } else {
        false
    }
}

#[test]
fn svg_preview_text_is_visible_upright_onpage_in_embedded_font() {
    let font = std::fs::read("examples/assets/fonts/RobotoMedium.ttf").unwrap();
    let mut fonts = BTreeMap::new();
    fonts.insert("Roboto Medium".to_string(), Base64OrRaw::Raw(font));
    let html = r#"<html><body><p style="font-family:'Roboto Medium';font-size:24px;">HELLO PREVIEW</p></body></html>"#;

    let mut w = Vec::new();
    let doc = PdfDocument::from_html(
        html,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut w,
    )
    .unwrap();
    let page = &doc.pages[0];
    let page_height = page.media_box.height.0;
    let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut w);

    let texts = parse_texts(&svg);
    assert!(
        !texts.is_empty(),
        "preview SVG contains no parseable <text> elements"
    );

    // (a) Readable unicode: glyphs are emitted one <text> per glyph, so
    // concatenating the element contents must reconstruct the source string.
    let joined: String = texts.iter().map(|t| t.content.as_str()).collect();
    assert!(
        joined.contains("HELLO"),
        "preview SVG text does not contain readable unicode 'HELLO' (got {joined:?})"
    );

    // (b) On-page + upright: the effective SVG baseline y for each glyph must
    // land inside [0, page_height], and the vertical scale (matrix `d`) must be
    // positive so the glyph is not mirrored / upside down.
    for t in &texts {
        // SVG maps (x_attr, y_attr) through the matrix: y' = b*x + d*y + f.
        let effective_y = t.matrix[1] * 0.0 + t.matrix[3] * t.y_attr + t.matrix[5];
        assert!(
            effective_y >= 0.0 && effective_y <= page_height,
            "glyph {:?} is off-page: effective y = {effective_y} not in [0, {page_height}] \
             (y_attr={}, matrix={:?})",
            t.content,
            t.y_attr,
            t.matrix,
        );
        assert!(
            t.matrix[3] > 0.0,
            "glyph {:?} is vertically mirrored (matrix d = {} <= 0)",
            t.content,
            t.matrix[3],
        );
    }

    // (c) Embedded font is actually used: a custom font was supplied, so there
    // must be an @font-face whose family exactly matches the <text> font-family,
    // and it must carry base64 font bytes. Otherwise the browser falls back.
    let text_family = &texts[0].font_family;
    assert!(
        !text_family.contains("Times") && !text_family.contains("serif"),
        "text fell back to a builtin family instead of the embedded font: {text_family:?}"
    );
    let face_family =
        font_face_family(&svg).expect("no @font-face rule emitted for the embedded custom font");
    assert_eq!(
        &face_family, text_family,
        "@font-face family {face_family:?} does not match <text> font-family {text_family:?}; \
         the browser would fall back to a default font"
    );
    assert!(
        font_face_has_base64(&svg),
        "@font-face does not embed base64 font bytes"
    );
}
