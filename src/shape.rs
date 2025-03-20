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
    #[cfg(feature = "text_layout")]
    pub(crate) fn from_inline_text(
        font_id: &FontId,
        inline_text: &azul_core::callbacks::InlineText,
        options: &TextShapingOptions,
    ) -> Self {
        let mut shaped_text = ShapedText {
            font_id: font_id.clone(),
            options: options.clone(),
            lines: Vec::new(),
            width: inline_text.content_size.width,
            height: inline_text.content_size.height,
        };

        // Process each line
        for (line_idx, line) in inline_text.lines.iter().enumerate() {
            let mut shaped_line = ShapedLine {
                words: Vec::new(),
                x: line.bounds.origin.x,
                y: line.bounds.origin.y,
                width: line.bounds.size.width,
                height: line.bounds.size.height,
                index: line_idx,
            };

            // Process each word in the line
            for (word_idx, word) in line.words.iter().enumerate() {
                match word {
                    azul_core::callbacks::InlineWord::Word(contents) => {
                        // Extract text from glyphs
                        let text: String = contents
                            .glyphs
                            .iter()
                            .filter_map(|g| {
                                g.unicode_codepoint
                                    .as_ref()
                                    .and_then(|cp| std::char::from_u32(*cp))
                                    .map(|c| c.to_string())
                            })
                            .collect();

                        let shaped_word = ShapedWord {
                            text,
                            x: contents.bounds.origin.x,
                            y: contents.bounds.origin.y,
                            width: contents.bounds.size.width,
                            height: contents.bounds.size.height,
                            index: word_idx,
                        };

                        shaped_line.words.push(shaped_word);
                    }
                    // Handle other word types (spaces, tabs, etc.) if needed for PDF generation
                    azul_core::callbacks::InlineWord::Space => {
                        // Add a space character as a word
                        shaped_line.words.push(ShapedWord {
                            text: " ".to_string(),
                            x: shaped_line.x
                                + (shaped_line
                                    .words
                                    .last()
                                    .map(|w| w.x + w.width)
                                    .unwrap_or(0.0)),
                            y: shaped_line.y,
                            width: inline_text.font_size_px * 0.25, // Approximate space width
                            height: inline_text.font_size_px,
                            index: word_idx,
                        });
                    }
                    azul_core::callbacks::InlineWord::Tab => {
                        // Add a tab character as a word
                        shaped_line.words.push(ShapedWord {
                            text: "\t".to_string(),
                            x: shaped_line.x
                                + (shaped_line
                                    .words
                                    .last()
                                    .map(|w| w.x + w.width)
                                    .unwrap_or(0.0)),
                            y: shaped_line.y,
                            width: inline_text.font_size_px * 4.0, // Approximate tab width
                            height: inline_text.font_size_px,
                            index: word_idx,
                        });
                    }
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

    pub fn get_ops(&self, origin_TOP_LEFT: Point) -> Vec<Op> {
        let font_id = &self.font_id;
        let line_height = self.options.line_height.unwrap_or(self.options.font_size);

        let mut ops = Vec::new();

        ops.push(Op::SaveGraphicsState);

        // Start text section
        ops.push(Op::StartTextSection);

        // The origin_TOP_LEFT is the top left origin of the entire text block being layouted
        // However, in PDF, the "set text cursor" sets the baseline of the first line...
        ops.push(Op::SetTextCursor {
            pos: Point {
                x: origin_TOP_LEFT.x,
                y: origin_TOP_LEFT.y,
            },
        });

        ops.push(Op::SetFontSize {
            size: self.options.font_size,
            font: font_id.clone(),
        });
        ops.push(Op::SetWordSpacing { pt: Pt(100.0) });
        ops.push(Op::SetLineHeight { lh: line_height });

        for line in &self.lines {
            // ... which is why we simply add a line break here before
            // the first line starts
            ops.push(Op::AddLineBreak);

            // In difference to the text matrix, this will MOVE the text cursor
            if line.x != 0.0 {
                ops.push(Op::MoveTextCursorAndSetLeading {
                    tx: line.x, // relative to text origin
                    ty: 0.0,    // handled by line break already
                });
            }

            let mut lastx = 0.0;
            for word in line.words.iter() {
                if word.text.trim().is_empty() {
                    continue;
                }

                let diff = word.x - lastx;
                if diff.abs() != 0.0 {
                    ops.push(Op::MoveTextCursorAndSetLeading {
                        tx: diff, // relative to text origin
                        ty: 0.0,  // handled by line break already
                    });
                }

                lastx = word.x;

                ops.push(Op::WriteText {
                    items: vec![TextItem::Text(word.text.clone())],
                    font: font_id.clone(),
                });
            }
        }

        // End text section
        ops.push(Op::EndTextSection);

        ops.push(Op::RestoreGraphicsState);

        ops
    }
}
