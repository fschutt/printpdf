//! Visual/structural regression tests for issue #220 findings F2b and F5:
//! every glyph id the shaper emitted must survive font subsetting.
//!
//! The bug: the original->subset glyph renumbering was "recovered" by looking each
//! used glyph's *character* up in the subset font's own cmap. Shaper-produced glyphs
//! don't have a usable char->gid cmap entry:
//!
//!   - ligature glyphs (the "fi" in "Configure") resolved to the plain 'f' outline,
//!     so "Configure" rendered "Conf gure" (F5);
//!   - glyphs whose codepoint maps to a *different* default glyph in the font's cmap
//!     (Noto CJK maps ASCII digits to another digit form than the shaper picks for
//!     list markers and page numbers) resolved to nothing, were remapped to gid 0 by
//!     `remap_gid`, and rendered as .notdef boxes: every `<ol>` marker and the
//!     "Page 1" digit showed tofu (F2b).
//!
//! Fixed in `crate::font::subset_font` (input-order renumbering is authoritative for
//! allsorts, both glyf and CFF) + `create_subset_runtime_info` (src/serialize.rs).

#![cfg(feature = "html")]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::{Command, Stdio};

use printpdf::*;

const TTF_PROBE: &[u8] = include_bytes!("../examples/assets/fonts/RobotoMedium.ttf");
const CFF_PROBE: &[u8] = include_bytes!("../examples/assets/fonts/NotoSansJP-Regular.otf");

/// Issue #220's document, reduced to the glyph classes that broke: `<ol>` marker
/// digits + periods, a `<ul>` bullet marker, ligature-bearing body text ("fi" in
/// Configure/filter, "fl" in offline), a body-text digit ("Page 1") and a literal
/// bullet routed through the probe font.
fn probe_html(family: &str) -> String {
    format!(
        r#"<html>
<head><style>body {{ font-family: '{family}'; font-size: 14px; }}</style></head>
<body>
    <p>Configure filter offline • probe</p>
    <ul><li>alpha</li><li>beta</li></ul>
    <ol><li>gamma</li><li>delta</li></ol>
    <div>Page 1</div>
</body></html>"#
    )
}

fn render_probe(family: &str, font_bytes: &[u8]) -> (PdfDocument, Vec<u8>) {
    let mut fonts = BTreeMap::new();
    fonts.insert(family.to_string(), Base64OrRaw::Raw(font_bytes.to_vec()));
    let mut warnings = Vec::new();
    let doc = PdfDocument::from_html(
        &probe_html(family),
        &BTreeMap::new(),
        &fonts,
        &GeneratePdfOptions::default(),
        &mut warnings,
    )
    .expect("from_html");
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    (doc, bytes)
}

/// Every glyph shown with an external font, in paint order:
/// `(font, original glyph id, text run cid)`.
fn external_glyphs(doc: &PdfDocument) -> Vec<(FontId, u16, String)> {
    let mut out = Vec::new();
    for page in &doc.pages {
        let mut cur: Option<FontId> = None;
        for op in &page.ops {
            match op {
                Op::SetFont { font, .. } => {
                    cur = match font {
                        PdfFontHandle::External(id) => Some(id.clone()),
                        PdfFontHandle::Builtin(_) => None,
                    };
                }
                Op::ShowText { items } => {
                    if let Some(fid) = &cur {
                        for item in items {
                            if let TextItem::GlyphIds(gs) = item {
                                for cp in gs {
                                    out.push((
                                        fid.clone(),
                                        cp.gid,
                                        cp.cid.clone().unwrap_or_default(),
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Resolve a reference (or pass through an inline object).
fn deref<'a>(pdf: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj.as_reference() {
        Ok(r) => pdf.get_object(r).expect("dangling reference"),
        Err(_) => obj,
    }
}

/// The embedded font program (FontFile2/3 bytes) of the page-resource font
/// registered under `resource_name` (== the printpdf `FontId.0`).
fn font_program_for_resource(pdf: &lopdf::Document, resource_name: &str) -> Option<Vec<u8>> {
    for (_, page_id) in pdf.get_pages() {
        let Ok((res_dict, res_ids)) = pdf.get_page_resources(page_id) else { continue };
        let mut dicts: Vec<&lopdf::Dictionary> = res_dict.into_iter().collect();
        for rid in res_ids {
            if let Ok(lopdf::Object::Dictionary(d)) = pdf.get_object(rid) {
                dicts.push(d);
            }
        }
        for res in dicts {
            let Ok(fonts) = res.get(b"Font") else { continue };
            let lopdf::Object::Dictionary(fonts) = deref(pdf, fonts) else { continue };
            let Ok(font) = fonts.get(resource_name.as_bytes()) else { continue };
            let lopdf::Object::Dictionary(font) = deref(pdf, font) else { continue };
            // Type0 -> DescendantFonts[0] -> FontDescriptor -> FontFile2/3
            let Ok(desc) = font.get(b"DescendantFonts") else { continue };
            let lopdf::Object::Array(desc) = deref(pdf, desc) else { continue };
            let Some(cid_font) = desc.first() else { continue };
            let lopdf::Object::Dictionary(cid_font) = deref(pdf, cid_font) else { continue };
            let Ok(fd) = cid_font.get(b"FontDescriptor") else { continue };
            let lopdf::Object::Dictionary(fd) = deref(pdf, fd) else { continue };
            for key in [b"FontFile2".as_slice(), b"FontFile3".as_slice()] {
                let Ok(ff) = fd.get(key) else { continue };
                let lopdf::Object::Stream(s) = deref(pdf, ff) else { continue };
                return Some(
                    s.decompressed_content()
                        .unwrap_or_else(|_| s.content.clone()),
                );
            }
        }
    }
    None
}

/// The glyph ids actually painted while `font_resource` is the selected font,
/// in content-stream order, across all pages. `font_resource` is the PDF font
/// resource name (== the printpdf `FontId.0`).
fn shown_gids_for_font(pdf: &lopdf::Document, font_resource: &str) -> Vec<u16> {
    fn collect(obj: &lopdf::Object, out: &mut Vec<u16>) {
        match obj {
            lopdf::Object::String(bytes, lopdf::StringFormat::Hexadecimal) => {
                for ch in bytes.chunks_exact(2) {
                    out.push(u16::from_be_bytes([ch[0], ch[1]]));
                }
            }
            lopdf::Object::Array(items) => {
                for it in items {
                    collect(it, out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for (_, page_id) in pdf.get_pages() {
        let content = pdf
            .get_and_decode_page_content(page_id)
            .expect("page content parses");
        let mut selected = false;
        for op in &content.operations {
            match op.operator.as_str() {
                "Tf" => {
                    selected = op
                        .operands
                        .first()
                        .and_then(|o| o.as_name().ok())
                        .map(|n| n == font_resource.as_bytes())
                        .unwrap_or(false);
                }
                "Tj" | "TJ" => {
                    if selected {
                        for operand in &op.operands {
                            collect(operand, &mut out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn advance(font: &ParsedFont, gid: u16) -> Option<u16> {
    font.get_or_decode_glyph(gid).map(|g| g.horz_advance)
}

/// Core assertion battery, shared by the TrueType and CFF probes.
///
/// * `expect_ligature`: also require a shaper-produced multi-glyph substitution
///   ("fi") whose gid differs from the plain 'f' glyph (F5).
/// * `must_reach_cmap`: characters whose subset-cmap mapping MUST have been
///   verified (they are guaranteed painted by the probe font in this document).
fn assert_shaped_glyphs_survive(
    doc: &PdfDocument,
    pdf_bytes: &[u8],
    original_font: &[u8],
    expect_ligature: bool,
    must_reach_cmap: &[char],
) {
    let glyphs = external_glyphs(doc);
    assert!(!glyphs.is_empty(), "document must paint external-font glyphs");

    // --- the glyph classes from #220 must be present in the ops --------------
    let has = |probe: &str| glyphs.iter().any(|(_, _, cid)| cid == probe);
    assert!(has("."), "list markers must produce '.' glyph runs");
    assert!(has("•"), "a bullet glyph must be painted");
    assert!(
        glyphs
            .iter()
            .any(|(_, _, cid)| cid.len() == 1 && cid.chars().all(|c| c.is_ascii_digit())),
        "digit glyphs (list markers / 'Page 1') must be painted"
    );

    // The probe font is the one that painted the ligature (TTF) or any digit (CFF).
    // (The bullet may be itemized into a fallback font — azul prefers its symbol
    // fallback chain over the requested family for '•' — so it is checked via the
    // generic no-.notdef sweep below, not pinned to the probe font.)
    let probe_font_id = if expect_ligature {
        let (fid, lig_gid, _) = glyphs
            .iter()
            .find(|(_, _, cid)| cid == "fi")
            .expect("the shaper must substitute the fi ligature (F5 precondition)");
        let (_, f_gid, _) = glyphs
            .iter()
            .find(|(f, _, cid)| cid == "f" && f == fid)
            .expect("an unligated 'f' must also be painted ('offline')");
        assert_ne!(
            lig_gid, f_gid,
            "the fi ligature must be its own glyph, not the plain 'f'"
        );
        fid.clone()
    } else {
        glyphs
            .iter()
            .find(|(_, _, cid)| cid.len() == 1 && cid.chars().all(|c| c.is_ascii_digit()))
            .map(|(f, _, _)| f.clone())
            .unwrap()
    };

    let probe_glyphs: Vec<u16> = glyphs
        .iter()
        .filter(|(f, _, _)| *f == probe_font_id)
        .map(|(_, gid, _)| *gid)
        .collect();
    let used_sorted: BTreeSet<u16> = probe_glyphs.iter().copied().collect();
    // allsorts renumbers to the position in the requested id list [0] ++ used
    // gids ascending, so original gid -> 1 + rank.
    let rank = |gid: u16| -> u16 { 1 + used_sorted.iter().position(|g| *g == gid).unwrap() as u16 };

    // --- the embedded subset program of the probe font parses ---------------
    // Resolved through the page resources under the probe font's resource name
    // (matching by face name is ambiguous: the system fallback that paints the
    // bullet may share a name prefix, e.g. Noto*).
    let pdf = lopdf::Document::load_mem(pdf_bytes).expect("output PDF parses");
    let subset_bytes = font_program_for_resource(&pdf, &probe_font_id.0)
        .unwrap_or_else(|| panic!("no embedded font program for {}", probe_font_id.0));
    let subset =
        ParsedFont::from_bytes(&subset_bytes, 0, &mut Vec::new()).expect("subset font parses");
    let original =
        ParsedFont::from_bytes(original_font, 0, &mut Vec::new()).expect("probe font parses");

    // --- subset cmap: marker chars stay reachable at their renumbered gids ---
    // Applicable whenever the original font's cmap gid for the char IS the gid
    // the shaper painted (allsorts keeps exactly those mappings, renumbered).
    // When the shaper picked a different glyph than the cmap default (Noto CJK
    // digit forms — the #220 body font), rendering must not depend on the
    // subset cmap at all; those chars are covered by the stream checks below.
    let mut cmap_checked: Vec<char> = Vec::new();
    for ch in ['1', '.', '•'] {
        let Some(orig_gid) = original.lookup_glyph_index(ch as u32) else {
            continue;
        };
        if !used_sorted.contains(&orig_gid) {
            continue; // painted by a fallback font or as a non-default glyph form
        }
        let gid = subset
            .lookup_glyph_index(ch as u32)
            .unwrap_or_else(|| panic!("subset cmap must map {ch:?}"));
        assert_eq!(
            gid,
            rank(orig_gid),
            "subset cmap must map {ch:?} to its renumbered gid"
        );
        assert_ne!(gid, 0, "subset cmap must not map {ch:?} to .notdef");
        assert!(
            subset.get_or_decode_glyph(gid).is_some(),
            "subset glyph {gid} for {ch:?} must have an outline"
        );
        cmap_checked.push(ch);
    }
    for ch in must_reach_cmap {
        assert!(
            cmap_checked.contains(ch),
            "cmap coverage of {ch:?} must have been verified (checked: {cmap_checked:?})"
        );
    }

    // --- content streams never paint .notdef, for ANY external font ----------
    // (Covers the bullet regardless of which fallback font azul itemized it to.
    // Before the fix, marker digits were painted as gid 0 — the tofu boxes.)
    let mut all_fonts: Vec<FontId> = glyphs.iter().map(|(f, _, _)| f.clone()).collect();
    all_fonts.sort_by(|a, b| a.0.cmp(&b.0));
    all_fonts.dedup();
    for fid in &all_fonts {
        let shown = shown_gids_for_font(&pdf, &fid.0);
        assert!(
            !shown.contains(&0),
            "content stream for font {} must never paint gid 0 (.notdef tofu) — F2b",
            fid.0
        );
    }

    // --- probe font: painted codes are the Identity-H codes of the renumbered
    // glyphs. For glyf and name-keyed CFF that IS the input-order renumbering;
    // for CID-keyed CFF (NotoSansJP) the code is the CID the subset charset
    // assigns to the renumbered glyph — viewers resolve codes through that
    // charset, so emitting raw subset gids showed wrong/missing glyphs in
    // Acrobat and Preview (#280).
    // Before the #220 fix the fi ligature was painted as the plain 'f' id (F5)
    // and unreachable glyphs as 0, so this sequence comparison catches both.
    let shown = shown_gids_for_font(&pdf, &probe_font_id.0);
    assert_eq!(
        shown.len(),
        probe_glyphs.len(),
        "every ops glyph must be painted exactly once in the content stream"
    );
    let subset_gid_to_code = printpdf::font::cff_charset_gid_to_cid_map(&subset_bytes, 0);
    let expected: Vec<u16> = probe_glyphs
        .iter()
        .map(|g| {
            let new_gid = rank(*g);
            subset_gid_to_code
                .as_ref()
                .and_then(|m| m.get(&new_gid).copied())
                .unwrap_or(new_gid)
        })
        .collect();
    assert_eq!(
        shown, expected,
        "content-stream codes must be the Identity-H codes (charset CIDs for CID-keyed \
         CFF) of the input-order renumbered ops glyph ids"
    );

    // --- outline identity: subset glyph k must be the original glyph of rank k.
    // (Advance widths are a robust fingerprint; a wrong remap pairs different
    // outlines and their advances diverge — e.g. 'f' vs the wider fi ligature.)
    for (i, orig_gid) in used_sorted.iter().enumerate() {
        let new_gid = (i + 1) as u16;
        assert_eq!(
            advance(&subset, new_gid),
            advance(&original, *orig_gid),
            "subset gid {new_gid} must carry the outline of original gid {orig_gid}"
        );
    }
}

/// F2b + F5 through a TrueType (glyf) font: Roboto Medium ligates "fi"/"fl" and
/// covers digits, '.' and '•'.
#[test]
fn shaper_glyphs_survive_subsetting_ttf() {
    let (doc, bytes) = render_probe("Subset Probe TTF", TTF_PROBE);
    // '1' (body "Page 1") and '.' (ol markers) are guaranteed probe-font glyphs.
    assert_shaped_glyphs_survive(&doc, &bytes, TTF_PROBE, true, &['1', '.']);
}

/// F2b through a CFF (OpenType/OTTO) font — the class of font issue #220's body
/// text actually fell back to (Noto CJK). NotoSansJP does not ligate Latin "fi",
/// so only the marker/digit battery applies.
#[test]
fn shaper_glyphs_survive_subsetting_cff() {
    let (doc, bytes) = render_probe("Subset Probe CFF", CFF_PROBE);
    assert_shaped_glyphs_survive(&doc, &bytes, CFF_PROBE, false, &['1', '.']);
}

// ---------------------------------------------------------------------------
// Visual verification via poppler (skipped when not installed; CI installs it)
// ---------------------------------------------------------------------------

fn tool_ready(tool: &str) -> bool {
    Command::new(tool)
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

fn run_tool_with_stdin(cmd: &mut Command, input: &[u8]) -> Vec<u8> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn tool");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write pdf to tool");
    let out = child.wait_with_output().expect("tool runs");
    out.stdout
}

/// The text layer of the fixed PDF must contain the ligated words as words —
/// with the ligature glyph missing from the subset, rendering showed
/// "Conf gure"; with a broken ToUnicode/ActualText the extraction would too.
#[test]
fn pdftotext_sees_ligated_words() {
    if !tool_ready("pdftotext") {
        eprintln!("skipping: pdftotext (poppler) not installed");
        return;
    }
    let (_doc, bytes) = render_probe("Subset Probe TTF", TTF_PROBE);
    let txt = run_tool_with_stdin(Command::new("pdftotext").args(["-", "-"]), &bytes);
    let txt = String::from_utf8_lossy(&txt);
    for needle in ["Configure", "filter", "offline", "Page 1"] {
        assert!(
            txt.contains(needle),
            "pdftotext must extract {needle:?}, got:\n{txt}"
        );
    }
}

/// Rendered-pixel smoke test: rasterize the page and require that the glyph
/// coverage (dark pixels) of the marker column area is nonzero — i.e. the list
/// markers draw *something*. A page whose markers are .notdef boxes still draws
/// dark pixels, so the strong guarantees live in the structural tests above;
/// this asserts the page rasterizes at all and text ink exists.
#[test]
fn page_rasterizes_with_text_ink() {
    if !tool_ready("pdftoppm") {
        eprintln!("skipping: pdftoppm (poppler) not installed");
        return;
    }
    let (_doc, bytes) = render_probe("Subset Probe TTF", TTF_PROBE);
    let ppm = run_tool_with_stdin(Command::new("pdftoppm").args(["-r", "60", "-"]), &bytes);
    let (w, h, rgb) = parse_ppm(&ppm);
    assert!(w > 0 && h > 0);
    let dark = rgb
        .chunks_exact(3)
        .filter(|p| p[0] < 120 && p[1] < 120 && p[2] < 120)
        .count();
    assert!(
        dark > 100,
        "rasterized page must contain text ink (found {dark} dark pixels)"
    );
}

/// Minimal P6 (binary) PPM parser: returns (width, height, rgb bytes).
fn parse_ppm(data: &[u8]) -> (usize, usize, Vec<u8>) {
    // Header: "P6\n<w> <h>\n<maxval>\n" then raw RGB triples.
    let mut fields = Vec::new(); // w, h, maxval
    let mut pos = 2; // skip "P6"
    while fields.len() < 3 {
        // skip whitespace + comments
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
    pos += 1; // single whitespace after maxval
    (fields[0], fields[1], data[pos..].to_vec())
}
