//! hOCR-style text geometry extraction: every text run of a page as
//! line / word / glyph boxes with the decoded text attached.
//!
//! This is the "selection layer" of a PDF viewer built on printpdf: render the
//! page with [`crate::PdfPage::to_svg`], overlay these boxes for hit-testing,
//! selection rectangles and copy-paste. The output mirrors the hOCR hierarchy
//! (`ocr_page` → `ocr_line` → `ocrx_word`) but as plain serde JSON instead of
//! HTML attributes — bboxes are `[x0, y0, x1, y1]` like hOCR's `bbox`.
//!
//! ## Coordinate space
//!
//! Boxes are in **points, top-left origin, y growing downward** — the same
//! space as the SVG produced by `to_svg` — so a viewer can overlay them on the
//! rendered page without further math. (PDF user space is bottom-left-up; the
//! flip `y_svg = page_height - y_pdf` is applied here.)
//!
//! ## Geometry
//!
//! Glyph advances come from the embedded font program (the same source the
//! renderer uses), and the pen math follows ISO 32000-1 §9.4.4:
//!
//! ```text
//! tx = ((w_glyph/1000)·size + Tc + (code==space ? Tw : 0)) · Tz/100
//! ```
//!
//! Character spacing (`Tc`), word spacing (`Tw`), horizontal scaling (`Tz`),
//! rise (`Ts`), the text matrix (`Tm`, including rotation/skew — boxes are the
//! axis-aligned hull of the transformed glyph quad), the graphics CTM (`cm`
//! with save/restore) and leading (`TL`/`T*`) are all folded in.

use std::collections::BTreeMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    ops::{Op, PdfFontHandle},
    text::TextItem,
    BuiltinFont, FontId, ParsedFont, PdfPage, PdfResources,
};

/// `[x0, y0, x1, y1]` in pt, top-left origin (hOCR `bbox` order).
pub type BBox = [f32; 4];

fn bbox_union(a: BBox, b: BBox) -> BBox {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// All text geometry of one page (hOCR `ocr_page`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageTextBoxes {
    /// Page width in pt.
    pub width: f32,
    /// Page height in pt.
    pub height: f32,
    /// Text lines in content-stream order.
    pub lines: Vec<TextLine>,
}

/// One baseline run of text (hOCR `ocr_line`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLine {
    /// Hull of all word boxes, `[x0, y0, x1, y1]` pt, top-left origin.
    pub bbox: BBox,
    /// The line's baseline y (top-left-origin pt) — where the glyphs sit;
    /// `bbox` extends above (ascent) and below (descent) of this.
    pub baseline: f32,
    pub words: Vec<TextWord>,
}

/// One whitespace-delimited word (hOCR `ocrx_word`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextWord {
    /// Hull of the word's glyph boxes.
    pub bbox: BBox,
    /// The decoded text (from ToUnicode / the font's cmap).
    pub text: String,
    /// Font size in pt.
    pub font_size: f32,
    /// The font resource this word was shown with, if an external font.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<FontId>,
    /// Per-glyph boxes, for character-precise selection/hit-testing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<GlyphBox>,
}

/// A single positioned glyph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlyphBox {
    pub bbox: BBox,
    /// The decoded text of this glyph (may be multiple chars for ligatures).
    pub text: String,
}

/// 2x3 matrix `[a b c d e f]`, PDF order.
#[derive(Debug, Clone, Copy)]
struct Mat([f32; 6]);

impl Mat {
    const IDENTITY: Mat = Mat([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// `self ∘ other`: apply `other` first, then `self`.
    fn mul(self, o: Mat) -> Mat {
        let a = self.0;
        let b = o.0;
        Mat([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }

    fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let m = self.0;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }
}

/// Text-space glyph quad -> device AABB (top-left origin).
fn quad_to_bbox(m: &Mat, x0: f32, y0: f32, x1: f32, y1: f32, page_height: f32) -> BBox {
    let corners = [
        m.apply(x0, y0),
        m.apply(x1, y0),
        m.apply(x0, y1),
        m.apply(x1, y1),
    ];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (x, y) in corners {
        // PDF user space (bottom-left up) -> SVG/top-left space.
        let y = page_height - y;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    [min_x, min_y, max_x, max_y]
}

/// Per-glyph metrics source: an external font's parsed face, or a builtin
/// standard-14 face (whose subset program ships with printpdf).
enum FontRef<'a> {
    External(FontId, &'a ParsedFont),
    Builtin(ParsedFont),
    None,
}

impl FontRef<'_> {
    fn parsed(&self) -> Option<&ParsedFont> {
        match self {
            FontRef::External(_, f) => Some(f),
            FontRef::Builtin(f) => Some(f),
            FontRef::None => None,
        }
    }

    fn id(&self) -> Option<FontId> {
        match self {
            FontRef::External(id, _) => Some(id.clone()),
            _ => None,
        }
    }
}

fn advance_em(font: Option<&ParsedFont>, gid: u16) -> f32 {
    font.map(|f| crate::ops::glyph_advance_em(f, gid)).unwrap_or(0.5)
}

/// (ascent_em, descent_em) of the current font; the conventional 0.8/-0.2
/// when no metrics are available.
fn vertical_extent_em(font: Option<&ParsedFont>) -> (f32, f32) {
    let Some(f) = font else { return (0.8, -0.2) };
    #[cfg(feature = "text_layout")]
    {
        let upm = f.pdf_font_metrics.units_per_em as f32;
        if upm > 0.0 {
            let asc = f.font_metrics.ascent as f32 / upm;
            let desc = f.font_metrics.descent as f32 / upm;
            if asc > 0.0 {
                return (asc, desc.min(0.0));
            }
        }
    }
    #[cfg(not(feature = "text_layout"))]
    {
        let upm = f.units_per_em as f32;
        if upm > 0.0 {
            let asc = f.font_metrics.ascent as f32 / upm;
            let desc = f.font_metrics.descent as f32 / upm;
            if asc > 0.0 {
                return (asc, desc.min(0.0));
            }
        }
    }
    (0.8, -0.2)
}

/// The walker's text state (ISO 32000-1, 9.3).
struct TextState {
    font_size: f32,
    char_spacing: f32,      // Tc, pt
    word_spacing: f32,      // Tw, pt
    h_scale: f32,           // Tz, fraction (1.0 = 100%)
    rise: f32,              // Ts, pt
    leading: f32,           // TL, pt
    /// Line matrix translation: where the current LINE starts (text space).
    line_origin: (f32, f32),
    /// Full text matrix from SetTextMatrix, if any (composed on top of
    /// line_origin translation).
    tm: Mat,
    /// Pen x offset from the line origin, along the baseline (text space pt).
    pen_x: f32,
}

impl Default for TextState {
    fn default() -> Self {
        TextState {
            font_size: 12.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scale: 1.0,
            rise: 0.0,
            leading: 0.0,
            line_origin: (0.0, 0.0),
            tm: Mat::IDENTITY,
            pen_x: 0.0,
        }
    }
}

impl TextState {
    /// Device matrix for the current pen position: CTM ∘ Tm ∘ translate(line
    /// origin + pen).
    fn device_matrix(&self, ctm: Mat) -> Mat {
        let translate = Mat([
            1.0,
            0.0,
            0.0,
            1.0,
            self.line_origin.0 + self.pen_x,
            self.line_origin.1 + self.rise,
        ]);
        ctm.mul(self.tm).mul(translate)
    }
}

/// Accumulates glyphs into words into lines.
struct Builder {
    lines: Vec<TextLine>,
    cur_line: Option<TextLine>,
    cur_word: Option<TextWord>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            lines: Vec::new(),
            cur_line: None,
            cur_word: None,
        }
    }

    fn push_glyph(
        &mut self,
        text: &str,
        bbox: BBox,
        baseline_y: f32,
        font_size: f32,
        font: Option<FontId>,
    ) {
        let word = self.cur_word.get_or_insert_with(|| TextWord {
            bbox,
            text: String::new(),
            font_size,
            font: font.clone(),
            glyphs: Vec::new(),
        });
        word.bbox = bbox_union(word.bbox, bbox);
        word.text.push_str(text);
        word.glyphs.push(GlyphBox {
            bbox,
            text: text.to_string(),
        });

        let line = self.cur_line.get_or_insert_with(|| TextLine {
            bbox,
            baseline: baseline_y,
            words: Vec::new(),
        });
        line.bbox = bbox_union(line.bbox, bbox);
    }

    fn end_word(&mut self) {
        if let Some(word) = self.cur_word.take() {
            if !word.text.trim().is_empty() {
                let line = self.cur_line.get_or_insert_with(|| TextLine {
                    bbox: word.bbox,
                    baseline: word.bbox[3],
                    words: Vec::new(),
                });
                line.bbox = bbox_union(line.bbox, word.bbox);
                line.words.push(word);
            }
        }
    }

    fn end_line(&mut self) {
        self.end_word();
        if let Some(line) = self.cur_line.take() {
            if !line.words.is_empty() {
                self.lines.push(line);
            }
        }
    }

    fn finish(mut self) -> Vec<TextLine> {
        self.end_line();
        self.lines
    }
}

impl PdfPage {
    /// Extract every text run of this page as line / word / glyph boxes with
    /// decoded text — the selection layer for a `to_svg`-based viewer. See the
    /// [module docs](crate::text_boxes) for coordinate conventions.
    pub fn extract_text_boxes(&self, resources: &PdfResources) -> PageTextBoxes {
        let page_width = self.media_box.width.0;
        let page_height = self.media_box.height.0;

        let mut builder = Builder::new();
        let mut st = TextState::default();
        let mut ctm = Mat::IDENTITY;
        let mut ctm_stack: Vec<Mat> = Vec::new();
        let mut in_text = false;
        let mut font: FontRef = FontRef::None;
        let mut builtin_cache: BTreeMap<BuiltinFont, ParsedFont> = BTreeMap::new();

        for op in &self.ops {
            match op {
                Op::StartTextSection => {
                    in_text = true;
                    st = TextState {
                        font_size: st.font_size,
                        char_spacing: st.char_spacing,
                        word_spacing: st.word_spacing,
                        h_scale: st.h_scale,
                        leading: st.leading,
                        ..TextState::default()
                    };
                }
                Op::EndTextSection => {
                    in_text = false;
                    builder.end_line();
                }
                Op::SaveGraphicsState => ctm_stack.push(ctm),
                Op::RestoreGraphicsState => {
                    if let Some(m) = ctm_stack.pop() {
                        ctm = m;
                    }
                }
                Op::SetTransformationMatrix { matrix } => {
                    let m = matrix.as_array();
                    ctm = ctm.mul(Mat([
                        m[0],
                        m[1],
                        m[2],
                        m[3],
                        m[4],
                        m[5],
                    ]));
                }
                Op::SetFont { font: handle, size } => {
                    st.font_size = size.0;
                    font = match handle {
                        PdfFontHandle::External(id) => match resources.fonts.map.get(id) {
                            Some(pf) => FontRef::External(id.clone(), &pf.parsed_font),
                            None => FontRef::None,
                        },
                        PdfFontHandle::Builtin(b) => {
                            let parsed = builtin_cache.entry(*b).or_insert_with(|| {
                                let subset = b.get_subset_font();
                                ParsedFont::from_bytes(&subset.bytes, 0, &mut Vec::new())
                                    .expect("builtin font programs always parse")
                            });
                            FontRef::Builtin(parsed.clone())
                        }
                    };
                }
                Op::SetLineHeight { lh } => st.leading = lh.0,
                Op::SetWordSpacing { pt } => st.word_spacing = pt.0,
                Op::SetCharacterSpacing { multiplier } => st.char_spacing = *multiplier,
                Op::SetHorizontalScaling { percent } => st.h_scale = *percent / 100.0,
                Op::SetLineOffset { multiplier } => st.rise = *multiplier,
                Op::SetTextCursor { pos } => {
                    // The parser normalizes Td/TD into absolute cursors.
                    if (pos.y.0 - st.line_origin.1).abs() > f32::EPSILON || st.pen_x != 0.0 {
                        builder.end_line();
                    }
                    st.line_origin = (pos.x.0, pos.y.0);
                    st.pen_x = 0.0;
                }
                Op::MoveTextCursorAndSetLeading { tx, ty } => {
                    builder.end_line();
                    st.leading = -*ty;
                    st.line_origin = (st.line_origin.0 + tx, st.line_origin.1 + ty);
                    st.pen_x = 0.0;
                }
                Op::SetTextMatrix { matrix } => {
                    let m = matrix.as_array();
                    let new = Mat([
                        m[0],
                        m[1],
                        m[2],
                        m[3],
                        m[4],
                        m[5],
                    ]);
                    // A vertical move is a new line; a horizontal one usually a
                    // positioned run on the same baseline.
                    if (new.0[5] - st.tm.0[5]).abs() > 0.5 {
                        builder.end_line();
                    }
                    st.tm = new;
                    st.line_origin = (0.0, 0.0);
                    st.pen_x = 0.0;
                }
                Op::AddLineBreak => {
                    builder.end_line();
                    st.line_origin = (st.line_origin.0, st.line_origin.1 - st.leading);
                    st.pen_x = 0.0;
                }
                Op::ShowText { items } if in_text => {
                    self.show_items(
                        items,
                        &mut st,
                        ctm,
                        &font,
                        &mut builder,
                        page_height,
                    );
                }
                Op::MoveToNextLineShowText { text } if in_text => {
                    builder.end_line();
                    st.line_origin = (st.line_origin.0, st.line_origin.1 - st.leading);
                    st.pen_x = 0.0;
                    let items = [TextItem::Text(text.clone())];
                    self.show_items(&items, &mut st, ctm, &font, &mut builder, page_height);
                }
                Op::SetSpacingMoveAndShowText {
                    word_spacing,
                    char_spacing,
                    text,
                } if in_text => {
                    st.word_spacing = *word_spacing;
                    st.char_spacing = *char_spacing;
                    builder.end_line();
                    st.line_origin = (st.line_origin.0, st.line_origin.1 - st.leading);
                    st.pen_x = 0.0;
                    let items = [TextItem::Text(text.clone())];
                    self.show_items(&items, &mut st, ctm, &font, &mut builder, page_height);
                }
                _ => {}
            }
        }

        PageTextBoxes {
            width: page_width,
            height: page_height,
            lines: builder.finish(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_items(
        &self,
        items: &[TextItem],
        st: &mut TextState,
        ctm: Mat,
        font: &FontRef,
        builder: &mut Builder,
        page_height: f32,
    ) {
        let parsed = font.parsed();
        let font_id = font.id();
        let (asc_em, desc_em) = vertical_extent_em(parsed);

        let mut emit = |st: &mut TextState,
                        builder: &mut Builder,
                        text: &str,
                        adv_em: f32,
                        is_space: bool| {
            // ISO 32000-1 §9.4.4: word spacing applies to single-byte code 32
            // only — for the simple-font path that is the space char; CID
            // glyphs never get Tw here (2-byte codes).
            let adv_pt = (adv_em * st.font_size
                + st.char_spacing
                + if is_space { st.word_spacing } else { 0.0 })
                * st.h_scale;

            if is_space {
                builder.end_word();
                st.pen_x += adv_pt;
                return;
            }

            let m = st.device_matrix(ctm);
            let bbox = quad_to_bbox(
                &m,
                0.0,
                desc_em * st.font_size,
                adv_em * st.font_size * st.h_scale,
                asc_em * st.font_size,
                page_height,
            );
            let (_, baseline_pdf) = m.apply(0.0, 0.0);
            builder.push_glyph(
                text,
                bbox,
                page_height - baseline_pdf,
                st.font_size,
                font_id.clone(),
            );
            st.pen_x += adv_pt;
        };

        for item in items {
            match item {
                TextItem::Text(t) => {
                    for c in t.chars() {
                        let adv = parsed
                            .and_then(|f| {
                                f.lookup_glyph_index(c as u32)
                                    .map(|gid| crate::ops::glyph_advance_em(f, gid))
                            })
                            .unwrap_or(0.5);
                        let mut buf = [0u8; 4];
                        emit(st, builder, c.encode_utf8(&mut buf), adv, c == ' ');
                    }
                }
                TextItem::GlyphIds(glyphs) => {
                    for g in glyphs {
                        if g.offset != 0.0 {
                            // TJ offset: negative moves right by |o|/1000 em.
                            let shift = -g.offset / 1000.0 * st.font_size * st.h_scale;
                            if shift > st.font_size * 0.15 {
                                builder.end_word();
                            }
                            st.pen_x += shift;
                        }
                        let text = g.cid.clone().unwrap_or_else(|| "\u{FFFD}".to_string());
                        let adv = advance_em(parsed, g.gid);
                        let is_space = text == " ";
                        emit(st, builder, &text, adv, is_space);
                    }
                }
                TextItem::Offset(o) => {
                    let shift = -o / 1000.0 * st.font_size * st.h_scale;
                    // A large rightward shift is a word gap (same heuristic as
                    // extract_text: < -100 thousandths).
                    if *o < -100.0 {
                        builder.end_word();
                    }
                    st.pen_x += shift;
                }
            }
        }
    }
}

impl crate::PdfDocument {
    /// [`PdfPage::extract_text_boxes`] for every page.
    pub fn extract_text_boxes(&self) -> Vec<PageTextBoxes> {
        self.pages
            .iter()
            .map(|p| p.extract_text_boxes(&self.resources))
            .collect()
    }
}
