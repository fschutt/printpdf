#![cfg(feature = "html")]
//! Arabic, end to end: HTML -> shaped glyphs -> PDF content stream / ToUnicode
//! -> SVG preview.
//!
//! azul's text3 shapes Arabic correctly (contextual init/medi/fina forms, the
//! lam-alef ligature, right-to-left visual order) and the PDF writer keeps those
//! glyph ids. The SVG preview (`PdfPage::to_svg`, the web demo's page view) did
//! not: it wrote each shaped glyph back out as its *Unicode* text, one `<text>`
//! element per glyph, and the browser shaped every one of those characters on
//! its own. A lone letter has no neighbours to join to, so the preview showed
//! the isolated form of every letter at the position of its contextual form,
//! and a parsed PDF's visual-order run was bidi-reversed a second time. The
//! preview now paints the glyph ids it was given with their outlines, and keeps
//! the `<text>` only as an unpainted layer for selection and search.
//!
//! Font: `tests/assets/fonts/arabic/Vazirmatn-ArabicTest.ttf`, Vazirmatn
//! (OFL, no Reserved Font Name, licence next to it) instanced at wght=400 and
//! subset to the test string with every layout feature kept:
//!
//! ```text
//! fonttools varLib.instancer Vazirmatn[wght].ttf wght=400 -o Vazirmatn-Regular.ttf
//! pyftsubset Vazirmatn-Regular.ttf --text="مرحبا بالعالم لا" --layout-features='*' \
//!     --no-hinting --glyph-names --output-file=Vazirmatn-ArabicTest.ttf
//! ```
//!
//! It has precomposed letters (no dots split off into mark glyphs) and a real
//! lam-alef ligature glyph, so every glyph below is a whole letter.

use std::collections::BTreeMap;

use printpdf::*;

const FONT: &[u8] = include_bytes!("assets/fonts/arabic/Vazirmatn-ArabicTest.ttf");
const FAMILY: &str = "Vazirmatn";
/// "Hello world" and the word "no", which is the lam-alef ligature.
const TEXT: &str = "مرحبا بالعالم لا";

/// HarfBuzz's shaping of `TEXT` in this font (`hb-shape`, script arab,
/// direction rtl), in visual order, left to right: the glyph id, its name in
/// the font, and the text it stands for.
const EXPECTED_VISUAL: [(u16, &str, &str); 15] = [
    (29, "uniFEFB lam-alef ligature", "لا"),
    (2, "space", " "),
    (26, "uniFEE2 meem final", "م"),
    (24, "uniFEDF lam initial", "ل"),
    (12, "uniFE8E alef final", "ا"),
    (22, "uniFECC ain medial", "ع"),
    (24, "uniFEDF lam initial", "ل"),
    (12, "uniFE8E alef final", "ا"),
    (14, "uniFE91 beh initial", "ب"),
    (2, "space", " "),
    (12, "uniFE8E alef final", "ا"),
    (15, "uniFE92 beh medial", "ب"),
    (17, "uniFEA3 hah initial", "ح"),
    (19, "uniFEAE reh final", "ر"),
    (27, "uniFEE3 meem initial", "م"),
];

/// The nominal (isolated) glyph each letter maps to through the font's cmap.
/// None of them may appear in shaped output: every letter here joins.
const NOMINAL: [(char, u16); 7] = [
    ('ا', 3),
    ('ب', 4),
    ('ح', 5),
    ('ر', 6),
    ('ع', 7),
    ('ل', 8),
    ('م', 9),
];

fn render() -> PdfDocument {
    let html = format!(
        r#"<html><head><style>
            body {{ font-family: '{FAMILY}'; font-size: 24px; margin: 20px; }}
        </style></head>
        <body><p dir="rtl" lang="ar">{TEXT}</p></body></html>"#
    );
    let mut fonts = BTreeMap::new();
    fonts.insert(FAMILY.to_string(), Base64OrRaw::Raw(FONT.to_vec()));
    let mut warnings = Vec::new();
    PdfDocument::from_html(
        &html,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("from_html")
}

/// One painted glyph of the HTML document: where its text matrix puts it and
/// what it shows.
#[derive(Debug, Clone)]
struct PlacedGlyph {
    x: f32,
    y: f32,
    gid: u16,
    text: String,
}

/// Every external-font glyph of page 1 with its `Tm` origin (PDF space).
fn placed_glyphs(doc: &PdfDocument) -> Vec<PlacedGlyph> {
    let mut out = Vec::new();
    let mut origin = (0.0, 0.0);
    let mut external = false;
    for op in &doc.pages[0].ops {
        match op {
            Op::SetFont { font, .. } => external = matches!(font, PdfFontHandle::External(_)),
            Op::SetTextMatrix { matrix } => {
                let m = matrix.as_array();
                origin = (m[4], m[5]);
            }
            Op::ShowText { items } if external => {
                for item in items {
                    if let TextItem::GlyphIds(glyphs) = item {
                        for g in glyphs {
                            out.push(PlacedGlyph {
                                x: origin.0,
                                y: origin.1,
                                gid: g.gid,
                                text: g.cid.clone().unwrap_or_default(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn expected_gids() -> Vec<u16> {
    EXPECTED_VISUAL.iter().map(|(gid, _, _)| *gid).collect()
}

/// Concatenates the per-glyph texts of a visual (left-to-right) RTL run in
/// reading order.
fn logical_text<'a>(visual: impl DoubleEndedIterator<Item = &'a str>) -> String {
    visual.rev().collect()
}

#[test]
fn html_shapes_arabic_into_contextual_forms_in_rtl_order() {
    let doc = render();
    let mut glyphs = placed_glyphs(&doc);
    assert_eq!(glyphs.len(), EXPECTED_VISUAL.len(), "glyphs: {glyphs:?}");

    // Left to right on the page.
    glyphs.sort_by(|a, b| a.x.total_cmp(&b.x));
    let gids: Vec<u16> = glyphs.iter().map(|g| g.gid).collect();
    assert_eq!(
        gids,
        expected_gids(),
        "shaped glyph ids (left to right) differ from HarfBuzz's: {:?}",
        EXPECTED_VISUAL
    );

    for (ch, nominal) in NOMINAL {
        assert!(
            !gids.contains(&nominal),
            "{ch:?} is drawn with its isolated cmap glyph {nominal}, not a contextual form"
        );
    }

    // One baseline, and the first letter of the text is the rightmost glyph.
    assert!(glyphs.iter().all(|g| (g.y - glyphs[0].y).abs() < 0.01));
    assert_eq!(glyphs.last().unwrap().text, "م");
    assert_eq!(
        glyphs.first().unwrap().text,
        "لا",
        "lam-alef is one ligature glyph"
    );
    assert_eq!(logical_text(glyphs.iter().map(|g| g.text.as_str())), TEXT);
}

/// Hex-string glyph codes shown by `Tj`/`TJ` on page 1, in content order.
fn content_codes(pdf: &lopdf::Document) -> Vec<u16> {
    fn collect(obj: &lopdf::Object, out: &mut Vec<u16>) {
        match obj {
            lopdf::Object::String(bytes, lopdf::StringFormat::Hexadecimal) => {
                out.extend(
                    bytes
                        .chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]])),
                );
            }
            lopdf::Object::Array(items) => items.iter().for_each(|i| collect(i, out)),
            _ => {}
        }
    }
    let (_, page) = pdf.get_pages().into_iter().next().expect("one page");
    let content = pdf
        .get_and_decode_page_content(page)
        .expect("content parses");
    let mut out = Vec::new();
    for op in &content.operations {
        if op.operator == "Tj" || op.operator == "TJ" {
            op.operands.iter().for_each(|o| collect(o, &mut out));
        }
    }
    out
}

fn deref<'a>(pdf: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj.as_reference() {
        Ok(r) => pdf.get_object(r).expect("dangling reference"),
        Err(_) => obj,
    }
}

/// The `bfchar` entries of the (only) Type0 font's ToUnicode CMap.
fn to_unicode(pdf: &lopdf::Document) -> BTreeMap<u16, String> {
    let font = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find(|d| d.get(b"Subtype").and_then(|s| s.as_name()).ok() == Some(b"Type0".as_slice()))
        .expect("a Type0 font");
    let lopdf::Object::Stream(cmap) = deref(pdf, font.get(b"ToUnicode").expect("ToUnicode")) else {
        panic!("ToUnicode is not a stream");
    };
    let bytes = cmap
        .decompressed_content()
        .unwrap_or_else(|_| cmap.content.clone());
    let cmap = String::from_utf8(bytes).expect("ascii CMap");

    let hex = |s: &str| -> Vec<u16> {
        let s = s.trim_matches(|c| c == '<' || c == '>');
        (0..s.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
            .collect()
    };
    let mut map = BTreeMap::new();
    let mut in_bfchar = false;
    for line in cmap.lines().map(str::trim) {
        if line.ends_with("beginbfchar") {
            in_bfchar = true;
        } else if line == "endbfchar" {
            in_bfchar = false;
        } else if in_bfchar {
            let mut parts = line.split_whitespace();
            let (Some(code), Some(text)) = (parts.next(), parts.next()) else {
                continue;
            };
            map.insert(hex(code)[0], String::from_utf16(&hex(text)).unwrap());
        }
    }
    map
}

#[test]
fn pdf_writes_the_shaped_glyph_ids_and_tounicode_restores_the_text() {
    let doc = render();

    // Without subsetting the codes ARE the font's glyph ids (Identity-H,
    // /CIDToGIDMap /Identity): the content stream must carry the contextual
    // forms, left to right, not a codepoint -> cmap re-mapping.
    let mut w = Vec::new();
    let full = doc.save(
        &PdfSaveOptions {
            subset_fonts: false,
            ..Default::default()
        },
        &mut w,
    );
    let full = lopdf::Document::load_mem(&full).expect("PDF parses");
    assert_eq!(content_codes(&full), expected_gids());

    // Subset (the default): codes are renumbered, and ToUnicode must map them
    // back to the source text, the ligature to both of its letters.
    for pdf in [
        full,
        lopdf::Document::load_mem(&doc.save(&PdfSaveOptions::default(), &mut w)).unwrap(),
    ] {
        let cmap = to_unicode(&pdf);
        let texts: Vec<&str> = content_codes(&pdf)
            .iter()
            .map(|c| cmap.get(c).map(String::as_str).unwrap_or("\u{FFFD}"))
            .collect();
        let expected: Vec<&str> = EXPECTED_VISUAL.iter().map(|(_, _, t)| *t).collect();
        assert_eq!(
            texts, expected,
            "ToUnicode of the shown codes, left to right"
        );
        assert_eq!(logical_text(texts.into_iter()), TEXT);
    }
}

/// `<use>` elements of a preview SVG: (referenced id, transform matrix).
fn svg_uses(svg: &str) -> Vec<(String, [f32; 6])> {
    let mut out = Vec::new();
    for tag in svg.split("<use ").skip(1) {
        let tag = &tag[..tag.find('>').unwrap()];
        let href = attr(tag, "xlink:href").expect("<use> without xlink:href");
        out.push((
            href.trim_start_matches('#').to_string(),
            matrix(tag).expect("<use> matrix"),
        ));
    }
    out
}

/// The value of attribute `name` in `tag` (the text between `<elem` and `>`).
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut from = 0;
    while let Some(i) = tag[from..].find(&key).map(|i| from + i) {
        // A whole attribute name, not the tail of a longer one (`fill` in `xfill`).
        if i == 0 || tag[..i].ends_with(char::is_whitespace) {
            let rest = &tag[i + key.len()..];
            return Some(&rest[..rest.find('"')?]);
        }
        from = i + key.len();
    }
    None
}

fn matrix(tag: &str) -> Option<[f32; 6]> {
    let m = attr(tag, "transform")?
        .strip_prefix("matrix(")?
        .strip_suffix(')')?;
    let v: Vec<f32> = m
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    v.try_into().ok()
}

/// The path data the preview defines for `id`, if any.
fn svg_path_def<'a>(svg: &'a str, id: &str) -> Option<&'a str> {
    let start = svg.find(&format!("<path id=\"{id}\""))? + "<path".len();
    let tag = &svg[start..];
    attr(&tag[..tag.find('>')?], "d")
}

/// The glyph id a preview `<use>` references (`glyph-<font>-<gid>`).
fn gid_of(def_id: &str) -> u16 {
    def_id
        .rsplit('-')
        .next()
        .unwrap()
        .parse()
        .expect("glyph def id ends in the gid")
}

fn is_arabic(c: char) -> bool {
    ('\u{0600}'..='\u{06FF}').contains(&c) || ('\u{FB50}'..='\u{FEFF}').contains(&c)
}

/// Every `<text>` holding Arabic must be unpainted (the selection layer):
/// a painted one is the browser drawing its own, unjoined, shaping.
fn assert_no_painted_arabic_text(svg: &str) {
    for el in svg.split("<text").skip(1) {
        let (tag, rest) = el.split_once('>').unwrap();
        let content = &rest[..rest.find("</text>").unwrap()];
        if content.chars().any(is_arabic) {
            assert!(
                attr(tag, "fill-opacity") == Some("0") && attr(tag, "stroke") == Some("none"),
                "the preview paints Arabic as <text> (the browser re-shapes it letter by letter): \
                 <text{tag}>{content}</text>"
            );
        }
    }
}

#[test]
fn svg_preview_draws_the_shaped_glyphs_not_reshaped_unicode() {
    let doc = render();
    let page = &doc.pages[0];
    let page_height = page.media_box.height.0;
    let svg = page.to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut Vec::new());

    assert_no_painted_arabic_text(&svg);

    // Every non-blank glyph is painted from the outline of exactly the glyph
    // id the shaper chose, at exactly the position the PDF puts it.
    let mut placed: Vec<PlacedGlyph> = placed_glyphs(&doc)
        .into_iter()
        .filter(|g| g.text != " ")
        .collect();
    placed.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut uses = svg_uses(&svg);
    uses.sort_by(|a, b| a.1[4].total_cmp(&b.1[4]));
    assert_eq!(
        uses.iter().map(|(id, _)| gid_of(id)).collect::<Vec<_>>(),
        placed.iter().map(|g| g.gid).collect::<Vec<_>>(),
        "the preview must paint the shaped glyph ids, left to right"
    );
    for ((id, m), g) in uses.iter().zip(&placed) {
        let d = svg_path_def(&svg, id).unwrap_or_else(|| panic!("no outline <path> for {id}"));
        assert!(
            d.starts_with('M') && d.len() > 20,
            "empty outline for {id}: {d:?}"
        );
        assert!(
            (m[4] - g.x).abs() < 0.01,
            "{id}: x {} != PDF x {}",
            m[4],
            g.x
        );
        assert!(
            (m[5] - (page_height - g.y)).abs() < 0.01,
            "{id}: y {} != PDF y",
            m[5]
        );
        // Upright: font units are y-up, so the y axis is flipped and positive x
        // stays positive.
        assert!(m[0] > 0.0 && m[3] < 0.0, "{id}: glyph is mirrored: {m:?}");
    }

    // The unpainted text layer still carries the text for selection/search.
    assert!(svg.contains("<text"), "the selection text layer is gone");
}

#[test]
fn svg_preview_of_a_parsed_pdf_draws_the_shaped_glyphs() {
    // The demo's "open PDF" path: a saved PDF parsed back. Its text is one
    // merged visual-order TJ run per line; the preview must pen-advance through
    // it with the font's widths rather than hand the run to the browser, which
    // would bidi-reverse visual-order text a second time.
    let doc = render();
    let mut w = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut w);
    let parsed = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut w).expect("parse");
    let page = &parsed.pages[0];
    let page_height = page.media_box.height.0;
    let svg = page.to_svg(
        &parsed.resources,
        &PdfToSvgOptions::default(),
        &mut Vec::new(),
    );

    assert_no_painted_arabic_text(&svg);

    let mut original: Vec<PlacedGlyph> = placed_glyphs(&doc)
        .into_iter()
        .filter(|g| g.text != " ")
        .collect();
    original.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut uses = svg_uses(&svg);
    uses.sort_by(|a, b| a.1[4].total_cmp(&b.1[4]));
    assert_eq!(
        uses.len(),
        original.len(),
        "one painted glyph per non-blank glyph"
    );

    // Same shapes as the in-memory document (the parsed font is the subset, so
    // compare outlines, not ids), at the same place.
    let in_memory =
        doc.pages[0].to_svg(&doc.resources, &PdfToSvgOptions::default(), &mut Vec::new());
    let mut in_memory_uses = svg_uses(&in_memory);
    in_memory_uses.sort_by(|a, b| a.1[4].total_cmp(&b.1[4]));
    for ((id, m), ((mem_id, _), g)) in uses.iter().zip(in_memory_uses.iter().zip(&original)) {
        assert_eq!(
            svg_path_def(&svg, id),
            svg_path_def(&in_memory, mem_id),
            "parsed glyph {id} is not the outline of {mem_id} ({:?})",
            g.text
        );
        assert!(
            (m[4] - g.x).abs() < 0.05,
            "{id} ({:?}): x {} != {}",
            g.text,
            m[4],
            g.x
        );
        assert!(
            (m[5] - (page_height - g.y)).abs() < 0.05,
            "{id}: y {} != {}",
            m[5],
            page_height - g.y
        );
    }
}
