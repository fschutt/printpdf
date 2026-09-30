//! Text color alpha must survive the HTML bridge.
//!
//! The bridge used to emit `SetFillColor` from the RGB channels only, so
//! `color: transparent` painted opaque black text and `rgba(…, 0.5)` painted
//! fully opaque text. OCR text layers (positioned `color: transparent` spans over
//! a page scan) depend on the first case: the text has to stay selectable and
//! searchable without covering the image.
//!
//! Expected ops per text run:
//! - alpha 0   -> `q`, fill color, `3 Tr` (invisible), BT..ET, `Q`
//! - 0 < alpha -> `q`, fill color, ExtGState with the fill alpha, BT..ET, `Q`
//! - opaque    -> unchanged (no scope, no Tr)
//!
//! Inline `background-color` behind text had the same bug and gets the same ExtGState.

#![cfg(feature = "html")]

use std::collections::BTreeMap;

use printpdf::*;

const FONT: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");

const HTML: &str = r#"<html>
<head><style>
    body { font-family: 'Alpha Probe'; font-size: 20px; }
    .page { position: relative; width: 400pt; height: 200pt; }
    .word { position: absolute; color: transparent; }
    .half { position: absolute; color: rgba(255, 0, 0, 0.5); }
    .solid { position: absolute; color: #0000ff; }
    .tint { background-color: rgba(0, 255, 0, 0.25); }
</style></head>
<body><div class="page">
    <span class="word" style="left: 5%; top: 10%;">HIDDENX</span>
    <span class="half" style="left: 5%; top: 40%;">HALFX</span>
    <span class="solid" style="left: 5%; top: 70%;">SOLIDX</span>
</div>
<p>plain <span class="tint">TINTX</span> text</p></body></html>"#;

fn render() -> PdfDocument {
    let mut fonts = BTreeMap::new();
    fonts.insert("Alpha Probe".to_string(), Base64OrRaw::Raw(FONT.to_vec()));
    let mut warnings = Vec::new();
    PdfDocument::from_html(HTML, &BTreeMap::new(), &fonts, &GeneratePdfOptions::default(), &mut warnings)
        .expect("from_html")
}

/// Ops of the q..Q / BT..ET run that shows `needle`, from the op that opened the
/// run's graphics scope (or its SetFillColor when unscoped) up to its ET.
fn run_prefix<'a>(ops: &'a [Op], needle: &str) -> &'a [Op] {
    let marker = ops
        .iter()
        .position(|op| actual_text(op).is_some_and(|t| t == needle))
        .unwrap_or_else(|| panic!("no ActualText span for {needle}"));
    let start = ops[..marker]
        .iter()
        .rposition(|op| matches!(op, Op::EndTextSection | Op::RestoreGraphicsState))
        .map_or(0, |i| i + 1);
    &ops[start..marker]
}

/// Decode the UTF-16BE `/ActualText` of a `BDC /Span` op.
fn actual_text(op: &Op) -> Option<String> {
    let Op::BeginMarkedContentWithProperties { properties: DictItem::Dict { map }, .. } = op else {
        return None;
    };
    let DictItem::String { data, .. } = map.get("ActualText")? else {
        return None;
    };
    let units: Vec<u16> = data[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    String::from_utf16(&units).ok()
}

fn has_invisible_mode(ops: &[Op]) -> bool {
    ops.iter().any(|op| matches!(op, Op::SetTextRenderingMode { mode: TextRenderingMode::Invisible }))
}

fn loaded_gs(ops: &[Op]) -> Option<&ExtendedGraphicsStateId> {
    ops.iter().find_map(|op| match op {
        Op::LoadGraphicsState { gs } => Some(gs),
        _ => None,
    })
}

#[test]
fn transparent_text_is_invisible_but_present() {
    let doc = render();
    let ops = &doc.pages[0].ops;
    let run = run_prefix(ops, "HIDDENX");
    assert!(matches!(run.first(), Some(Op::SaveGraphicsState)), "run must open a q scope: {run:?}");
    assert!(has_invisible_mode(run), "transparent text must use Tr 3: {run:?}");
}

#[test]
fn translucent_text_gets_fill_alpha() {
    let doc = render();
    let ops = &doc.pages[0].ops;
    let run = run_prefix(ops, "HALFX");
    assert!(!has_invisible_mode(run));
    let gs = loaded_gs(run).expect("translucent text must load an ExtGState");
    assert!(doc.resources.extgstates.map.contains_key(gs), "ExtGState must be registered");
}

#[test]
fn opaque_text_is_unchanged() {
    let doc = render();
    let ops = &doc.pages[0].ops;
    let run = run_prefix(ops, "SOLIDX");
    assert!(!has_invisible_mode(run));
    assert!(loaded_gs(run).is_none());
    assert!(!run.iter().any(|op| matches!(op, Op::SaveGraphicsState)));
}

/// Tr is graphics state: it must not leak from the transparent run into the
/// runs after it, and every q opened for a run must be closed.
#[test]
fn scopes_are_balanced() {
    let doc = render();
    let ops = &doc.pages[0].ops;
    let depth = ops.iter().try_fold(0i32, |d, op| {
        let d = match op {
            Op::SaveGraphicsState => d + 1,
            Op::RestoreGraphicsState => d - 1,
            _ => d,
        };
        (d >= 0).then_some(d)
    });
    assert_eq!(depth, Some(0), "unbalanced q/Q");
}

/// End to end: the saved PDF emits `3 Tr` and a ~0.5 fill-alpha ExtGState.
#[test]
fn saved_pdf_contains_invisible_text_and_alpha() {
    let doc = render();
    let mut warnings = Vec::new();
    let opts = PdfSaveOptions { optimize: false, ..Default::default() };
    let bytes = doc.save(&opts, &mut warnings);
    let parsed = lopdf::Document::load_mem(&bytes).expect("valid PDF");
    let page_id = *parsed.get_pages().values().next().expect("one page");
    let content = parsed.get_page_content(page_id);
    let content = String::from_utf8_lossy(&content);
    assert!(content.contains("3 Tr"), "no invisible-text operator in content stream");

    // ExtGStates are serialized inline in the /ExtGState resource dictionary.
    let is_half_alpha = |d: &lopdf::Dictionary| {
        d.get(b"ca").ok().and_then(|ca| ca.as_float().ok()).is_some_and(|ca| (ca - 0.5).abs() < 0.01)
    };
    let half_alpha = parsed.objects.values().filter_map(|obj| obj.as_dict().ok()).any(|d| {
        is_half_alpha(d) || d.iter().any(|(_, v)| v.as_dict().is_ok_and(is_half_alpha))
    });
    assert!(half_alpha, "no ExtGState with /ca 0.5 for the rgba text");
}

/// A translucent inline background (`<span style="background-color: rgba(…)">`)
/// must be filled through an ExtGState in every q scope that paints it — including
/// the glyph-run background pass, which used to paint it opaque.
#[test]
fn translucent_inline_background_gets_fill_alpha() {
    let doc = render();
    let ops = &doc.pages[0].ops;
    let is_tint = |op: &Op| matches!(op, Op::SetFillColor { col: Color::Rgb(c) } if c.r < 0.01 && c.g > 0.99 && c.b < 0.01);
    let tinted_scopes: Vec<&[Op]> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, Op::DrawPolygon { .. }))
        .map(|(i, _)| {
            let start = ops[..i].iter().rposition(|op| matches!(op, Op::SaveGraphicsState)).unwrap_or(0);
            &ops[start..i]
        })
        .filter(|scope| scope.iter().any(is_tint))
        .collect();
    assert!(!tinted_scopes.is_empty(), "no green background fill");
    for scope in tinted_scopes {
        let gs = loaded_gs(scope).unwrap_or_else(|| panic!("opaque background fill: {scope:?}"));
        assert!(doc.resources.extgstates.map.contains_key(gs));
    }
}
