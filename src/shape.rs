//! Text shaping and measurement
//!
//! This module provides functionality for measuring and shaping text,
//! allowing for complex text layout with precise positioning, line breaks,
//! and text flow around "holes" (like images or other non-text content).

use std::collections::BTreeMap;

use azul_core::{
    callbacks::InlineText,
    ui_solver::ResolvedTextLayoutOptions,
    window::{LogicalPosition, LogicalRect, LogicalSize},
};
use azul_css::StyleTextAlign;
use azul_layout::text::layout::{
    position_words, shape_words, split_text_into_words, word_positions_to_inline_text_layout,
};

use crate::{FontId, Op, ParsedFont, PdfDocument, PdfResources, Point, Pt, Rect, TextItem};

/// Represents a "hole" in the text layout where text won't flow
#[derive(Debug, Clone)]
pub struct TextHole {
    /// The rectangular area of the hole
    pub rect: Rect,
}

/// Horizontal text alignment options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    /// Left align text (default)
    Left,
    /// Center align text
    Center,
    /// Right align text
    Right,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Left
    }
}

/// Options for text shaping and layout
#[derive(Debug, Clone)]
pub struct TextShapingOptions {
    /// Font size in points
    pub font_size: Pt,
    /// Line height in points (optional, defaults to font's recommended line height)
    pub line_height: Option<Pt>,
    /// Letter spacing multiplier (1.0 = default spacing)
    pub letter_spacing: Option<f32>,
    /// Word spacing multiplier (1.0 = default spacing)
    pub word_spacing: Option<f32>,
    /// Width of tab character in points
    pub tab_width: Option<f32>,
    /// Maximum width of text block (for line wrapping)
    pub max_width: Option<Pt>,
    /// Horizontal text alignment
    pub align: TextAlign,
    /// Rectangular "holes" where text won't flow
    pub holes: Vec<TextHole>,
}

impl Default for TextShapingOptions {
    fn default() -> Self {
        Self {
            font_size: Pt(12.0),
            line_height: None,
            letter_spacing: None,
            word_spacing: None,
            tab_width: None,
            max_width: None,
            align: TextAlign::default(),
            holes: Vec::new(),
        }
    }
}

/// A shaped word with positioning information
#[derive(Debug, Clone)]
pub struct ShapedWord {
    /// The text content of the word
    pub text: String,
    /// X position relative to the text origin
    pub x: f32,
    /// Y position relative to the text origin
    pub y: f32,
    /// Width of the word in points
    pub width: f32,
    /// Height of the word in points
    pub height: f32,
    /// Index of the word within its line
    pub index: usize,
}

/// A line of shaped text
#[derive(Debug, Clone)]
pub struct ShapedLine {
    /// Words making up the line
    pub words: Vec<ShapedWord>,
    /// X position relative to the text origin
    pub x: f32,
    /// Y position relative to the text origin
    pub y: f32,
    /// Width of the line in points
    pub width: f32,
    /// Height of the line in points
    pub height: f32,
    /// Line number (0-based)
    pub index: usize,
}

/// A block of shaped text with full layout information
#[derive(Debug, Clone)]
pub struct ShapedText {
    /// Lines making up the text block
    pub lines: Vec<ShapedLine>,
    /// Total width of the text block in points
    pub width: f32,
    /// Total height of the text block in points
    pub height: f32,
    /// Origin point of the text block
    pub position: Point,
}

/// Extract ShapedText from InlineText
fn extract_shaped_text(
    inline_text: &InlineText,
    origin: Point,
) -> ShapedText {
    let mut shaped_text = ShapedText {
        lines: Vec::new(),
        width: inline_text.content_size.width,
        height: inline_text.content_size.height,
        position: origin,
    };

    // Process each line
    for (line_idx, line) in inline_text.lines.iter().enumerate() {
        let mut shaped_line = ShapedLine {
            words: Vec::new(),
            x: line.bounds.origin.x + origin.x.0,
            y: line.bounds.origin.y + origin.y.0,
            width: line.bounds.size.width,
            height: line.bounds.size.height,
            index: line_idx,
        };

        // Process each word in the line
        for (word_idx, word) in line.words.iter().enumerate() {
            match word {
                azul_core::callbacks::InlineWord::Word(contents) => {
                    // Extract text from glyphs
                    let text: String = contents.glyphs.iter()
                        .filter_map(|g| {
                            g.unicode_codepoint
                                .as_ref()
                                .and_then(|cp| std::char::from_u32(*cp))
                                .map(|c| c.to_string())
                        })
                        .collect();

                    let shaped_word = ShapedWord {
                        text,
                        x: contents.bounds.origin.x + origin.x.0,
                        y: contents.bounds.origin.y + origin.y.0,
                        width: contents.bounds.size.width,
                        height: contents.bounds.size.height,
                        index: word_idx,
                    };

                    shaped_line.words.push(shaped_word);
                },
                // Handle other word types (spaces, tabs, etc.) if needed for PDF generation
                azul_core::callbacks::InlineWord::Space => {
                    // Add a space character as a word
                    shaped_line.words.push(ShapedWord {
                        text: " ".to_string(),
                        x: shaped_line.x + (shaped_line.words.last().map(|w| w.x + w.width).unwrap_or(0.0)),
                        y: shaped_line.y,
                        width: inline_text.font_size_px * 0.25, // Approximate space width
                        height: inline_text.font_size_px,
                        index: word_idx,
                    });
                },
                azul_core::callbacks::InlineWord::Tab => {
                    // Add a tab character as a word
                    shaped_line.words.push(ShapedWord {
                        text: "\t".to_string(),
                        x: shaped_line.x + (shaped_line.words.last().map(|w| w.x + w.width).unwrap_or(0.0)),
                        y: shaped_line.y,
                        width: inline_text.font_size_px * 4.0, // Approximate tab width
                        height: inline_text.font_size_px,
                        index: word_idx,
                    });
                },
                azul_core::callbacks::InlineWord::Return => {
                    // Usually handled by line breaks, but include for completeness
                }
            }
        }

        if !shaped_line.words.is_empty() {
            shaped_text.lines.push(shaped_line);
        }
    }

    shaped_text
}

/// Shape text using the specified font and options
///
/// This function performs full text shaping and layout, including:
/// - Breaking text into words and lines
/// - Positioning glyphs with proper kerning
/// - Handling line breaks and wrapping
/// - Flowing text around "holes"
/// - Aligning text horizontally
///
/// # Arguments
/// * `text` - The text to shape
/// * `font` - The font to use for shaping
/// * `options` - Text shaping and layout options
/// * `origin` - The origin point for the text block
///
/// # Returns
/// A `ShapedText` containing the fully laid out text
pub fn shape_text(
    text: &str,
    font: &crate::font::ParsedFont,
    options: &TextShapingOptions,
    origin: Point,
) -> ShapedText {
    // Convert holes to azul_layout format
    let holes = options
        .holes
        .iter()
        .map(|hole| LogicalRect {
            origin: LogicalPosition {
                x: hole.rect.x.0,
                y: hole.rect.y.0,
            },
            size: LogicalSize {
                width: hole.rect.width.0,
                height: hole.rect.height.0,
            },
        })
        .collect::<Vec<_>>();

    // Create layout options
    let resolved_options = ResolvedTextLayoutOptions {
        font_size_px: options.font_size.0,
        line_height: options.line_height.map(|lh| lh.0).into(),
        letter_spacing: options.letter_spacing.into(),
        word_spacing: options.word_spacing.into(),
        tab_width: options.tab_width.into(),
        max_horizontal_width: options.max_width.map(|w| w.0).into(),
        leading: None.into(),
        holes: holes.into(),
    };

    // Split text into words
    let words = split_text_into_words(text);

    // Use adapter to convert to azul_layout's ParsedFont type
    let azul_font = &convert_to_azul_parsed_font(font);
    
    // Shape words using azul_layout's shaping
    let shaped_words = shape_words(&words, azul_font);

    // Position words
    let word_positions = position_words(&words, &shaped_words, &resolved_options);

    // Create text layout
    let mut inline_text_layout = word_positions_to_inline_text_layout(&word_positions);

    let cs = inline_text_layout.content_size;
    // Apply horizontal alignment if not left-aligned
    match options.align {
        TextAlign::Left => {} // Default
        TextAlign::Center => {
            inline_text_layout.align_children_horizontal(
                &cs,
                StyleTextAlign::Center,
            );
        }
        TextAlign::Right => {
            inline_text_layout
                .align_children_horizontal(&cs, StyleTextAlign::Right);
        }
    }

    // Get inline text with final positioning
    let inline_text = azul_core::app_resources::get_inline_text(
        &words,
        &shaped_words,
        &word_positions,
        &inline_text_layout,
    );
    
    // Extract shaped text from inline text
    extract_shaped_text(&inline_text, origin)
}

/// Convert ShapedText to PDF operations
///
/// This function converts the shaped text to a series of PDF operations
/// that can be added to a page.
///
/// # Arguments
/// * `shaped_text` - The shaped text to convert
/// * `font_id` - The font ID to use for text operations
///
/// # Returns
/// A vector of PDF operations
pub fn shaped_text_to_ops(shaped_text: &ShapedText, font_id: &FontId) -> Vec<Op> {
    let mut ops = Vec::new();

    // Start text section
    ops.push(Op::StartTextSection);

    // For each line in shaped text
    for line in &shaped_text.lines {
        for word in &line.words {
            // Set text cursor to word position
            ops.push(Op::SetTextCursor {
                pos: Point {
                    x: Pt(word.x),
                    y: Pt(word.y),
                },
            });

            // Write the word
            ops.push(Op::SetFontSize {
                size: Pt(word.height),
                font: font_id.clone(),
            });
            ops.push(Op::WriteText {
                items: vec![TextItem::Text(word.text.clone())],
                font: font_id.clone(),
            });
        }
    }

    // End text section
    ops.push(Op::EndTextSection);

    ops
}

/// Measure the width and height of text with the specified font and size
///
/// This is a simpler alternative to full text shaping when you just need
/// dimensions. Note that this measures a single line of text without any
/// line breaking or advanced layout.
///
/// # Arguments
/// * `text` - The text to measure
/// * `font` - The font to use for measurement
/// * `font_size` - The font size in points
///
/// # Returns
/// A tuple of (width, height) in points
pub fn measure_text(text: &str, font: &ParsedFont, font_size: Pt) -> (f32, f32) {
    let mut total_width = 0.0;
    let mut max_height = 0.0_f32;

    for ch in text.chars() {
        if let Some(glyph_id) = font.lookup_glyph_index(ch as u32) {
            if let Some((width, height)) = font.get_glyph_size(glyph_id) {
                // Scale to font size
                let scale_factor = font_size.0 / font.font_metrics.units_per_em as f32;
                total_width += width as f32 * scale_factor;
                let dup = height as f32 * scale_factor as f32;
                max_height = max_height.max(dup);
            }
        }
    }

    (total_width, max_height)
}

fn convert_to_azul_parsed_font(font: &crate::font::ParsedFont) -> azul_layout::text::shaping::ParsedFont {
    azul_layout::text::shaping::ParsedFont {
        font_metrics: convert_font_metrics(&font.font_metrics),
        num_glyphs: font.num_glyphs,
        hmtx_data: font.hmtx_data.clone(),
        hhea_table: font.hhea_table.clone().unwrap_or(allsorts_subset_browser::tables::HheaTable {
            ascender: 0,
            descender: 0,
            line_gap: 0,
            advance_width_max: 0,
            min_left_side_bearing: 0,
            min_right_side_bearing: 0,
            x_max_extent: 0,
            caret_slope_rise: 0,
            caret_slope_run: 0,
            caret_offset: 0,
            num_h_metrics: 0,
        }),
        maxp_table: font.maxp_table.clone().unwrap_or(allsorts_subset_browser::tables::MaxpTable {
            num_glyphs: 0,
            version1_sub_table: None,
        }),
        gsub_cache: font.gsub_cache.clone(),
        gpos_cache: font.gpos_cache.clone(),
        opt_gdef_table: font.opt_gdef_table.clone(),
        glyph_records_decoded: convert_glyph_records(&font.glyph_records_decoded),
        space_width: font.space_width,
        cmap_subtable: font.cmap_subtable.clone(),
    }
}

fn convert_font_metrics(metrics: &crate::font::FontMetrics) -> azul_layout::text::layout::FontMetrics {
    azul_layout::text::layout::FontMetrics {
        units_per_em: metrics.units_per_em,
        font_flags: metrics.font_flags,
        x_min: metrics.x_min,
        y_min: metrics.y_min,
        x_max: metrics.x_max,
        y_max: metrics.y_max,
        ascender: metrics.ascender,
        descender: metrics.descender,
        line_gap: metrics.line_gap,
        advance_width_max: metrics.advance_width_max,
        min_left_side_bearing: metrics.min_left_side_bearing,
        min_right_side_bearing: metrics.min_right_side_bearing,
        x_max_extent: metrics.x_max_extent,
        caret_slope_rise: metrics.caret_slope_rise,
        caret_slope_run: metrics.caret_slope_run,
        caret_offset: metrics.caret_offset,
        num_h_metrics: metrics.num_h_metrics,
        x_avg_char_width: metrics.x_avg_char_width,
        us_weight_class: metrics.us_weight_class,
        us_width_class: metrics.us_width_class,
        fs_type: metrics.fs_type,
        y_subscript_x_size: metrics.y_subscript_x_size,
        y_subscript_y_size: metrics.y_subscript_y_size,
        y_subscript_x_offset: metrics.y_subscript_x_offset,
        y_subscript_y_offset: metrics.y_subscript_y_offset,
        y_superscript_x_size: metrics.y_superscript_x_size,
        y_superscript_y_size: metrics.y_superscript_y_size,
        y_superscript_x_offset: metrics.y_superscript_x_offset,
        y_superscript_y_offset: metrics.y_superscript_y_offset,
        y_strikeout_size: metrics.y_strikeout_size,
        y_strikeout_position: metrics.y_strikeout_position,
        s_family_class: metrics.s_family_class,
        panose: metrics.panose,
        ul_unicode_range1: metrics.ul_unicode_range1,
        ul_unicode_range2: metrics.ul_unicode_range2,
        ul_unicode_range3: metrics.ul_unicode_range3,
        ul_unicode_range4: metrics.ul_unicode_range4,
        ach_vend_id: metrics.ach_vend_id,
        fs_selection: metrics.fs_selection,
        us_first_char_index: metrics.us_first_char_index,
        us_last_char_index: metrics.us_last_char_index,
        s_typo_ascender: metrics.s_typo_ascender.into(),
        s_typo_descender: metrics.s_typo_descender.into(),
        s_typo_line_gap: metrics.s_typo_line_gap.into(),
        us_win_ascent: metrics.us_win_ascent.into(),
        us_win_descent: metrics.us_win_descent.into(),
        ul_code_page_range1: metrics.ul_code_page_range1.into(),
        ul_code_page_range2: metrics.ul_code_page_range2.into(),
        sx_height: metrics.sx_height.into(),
        s_cap_height: metrics.s_cap_height.into(),
        us_default_char: metrics.us_default_char.into(),
        us_break_char: metrics.us_break_char.into(),
        us_max_context: metrics.us_max_context.into(),
        us_lower_optical_point_size: metrics.us_lower_optical_point_size.into(),
        us_upper_optical_point_size: metrics.us_upper_optical_point_size.into(),
    }
}

fn convert_glyph_records(records: &BTreeMap<u16, crate::font::OwnedGlyph>) -> BTreeMap<u16, azul_layout::text::shaping::OwnedGlyph> {
    records.iter().map(|(k, v)| {
        (*k, azul_layout::text::shaping::OwnedGlyph {
            bounding_box: azul_layout::text::shaping::OwnedGlyphBoundingBox {
                max_x: v.bounding_box.max_x,
                max_y: v.bounding_box.max_y,
                min_x: v.bounding_box.min_x,
                min_y: v.bounding_box.min_y,
            },
            horz_advance: v.horz_advance,
            outline: v.outline.as_ref().map(|o| convert_glyph_outline(o)),
        })
    }).collect()
}

fn convert_glyph_outline(outline: &crate::font::GlyphOutline) -> azul_layout::text::shaping::GlyphOutline {
    azul_layout::text::shaping::GlyphOutline {
        operations: convert_glyph_outline_operations(&outline.operations),
    }
}

fn convert_glyph_outline_operations(ops: &[crate::font::GlyphOutlineOperation]) -> azul_layout::text::shaping::GlyphOutlineOperationVec {
    ops.iter().map(to_azul_glyph_outline_operation).collect::<Vec<_>>().into()
}

/// Convert from printpdf GlyphOutlineOperation to azul_layout GlyphOutlineOperation
pub fn to_azul_glyph_outline_operation(
    op: &crate::font::GlyphOutlineOperation
) -> azul_layout::text::shaping::GlyphOutlineOperation {
    use crate::font::GlyphOutlineOperation as PdfOp;
    use azul_layout::text::shaping::GlyphOutlineOperation as AzulOp;
    use azul_layout::text::shaping::{
        OutlineMoveTo, OutlineLineTo, OutlineQuadTo, OutlineCubicTo
    };
    
    match op {
        PdfOp::MoveTo(m) => AzulOp::MoveTo(OutlineMoveTo {
            x: m.x,
            y: m.y,
        }),
        PdfOp::LineTo(l) => AzulOp::LineTo(OutlineLineTo {
            x: l.x,
            y: l.y,
        }),
        PdfOp::QuadraticCurveTo(q) => AzulOp::QuadraticCurveTo(OutlineQuadTo {
            ctrl_1_x: q.ctrl_1_x,
            ctrl_1_y: q.ctrl_1_y,
            end_x: q.end_x,
            end_y: q.end_y,
        }),
        PdfOp::CubicCurveTo(c) => AzulOp::CubicCurveTo(OutlineCubicTo {
            ctrl_1_x: c.ctrl_1_x,
            ctrl_1_y: c.ctrl_1_y,
            ctrl_2_x: c.ctrl_2_x,
            ctrl_2_y: c.ctrl_2_y,
            end_x: c.end_x,
            end_y: c.end_y,
        }),
        PdfOp::ClosePath => AzulOp::ClosePath,
    }
}

/// Convert from azul_layout GlyphOutlineOperation to printpdf GlyphOutlineOperation
pub fn from_azul_glyph_outline_operation(
    op: &azul_layout::text::shaping::GlyphOutlineOperation
) -> crate::font::GlyphOutlineOperation {
    use crate::font::{
        GlyphOutlineOperation as PdfOp,
        OutlineMoveTo, OutlineLineTo, OutlineQuadTo, OutlineCubicTo
    };
    use azul_layout::text::shaping::GlyphOutlineOperation as AzulOp;
    
    match op {
        AzulOp::MoveTo(m) => PdfOp::MoveTo(OutlineMoveTo {
            x: m.x,
            y: m.y,
        }),
        AzulOp::LineTo(l) => PdfOp::LineTo(OutlineLineTo {
            x: l.x,
            y: l.y,
        }),
        AzulOp::QuadraticCurveTo(q) => PdfOp::QuadraticCurveTo(OutlineQuadTo {
            ctrl_1_x: q.ctrl_1_x,
            ctrl_1_y: q.ctrl_1_y,
            end_x: q.end_x,
            end_y: q.end_y,
        }),
        AzulOp::CubicCurveTo(c) => PdfOp::CubicCurveTo(OutlineCubicTo {
            ctrl_1_x: c.ctrl_1_x,
            ctrl_1_y: c.ctrl_1_y,
            ctrl_2_x: c.ctrl_2_x,
            ctrl_2_y: c.ctrl_2_y,
            end_x: c.end_x,
            end_y: c.end_y,
        }),
        AzulOp::ClosePath => PdfOp::ClosePath,
    }
}