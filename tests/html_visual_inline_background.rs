//! Visual/structural regression test for issue #220 finding F4:
//! an inline element's `background-color` must paint UNDER its own text.
//!
//! azul's display list sequences a text block as
//!
//!     TextLayout (shaping data) .. Rect(s) (inline backgrounds) .. Text runs
//!
//! Screen renderers paint the trailing `Text` runs, so the background rects land
//! underneath. The PDF bridge draws text from the `TextLayout` item instead (the
//! `Text` runs carry no cid/shaping data) — and used to do so at the
//! `TextLayout`'s own position, i.e. BEFORE the background rects, which then
//! painted over the glyphs: `<span class="highlight">printpdf</span>` showed an
//! opaque yellow box with invisible text.
//!
//! Fixed in `display_list_to_printpdf_ops_with_margins` (src/html/bridge.rs):
//! TextLayout emission is deferred to the position of its `Text` runs.

#![cfg(feature = "html")]

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, Stdio};

use printpdf::*;

const FONT: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

/// #ffffcc, the highlight color from issue #220.
const BG: (f32, f32, f32) = (1.0, 1.0, 0.8);

const HTML: &str = r#"<html>
<head><style>
    body { font-family: 'BG Probe'; font-size: 14px; }
    .highlight { background-color: #ffffcc; padding: 5px; }
</style></head>
<body>
    <p>before <span class="highlight">XMARKX</span> after</p>
</body></html>"#;

fn render() -> (PdfDocument, Vec<u8>) {
    let mut fonts = BTreeMap::new();
    fonts.insert("BG Probe".to_string(), Base64OrRaw::Raw(FONT.to_vec()));
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        HTML,
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("from_html");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    (doc, bytes)
}

fn is_bg_color(col: &Color) -> bool {
    match col {
        Color::Rgb(rgb) => {
            (rgb.r - BG.0).abs() < 0.01 && (rgb.g - BG.1).abs() < 0.01 && (rgb.b - BG.2).abs() < 0.01
        }
        _ => false,
    }
}

/// Structural guard: in the page op stream (which serializes 1:1 into the
/// content stream), every #ffffcc background fill must be emitted BEFORE the
/// BT..ET text section that shows the span's glyphs. Before the fix the two
/// azul `Rect` items for the span were translated AFTER the paragraph's text
/// sections, covering the glyphs.
#[test]
fn inline_background_rects_precede_span_text() {
    let (doc, _) = render();
    let page = doc.pages.first().expect("one page");

    let mut bg_fill_indices = Vec::new(); // DrawPolygon ops filled with #ffffcc
    let mut span_text_start: Option<usize> = None; // StartTextSection of "XMARKX"

    let mut current_fill_is_bg = false;
    let mut section_start: Option<usize> = None;
    let mut section_text = String::new();

    for (idx, op) in page.ops.iter().enumerate() {
        match op {
            Op::SetFillColor { col } => current_fill_is_bg = is_bg_color(col),
            Op::DrawPolygon { .. } => {
                if current_fill_is_bg {
                    bg_fill_indices.push(idx);
                }
            }
            Op::StartTextSection => {
                section_start = Some(idx);
                section_text.clear();
            }
            Op::ShowText { items } => {
                for item in items {
                    if let TextItem::GlyphIds(gs) = item {
                        for cp in gs {
                            if let Some(cid) = &cp.cid {
                                section_text.push_str(cid);
                            }
                        }
                    }
                }
            }
            Op::EndTextSection => {
                if section_text.contains("XMARKX") && span_text_start.is_none() {
                    span_text_start = section_start;
                }
                section_start = None;
            }
            _ => {}
        }
    }

    assert!(
        !bg_fill_indices.is_empty(),
        "the span's background-color must be painted at all"
    );
    let text_at = span_text_start.expect("the span's text must be painted");
    let last_bg = *bg_fill_indices.iter().max().unwrap();
    assert!(
        last_bg < text_at,
        "every inline background fill (last at op {last_bg}) must precede the \
         span's text section (op {text_at}) — background painted OVER its text (F4)"
    );
}

// ---------------------------------------------------------------------------
// Visual verification via poppler (skipped when not installed; CI installs it)
// ---------------------------------------------------------------------------

fn pdftoppm_ready() -> bool {
    Command::new("pdftoppm")
        .arg("-v")
        .output()
        .map(|out| {
            let banner = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            banner.contains("Poppler")
        })
        .unwrap_or(false)
}

/// Pixel probe: rasterize the page, locate the yellow highlight box, and require
/// dark (glyph) pixels inside it. Before the fix the box was uniformly yellow —
/// the text was under the fill.
#[test]
fn highlight_text_is_visible_over_background() {
    if !pdftoppm_ready() {
        eprintln!("skipping: pdftoppm (poppler) not installed");
        return;
    }
    let (_doc, bytes) = render();

    let mut child = Command::new("pdftoppm")
        .args(["-r", "100", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pdftoppm");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&bytes)
        .expect("pipe pdf");
    let out = child.wait_with_output().expect("pdftoppm runs");
    let (w, h, rgb) = parse_ppm(&out.stdout);
    assert!(w > 0 && h > 0, "raster output must be non-empty");

    // Bounding box of yellow-ish pixels (#ffffcc, allow antialiasing slack).
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let p = &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
            if p[0] > 240 && p[1] > 240 && p[2] > 150 && p[2] < 235 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    assert!(
        min_x < max_x && min_y < max_y,
        "the yellow highlight box must be painted"
    );

    // Inside the box INTERIOR there must be glyph ink: dark pixels. Probe with a
    // 20% inset so glyph fragments that poke past the box edge (the broken
    // rendering left exactly such slivers visible) cannot satisfy the check —
    // only text actually drawn on top of the fill can.
    let inset_x = (max_x - min_x) / 5;
    let inset_y = (max_y - min_y) / 5;
    let mut dark = 0usize;
    for y in (min_y + inset_y)..=(max_y - inset_y) {
        for x in (min_x + inset_x)..=(max_x - inset_x) {
            let p = &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
            if p[0] < 120 && p[1] < 120 && p[2] < 120 {
                dark += 1;
            }
        }
    }
    assert!(
        dark > 10,
        "the highlight box interior ({min_x},{min_y})-({max_x},{max_y}) must show its \
         text (found only {dark} dark pixels) — background painted OVER the text (F4)"
    );
}

/// Minimal P6 (binary) PPM parser: returns (width, height, rgb bytes).
fn parse_ppm(data: &[u8]) -> (usize, usize, Vec<u8>) {
    assert!(data.starts_with(b"P6"), "expected binary PPM from pdftoppm");
    let mut fields = Vec::new(); // w, h, maxval
    let mut pos = 2;
    while fields.len() < 3 {
        while pos < data.len() && (data[pos].is_ascii_whitespace() || data[pos] == b'#') {
            if data[pos] == b'#' {
                while pos < data.len() && data[pos] != b'\n' {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
        }
        let start = pos;
        while pos < data.len() && data[pos].is_ascii_digit() {
            pos += 1;
        }
        fields.push(
            std::str::from_utf8(&data[start..pos])
                .unwrap()
                .parse::<usize>()
                .expect("ppm header field"),
        );
    }
    pos += 1; // single whitespace byte after maxval
    (fields[0], fields[1], data[pos..].to_vec())
}
