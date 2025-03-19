//! Text shaping and measurement
//!
//! This module provides functionality for measuring and shaping text,
//! allowing for complex text layout with precise positioning, line breaks,
//! and text flow around "holes" (like images or other non-text content).

use std::collections::BTreeMap;

use azul_core::{
    ui_solver::ResolvedTextLayoutOptions,
    window::{LogicalPosition, LogicalRect, LogicalSize},
};
use azul_layout::text::layout::{
    position_words, split_text_into_words, word_positions_to_inline_text_layout,
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

/// A shaped glyph with positioning information
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// The character this glyph represents
    pub char: char,
    /// The glyph identifier in the font
    pub glyph_id: u16,
    /// X position relative to the text origin
    pub x: f32,
    /// Y position relative to the text origin
    pub y: f32,
    /// Width of the glyph in points
    pub width: f32,
    /// Height of the glyph in points
    pub height: f32,
    /// Horizontal advance of the glyph in points
    pub advance: f32,
}

/// A shaped word with positioning information
#[derive(Debug, Clone)]
pub struct ShapedWord {
    /// The text content of the word
    pub text: String,
    /// Shaped glyphs making up the word
    pub glyphs: Vec<ShapedGlyph>,
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
    font: &ParsedFont,
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

    // Shape and scale words
    let scaled_words = words_to_scaled_words(
        &words,
        &font.original_bytes,
        font.original_index as u32,
        font.font_metrics,
        options.font_size.0,
    );

    // Position words
    let word_positions = position_words(&words, &scaled_words, &resolved_options);

    // Create text layout
    let mut inline_text_layout =
        word_positions_to_inline_text_layout(&word_positions, &scaled_words);

    // Apply horizontal alignment if not left-aligned
    match options.align {
        TextAlign::Left => {} // Default
        TextAlign::Center => {
            inline_text_layout.align_children_horizontal(StyleTextAlignmentHorz::Center);
        }
        TextAlign::Right => {
            inline_text_layout.align_children_horizontal(StyleTextAlignmentHorz::Right);
        }
    }

    // Get layouted glyphs with final positioning
    let layouted_glyphs = get_layouted_glyphs(&word_positions, &scaled_words, &inline_text_layout);

    // Create our ShapedText result
    let mut shaped_text = ShapedText {
        lines: Vec::new(),
        width: inline_text_layout.content_size.width,
        height: inline_text_layout.content_size.height,
        position: origin,
    };

    // Group glyphs by line and word for easier processing
    let mut line_word_glyphs: BTreeMap<usize, BTreeMap<usize, Vec<ShapedGlyph>>> = BTreeMap::new();

    // Process each glyph
    for glyph in layouted_glyphs.glyphs {
        let line_index = glyph.line_index;
        let word_index = glyph.word_index;

        let shaped_glyph = ShapedGlyph {
            char: glyph.character,
            glyph_id: glyph.glyph_index,
            x: glyph.offset.x + origin.x.0,
            y: glyph.offset.y + origin.y.0,
            width: glyph.size.width,
            height: glyph.size.height,
            advance: glyph.advance,
        };

        line_word_glyphs
            .entry(line_index)
            .or_default()
            .entry(word_index)
            .or_default()
            .push(shaped_glyph);
    }

    // Process each line
    for (line_idx, word_map) in line_word_glyphs {
        if line_idx >= inline_text_layout.lines.len() {
            continue;
        }

        let line_layout = &inline_text_layout.lines.as_slice()[line_idx];

        let mut shaped_line = ShapedLine {
            words: Vec::new(),
            x: line_layout.bounds.origin.x + origin.x.0,
            y: line_layout.bounds.origin.y + origin.y.0,
            width: line_layout.bounds.size.width,
            height: line_layout.bounds.size.height,
            index: line_idx,
        };

        // Process each word in the line
        for (word_idx, glyphs) in word_map {
            if word_idx >= line_layout.children.len() {
                continue;
            }

            let word_layout = &line_layout.children[word_idx];
            let word_text: String = glyphs.iter().map(|g| g.char).collect();

            let shaped_word = ShapedWord {
                text: word_text,
                glyphs,
                x: word_layout.rect.origin.x + origin.x.0,
                y: word_layout.rect.origin.y + origin.y.0,
                width: word_layout.rect.size.width,
                height: word_layout.rect.size.height,
                index: word_idx,
            };

            shaped_line.words.push(shaped_word);
        }

        // Sort words by their index
        shaped_line.words.sort_by_key(|w| w.index);
        shaped_text.lines.push(shaped_line);
    }

    // Sort lines by their index
    shaped_text.lines.sort_by_key(|l| l.index);

    shaped_text
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

// Add methods to PdfResources
impl PdfResources {
    /// Shape text using a font from the document's resources
    pub fn shape_text(
        &self,
        text: &str,
        font_id: &FontId,
        options: &TextShapingOptions,
        origin: Point,
    ) -> Option<ShapedText> {
        let font = self.fonts.map.get(font_id)?;
        Some(shape_text(text, font, options, origin))
    }

    /// Measure text using a font from the document's resources
    pub fn measure_text(&self, text: &str, font_id: &FontId, font_size: Pt) -> Option<(f32, f32)> {
        let font = self.fonts.map.get(font_id)?;
        Some(measure_text(text, font, font_size))
    }
}

// Add methods to PdfDocument
impl PdfDocument {
    /// Shape text using a font from the document
    pub fn shape_text(
        &self,
        text: &str,
        font_id: &FontId,
        options: &TextShapingOptions,
        origin: Point,
    ) -> Option<ShapedText> {
        self.resources.shape_text(text, font_id, options, origin)
    }

    /// Measure text using a font from the document
    pub fn measure_text(&self, text: &str, font_id: &FontId, font_size: Pt) -> Option<(f32, f32)> {
        self.resources.measure_text(text, font_id, font_size)
    }
}
