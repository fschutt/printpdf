// Measurement harness for the azul-layout/font bugs, driven through printpdf.
// Walks the STRUCTURED page ops and prints one line per positioned text run:
//   Ytop (page-top-down pt)   x   fontId   "text"
// so we can compare against tests/e2e/ground_truth.mjs (browser truth).
//
// Run: cargo run --example repro_invoice_layout --features html
extern crate printpdf;

use printpdf::*;
use std::collections::BTreeMap;

const A4_H_PT: f32 = 297.0 * 2.83465;

// The exact invoice from the demo (script.js), plus a deliberately tall table
// cell (2 lines via <br/>) so table-cell vertical centering is exercised.
const INVOICE: &str = r#"<html>
<head><style>
  body { font-family: Helvetica, sans-serif; color: #222; }
  .head { display: flex; justify-content: space-between; }
  h1 { color: #b7410e; margin: 0 0 4px 0; }
  .muted { color: #666; font-size: 10pt; }
  table { width: 100%; border-collapse: collapse; margin-top: 24px; }
  th { text-align: left; border-bottom: 2px solid #b7410e; padding: 6px 4px; }
  td { border-bottom: 1px solid #ddd; padding: 6px 4px; }
  .total { text-align: right; font-size: 14pt; margin-top: 16px; font-weight: bold; }
</style></head>
<body>
  <div class="head">
    <div><h1>INVOICE #2026-071</h1><p class="muted">Issued 2026-07-17 Due 2026-08-16</p></div>
  </div>
  <p><strong>Billed to</strong><br/>Ferris Crab GmbH<br/>Hafenstrasse 12, 20359 Hamburg</p>
  <table>
    <tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr>
    <tr><td>PDF generation consulting</td><td>12 h</td><td>Tall line one<br/>line two</td><td>1440</td></tr>
    <tr><td>WASM integration</td><td>8 h</td><td>120</td><td>960</td></tr>
  </table>
  <p class="total">Total: 2760</p>
</body></html>"#;

// A block-content table cell (<td><p>...</p></td>) to exercise the block-content
// vertical-align path specifically.
const BLOCK_CELL: &str = r#"<html><head><style>
  table { border-collapse: collapse; }
  td { border: 1px solid #999; padding: 0; vertical-align: middle; }
</style></head><body>
<table>
  <tr>
    <td><p>short</p></td>
    <td><p>line one<br/>line two<br/>line three</p></td>
  </tr>
</table>
</body></html>"#;

// Layout primitives sanity check vs browser: parent-child & sibling margin
// collapse, padding container offset, and text-align. Compare baseline-to-baseline
// gaps (cancels the ascent offset) and x (left) for alignment.
const PRIMITIVES: &str = r#"<html><head><style>
  .box { padding: 20px; }
  .mt { margin-top: 30px; }
  .center { text-align: center; }
  .right { text-align: right; }
</style></head><body>
  <div class="box"><p>PadTopLine</p><p class="mt">MarginTop30</p></div>
  <p class="center">CenteredText</p>
  <p class="right">RightText</p>
</body></html>"#;

// Styled primitives (font-family set → avoids the system-CJK fallback): box model
// (border+padding vertical offset), flex column stacking, inline-block packing.
const PRIM2: &str = r#"<html><head><style>
  body { font-family: Helvetica; }
  .bp { border: 5px solid; padding: 10px; }
  .col { display:flex; flex-direction:column; }
  .ib { display:inline-block; width:80px; }
</style></head><body>
  <div class="bp"><p>BorderPad</p></div>
  <div class="col"><div>ColA</div><div>ColB</div></div>
  <p><span class="ib">IBone</span><span class="ib">IBtwo</span>EndText</p>
</body></html>"#;

// Float text-wrap + position:absolute offsets (both observable, commonly buggy).
const FLOATC: &str = r#"<html><head><style>
  body { font-family: Helvetica; }
  .fl { float: left; width: 100px; height: 40px; }
</style></head><body>
  <div class="fl">FloatBox</div>
  <p>WrapLineOne beside the float here more words padding padding padding padding padding BelowFloatLine after the float clears down below the box area now continuing</p>
</body></html>"#;

const ABSC: &str = r#"<html><head><style>
  body { font-family: Helvetica; }
  .rel { position: relative; height: 100px; }
  .abs { position: absolute; top: 20px; left: 30px; }
</style></head><body>
  <div class="rel"><span class="abs">AbsPositioned</span>NormalFlow</div>
</body></html>"#;

// Explicit line-height: confirm whether `line-height:40px` (≥32.768px absolute)
// overflows the compact-cache i16 (×1000 scale) and wrongly renders as "normal".
// L1->L2 baseline gap should be 40px=30pt; if bug, it collapses to ~normal (~14pt).
const LHTEST: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
  <p style="line-height:40px">Big1<br>Big2</p>
  <p style="line-height:2">Two1<br>Two2</p>
</body></html>"#;

// Blast-radius check for the escaped-margin height collapse: a single <p> in body
// (no float). If body height collapses (~0) here too, the double-subtraction is a
// GENERAL bug; if body correctly contains the <p>, it's float-specific.
const NOFLOATP: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body><p>JustOneParagraphHereWithSomeText</p></body></html>"#;
// Nested block whose child's margins escape, followed by a sibling. If the inner
// div height collapses, "AfterP" overlaps/mis-positions relative to "InnerP".
const NESTEDP: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body><div><p>InnerP</p></div><p>AfterP</p></body></html>"#;

// Lists (marker + indentation) and a table with colspan — common, complex.
const LISTC: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body><ul><li>ItemOne</li><li>ItemTwo</li></ul><ol><li>NumOne</li><li>NumTwo</li></ol></body></html>"#;
const TABLEC: &str = r#"<html><head><style>body{font-family:Helvetica}table{border-collapse:collapse}td,th{border:1px solid;padding:4px;text-align:left}</style></head><body><table><tr><th colspan="2">SpanHeader</th></tr><tr><td>CellA</td><td>CellBwider</td></tr></table></body></html>"#;

// white-space (nowrap must NOT wrap; pre preserves runs) + max-width (box caps at 150px,
// text wraps within). Common, observable.
const WSMAX: &str = r#"<html><head><style>body{font-family:Helvetica}
  .nw{white-space:nowrap}
  .mw{max-width:150px}
</style></head><body>
<p class="nw">NOWRAP alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi</p>
<div class="mw">MAXWIDTH alpha beta gamma delta epsilon zeta eta theta iota kappa</div>
</body></html>"#;

// Nested flex (column>row) and CSS grid (2 cols, gap) — heavily used, complex.
const NESTFLEX: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body><div style="display:flex;flex-direction:column"><div style="display:flex;justify-content:space-between"><span>LeftCol</span><span>RightCol</span></div><div>SecondRow</div></div></body></html>"#;
const GRIDC: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body><div style="display:grid;grid-template-columns:1fr 1fr;gap:10px"><div>GridA</div><div>GridB</div><div>GridC</div><div>GridD</div></div></body></html>"#;

// Percentage width, box-sizing:border-box vs content-box, negative margin.
const PCTBOX: &str = r#"<html><head><style>body{font-family:Helvetica}
  .half{width:50%}
  .bb{width:200px;padding:20px;box-sizing:border-box}
  .cb{width:200px;padding:20px}
  .neg{margin-top:-10px}
</style></head><body>
<div class="half">HalfWidthDiv</div>
<div class="bb">BorderBoxDiv</div>
<div class="cb">ContentBoxDiv</div>
<div style="height:30px">TallDiv</div>
<div class="neg">PulledUpDiv</div>
</body></html>"#;

// rowspan (validates the row half of the span-attr fix), min-width beating width,
// overflow:hidden clipping to an explicit height.
const ROWMIN: &str = r#"<html><head><style>body{font-family:Helvetica}table{border-collapse:collapse}td{border:1px solid;padding:2px}</style></head><body>
<table><tr><td rowspan="2">SpanRows</td><td>R1C2</td></tr><tr><td>R2C2</td></tr></table>
<div style="min-width:300px;width:100px">MinWidthDiv</div>
<div style="overflow:hidden;height:20px;width:100px">Clip this very tall overflowing content down to twenty pixels tall box here now</div>
</body></html>"#;

// text-indent (first line only), letter-spacing, word-spacing, text-align:justify.
const TEXTFX: &str = r#"<html><head><style>body{font-family:Helvetica}
  .ind{text-indent:30px}
  .ls{letter-spacing:5px}
  .ws{word-spacing:25px}
</style></head><body>
<p class="ind">IndentedLine one wraps here to a second line that should not be indented at all when it flows onto the next line below</p>
<p class="ls">ABCDE</p>
<p class="plain">ABCDE</p>
<p class="ws">aa bb cc</p>
</body></html>"#;

// vertical-align super/sub (baseline shift on inline spans) + direction:rtl.
const VALIGN: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<p>baseline<span style="vertical-align:super">SUP</span>middle<span style="vertical-align:sub">SUB</span>tail</p>
<p dir="rtl" style="width:200px">RTLTEXT here</p>
</body></html>"#;

// direction:rtl (right-align), flex-grow (item fills remaining), flex-wrap (items wrap).
const RTLGROW: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<p dir="rtl" style="width:200px">RTLATTR here</p>
<p style="direction:rtl;width:200px">RTLCSS here</p>
<div style="display:flex"><div style="flex-grow:1">Grow1</div><div>FixedR</div></div>
<div style="display:flex;flex-wrap:wrap;width:150px"><div style="width:80px">WA</div><div style="width:80px">WB</div><div style="width:80px">WC</div></div>
</body></html>"#;

// calc() widths (common in real layouts).
const CALCW: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<div style="width:calc(100% - 40px)">CalcMinus</div>
<div style="width:calc(50% + 20px)">CalcPlus</div>
<div style="width:calc(100px + 2em)">CalcEm</div>
</body></html>"#;

// :root selector — font-size:40px on :root inherits to <p>; without :root support
// the rule is dropped and text stays at the 16px default.
const ROOTSEL: &str = r#"<html><head><style>:root{font-size:40px}body{font-family:Helvetica}</style></head><body>
<p>RootFont</p>
</body></html>"#;

// Common-feature probe: flex justify-content center / flex-end / space-between; text-align
// center on a block; white-space:pre preserving runs of spaces.
const PROBE: &str = r#"<html><head><style>body{font-family:Helvetica}
  .fc{display:flex;justify-content:center;width:200px}
  .fe{display:flex;justify-content:flex-end;width:200px}
  .sb{display:flex;justify-content:space-between;width:200px}
  .tc{text-align:center;width:200px}
  .pre{white-space:pre}
</style></head><body>
<div class="fc"><span>CEN</span></div>
<div class="fe"><span>END</span></div>
<div class="sb"><span>L</span><span>R</span></div>
<div class="tc">CenterText</div>
<p class="pre">a    b</p>
</body></html>"#;

// aspect-ratio on a FLEX ITEM: height:40px + aspect-ratio:3/1 → width 120px.
const ARFLEX: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<div style="display:flex"><div style="height:40px;aspect-ratio:3/1">FA</div><div>Sib</div></div>
</body></html>"#;
// aspect-ratio (height from width) + multi-column (column-count).
const ARCOL: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<div style="width:100px;aspect-ratio:2/1">AR</div>
<div style="column-count:2;column-gap:10px;width:200px">alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron</div>
</body></html>"#;

// CSS custom properties (var()) + flex gap.
const VARGAP: &str = r#"<html><head><style>body{font-family:Helvetica}
  :root{--boxw:150px}
  .v{width:var(--boxw)}
</style></head><body>
<div class="v">VarWidth</div>
<div style="display:flex;gap:20px"><div style="width:40px">GA</div><div style="width:40px">GB</div><div style="width:40px">GC</div></div>
</body></html>"#;

// position:absolute anchored via right/bottom (only top/left validated before).
const ABSRB: &str = r#"<html><head><style>body{font-family:Helvetica}</style></head><body>
<div style="position:relative;width:200px;height:100px"><span style="position:absolute;right:10px;bottom:20px">RB</span></div>
</body></html>"#;

fn load_font(name: &str) -> (String, Base64OrRaw) {
    let path = format!("examples/assets/fonts/{name}.ttf");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    (name.to_string(), Base64OrRaw::Raw(bytes))
}

fn load_font_abs(family: &str, path: &str) -> (String, Base64OrRaw) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    (family.to_string(), Base64OrRaw::Raw(bytes))
}

// Variable-font weight check: render the same text at weight 400 and 900 with a
// wght-axis VF. If variable weight works, the 900 run is heavier => different
// (wider) advances, so the measured run widths differ. tuple=None renders both
// identically.
const VF_RH: &str = "../azul/doc/fonts/RedHatDisplay-VariableFont_wght.ttf";

fn vf_doc(weight: u32) -> String {
    format!(
        "<html><body><p style=\"font-family:'Red Hat Display'; font-weight:{weight}\">Weight AVWAVjo</p></body></html>"
    )
}

fn dump_vf() {
    println!("\n########## VF_WEIGHT (RedHatDisplay wght; widths must DIFFER by weight) ##########");
    for weight in [400u32, 700, 900] {
        let mut fonts = BTreeMap::new();
        let (k, v) = load_font_abs("Red Hat Display", VF_RH);
        fonts.insert(k, v);
        let images = BTreeMap::new();
        let options = GeneratePdfOptions::default();
        let mut warnings = Vec::new();
        let doc = PdfDocument::from_html(&vf_doc(weight), &images, &fonts, &options, &mut warnings)
            .unwrap();
        let nfonts = doc.resources.fonts.map.len();
        let width = vf_run_width(&doc);
        println!("  weight {weight}: embedded_faces={nfonts}  run_width={width:.2}");
    }
}

// Width (max x minus min x over all positioned glyphs) of the single text run.
fn vf_run_width(doc: &PdfDocument) -> f32 {
    let page = &doc.pages[0];
    let mut cur = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for op in &page.ops {
        match op {
            Op::SetTextMatrix { matrix } => cur = matrix.as_array(),
            Op::ShowText { .. } => {
                min = min.min(cur[4]);
                max = max.max(cur[4]);
            }
            _ => {}
        }
    }
    if max > min {
        max - min
    } else {
        0.0
    }
}

fn dump(label: &str, html: &str) {
    let mut fonts = BTreeMap::new();
    for f in [
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
    ] {
        let (k, v) = load_font(f);
        fonts.insert(k, v);
    }
    let images = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(html, &images, &fonts, &options, &mut warnings).unwrap();

    println!("\n########## {label} ##########");
    println!("embedded fonts ({}):", doc.resources.fonts.map.len());
    for (id, _f) in &doc.resources.fonts.map {
        println!("    {}", id.0);
    }
    if !warnings.is_empty() {
        println!("warnings: {warnings:?}");
    }

    let page = &doc.pages[0];
    let mut cur_font = String::from("?");
    let mut cur_size = 0.0f32;
    let mut cur = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    // Accumulate consecutive glyphs that share the same baseline Y into one run.
    let mut run_text = String::new();
    let mut run_x0 = 0.0f32;
    let mut run_y = f32::NAN;
    let mut run_font = String::new();

    let flush = |t: &mut String, x0: f32, y: f32, font: &str, size: f32| {
        if !t.is_empty() {
            let ytop = A4_H_PT - y;
            println!(
                "  Ytop={ytop:8.2}  x={x0:7.2}  sz={size:5.1}  {font:>22}  {:?}",
                t
            );
            t.clear();
        }
    };

    for op in &page.ops {
        match op {
            Op::SetFont { font, size } => {
                cur_font = font.get_resource_name();
                cur_size = size.0;
            }
            Op::SetTextMatrix { matrix } => {
                cur = matrix.as_array();
            }
            Op::ShowText { items } => {
                let x = cur[4];
                let y = cur[5];
                // New run when baseline Y changes or font changes.
                if (y - run_y).abs() > 0.01 || run_font != cur_font {
                    flush(&mut run_text, run_x0, run_y, &run_font, cur_size);
                    run_x0 = x;
                    run_y = y;
                    run_font = cur_font.clone();
                }
                for it in items {
                    if let TextItem::GlyphIds(gs) = it {
                        for g in gs {
                            if let Some(c) = &g.cid {
                                run_text.push_str(c);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut run_text, run_x0, run_y, &run_font, cur_size);
}

// Diagnostics for the h1/flex font-size bug: isolate flex vs unit resolution.
const H1_ALONE: &str = r#"<html><body><h1>H1 two-em heading</h1><p class="m" style="font-size:10pt">ten point para</p></body></html>"#;
const H1_IN_FLEX: &str = r#"<html><head><style>.head{display:flex;justify-content:space-between}</style></head>
<body><div class="head"><div><h1>H1 two-em heading</h1><p style="font-size:10pt">ten point para</p></div></div></body></html>"#;
const EM_ALONE: &str = r#"<html><body><div style="font-size:2em">two em div</div><div style="font-size:24px">24px div</div></body></html>"#;
const EM_IN_FLEX: &str = r#"<html><head><style>.f{display:flex}</style></head><body><div class="f"><div style="font-size:2em">two em div</div></div></body></html>"#;

const MINI_FLEX: &str = r#"<html><head><style>.h{display:flex}h1{margin:0 0 4px 0}.m{font-size:10pt}</style></head><body><div class="h"><div><h1>TITLE</h1><p class="m">sub</p></div></div><p>AFTER</p></body></html>"#;
const MINI_FLEX_SB: &str = r#"<html><head><style>.h{display:flex;justify-content:space-between}h1{margin:0 0 4px 0}.m{font-size:10pt}</style></head><body><div class="h"><div><h1>TITLE</h1><p class="m">sub</p></div></div><p>AFTER</p></body></html>"#;
const MINI_BLOCK: &str = r#"<html><head><style>h1{margin:0 0 4px 0}.m{font-size:10pt}</style></head><body><div><div><h1>TITLE</h1><p class="m">sub</p></div></div><p>AFTER</p></body></html>"#;
// Long text in the flex item — if flex measures the item at min-content width the
// long strings wrap and the container over-sizes (the invoice's failure mode).
const MINI_FLEX_LONG: &str = r#"<html><head><style>.h{display:flex;justify-content:space-between}h1{margin:0 0 4px 0}.m{font-size:10pt}</style></head><body><div class="h"><div><h1>INVOICE #2026-071</h1><p class="m">Issued 2026-07-17 Due 2026-08-16</p></div></div><p>AFTER</p></body></html>"#;

fn dump_dl_html(label: &str, html: &str) {
    let mut fonts = BTreeMap::new();
    for f in ["Helvetica", "Helvetica-Bold"] {
        let (k, v) = load_font(f);
        fonts.insert(k, v);
    }
    let images = BTreeMap::new();
    let options = GeneratePdfOptions::default();
    let mut warnings = Vec::new();
    let (_doc, dbg) =
        PdfDocument::from_html_debug(html, &images, &fonts, &options, &mut warnings).unwrap();
    println!("########## {label} DISPLAY LIST ##########");
    if let Some(dl) = dbg.display_list_debug.get(0) {
        for line in dl.lines() {
            let l = line.trim();
            if l.contains("at (") || l.contains("size") || l.contains("Rect") || l.contains("TextLayout") || l.contains("Text ") {
                println!("  {l}");
            }
        }
    }
}

fn main() {
    if std::env::var_os("PRIMDL").is_some() {
        dump_dl_html("PRIMITIVES", PRIMITIVES);
        return;
    }
    if std::env::var_os("FLOATDL").is_some() {
        dump_dl_html("FLOATC", FLOATC);
        return;
    }
    if std::env::var_os("NOFLOATP").is_some() {
        dump_dl_html("NOFLOATP", NOFLOATP);
        dump("NESTEDP", NESTEDP);
        dump_dl_html("NESTEDP", NESTEDP);
        return;
    }
    if std::env::var_os("LISTTBL").is_some() {
        dump("LISTC", LISTC);
        dump("TABLEC", TABLEC);
        dump_dl_html("TABLEC", TABLEC);
        return;
    }
    if std::env::var_os("WSMAX").is_some() {
        dump("WSMAX", WSMAX);
        dump_dl_html("WSMAX", WSMAX);
        return;
    }
    if std::env::var_os("FLEXGRID").is_some() {
        dump("NESTFLEX", NESTFLEX);
        dump_dl_html("NESTFLEX", NESTFLEX);
        dump("GRIDC", GRIDC);
        dump_dl_html("GRIDC", GRIDC);
        return;
    }
    if std::env::var_os("PCTBOX").is_some() {
        dump_dl_html("PCTBOX", PCTBOX);
        return;
    }
    if std::env::var_os("ROWMIN").is_some() {
        dump_dl_html("ROWMIN", ROWMIN);
        return;
    }
    if std::env::var_os("TEXTFX").is_some() {
        dump("TEXTFX", TEXTFX);
        dump_dl_html("TEXTFX", TEXTFX);
        return;
    }
    if std::env::var_os("VALIGN").is_some() {
        dump("VALIGN", VALIGN);
        return;
    }
    if std::env::var_os("RTLGROW").is_some() {
        dump("RTLGROW", RTLGROW);
        dump_dl_html("RTLGROW", RTLGROW);
        return;
    }
    if std::env::var_os("CALCW").is_some() {
        dump_dl_html("CALCW", CALCW);
        return;
    }
    if std::env::var_os("ROOTSEL").is_some() {
        dump("ROOTSEL", ROOTSEL);
        dump_dl_html("ROOTSEL", ROOTSEL);
        return;
    }
    if std::env::var_os("PROBE").is_some() {
        dump("PROBE", PROBE);
        dump_dl_html("PROBE", PROBE);
        return;
    }
    if std::env::var_os("ARCOL").is_some() {
        dump("ARCOL", ARCOL);
        dump_dl_html("ARCOL", ARCOL);
        return;
    }
    if std::env::var_os("VARGAP").is_some() {
        dump_dl_html("VARGAP", VARGAP);
        return;
    }
    if std::env::var_os("ARFLEX").is_some() {
        dump_dl_html("ARFLEX", ARFLEX);
        return;
    }
    if std::env::var_os("ABSRB").is_some() {
        dump("ABSRB", ABSRB);
        dump_dl_html("ABSRB", ABSRB);
        return;
    }
    if std::env::var_os("LSCHECK").is_some() {
        let mut fonts = BTreeMap::new();
        let (k, v) = load_font("Helvetica");
        fonts.insert(k, v);
        for (label, style) in [
            ("plain", ""),
            ("ls5", "letter-spacing:5px"),
            ("ws25", "word-spacing:25px"),
        ] {
            let html = format!(
                "<html><body><p style=\"font-family:Helvetica;{style}\">aa bb cc</p></body></html>"
            );
            let doc = PdfDocument::from_html(
                &html,
                &BTreeMap::new(),
                &fonts,
                &GeneratePdfOptions::default(),
                &mut Vec::new(),
            )
            .unwrap();
            println!("{label}: run_width={:.2}", vf_run_width(&doc));
        }
        return;
    }
    if std::env::var_os("DL").is_some() {
        dump_dl_html("INVOICE", INVOICE);
        dump_dl_html("MINI_FLEX", MINI_FLEX);
        dump_dl_html("MINI_FLEX_SB", MINI_FLEX_SB);
        dump_dl_html("MINI_FLEX_LONG", MINI_FLEX_LONG);
        dump_dl_html("MINI_BLOCK", MINI_BLOCK);
        return;
    }
    if std::env::var_os("ONLY_EM").is_some() {
        dump("EM_ALONE (expect 24pt, 18pt)", EM_ALONE);
        return;
    }
    if std::env::var_os("PRIM").is_some() {
        dump("PRIMITIVES", PRIMITIVES);
        return;
    }
    if std::env::var_os("PRIM2").is_some() {
        dump("PRIM2", PRIM2);
        return;
    }
    if std::env::var_os("FLOATC").is_some() {
        dump("FLOATC", FLOATC);
        dump("ABSC", ABSC);
        return;
    }
    if std::env::var_os("LHTEST").is_some() {
        dump("LHTEST", LHTEST);
        return;
    }
    dump("INVOICE", INVOICE);
    dump("BLOCK_CELL", BLOCK_CELL);
    dump("H1_ALONE (expect h1=24pt, para=10pt)", H1_ALONE);
    dump("H1_IN_FLEX (expect h1=24pt, para=10pt)", H1_IN_FLEX);
    dump("EM_ALONE (expect 24pt, 18pt)", EM_ALONE);
    dump("EM_IN_FLEX (expect 24pt)", EM_IN_FLEX);
    dump_vf();
}
