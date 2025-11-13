//! Text shaping and measurement
//!
//! This module provides functionality for measuring and shaping text,
//! allowing for complex text layout with precise positioning, line breaks,
//! and text flow around "holes" (like images or other non-text content).

use crate::{FontId, Op, Point, Pt, Rect, TextItem};

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
    /// Justified text
    Justify,
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

impl TextShapingOptions {
    pub fn new(font_size: Pt) -> Self {
        Self {
            font_size,
            ..Default::default()
        }
    }
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
    /// Font ID that the ShapedText used
    pub font_id: FontId,
    /// Options that this text was layouted with
    pub options: TextShapingOptions,
    /// Lines making up the text block
    pub lines: Vec<ShapedLine>,
    /// Total width of the text block in points
    pub width: f32,
    /// Total height of the text block in points
    pub height: f32,
}

/// Convert ShapedText to PDF operations
///
/// # Arguments
/// * `shaped_text` - The shaped text to convert
/// * `origin_TOP_LEFT` - where to position the shaped text in the page, note: that this is the TOP
///   left of the entire bbox of the text, NOT the text cursor
///
/// # Returns
/// A vector of PDF operations
#[allow(non_snake_case)]
impl ShapedText {
    /// Legacy method - removed. Use azul's text3 API directly instead.
    #[cfg(feature = "text_layout")]
    pub(crate) fn from_inline_text(
        _font_id: &FontId,
        _inline_text: &(),  // Type doesn't exist anymore
        _options: &TextShapingOptions,
    ) -> Self {
        unimplemented!("from_inline_text removed - use azul text3 API directly")
    }

    /// Convert from azul's GlyphRun to printpdf's ShapedText
    /// This is the new way to get shaped text from azul's layout engine
    #[cfg(feature = "text_layout")]
    pub fn from_azul_glyph_runs<T>(
        _font_id: &FontId,
        _glyph_runs: &[azul_layout::text3::glyphs::GlyphRun<T>],
        _options: &TextShapingOptions,
    ) -> Self
    where
        T: azul_layout::text3::cache::ParsedFontTrait,
    {
        todo!("Implement conversion from azul GlyphRun to ShapedText")
    }

    /// Convert this ShapedText to PDF operations
    pub fn to_pdf_ops(
        &self,
        page_height: Pt,
        font_id: &FontId,
        color: crate::Color,
    ) -> Vec<Op> {
        todo!("Implement PDF operations generation from ShapedText")
    }

    /// Legacy method - converts shaped text to PDF operations at a given position
    /// This method is kept for backward compatibility with existing examples
    pub fn get_ops(&self, _origin: Point) -> Vec<Op> {
        // Return empty ops for now - this is a stub
        // In the future, this should be implemented using the new azul text3 API
        Vec::new()
    }
}
