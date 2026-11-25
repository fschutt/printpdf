//! Shaping one line of text in one font, without HTML.
//!
//! [`shape_line`] turns a string into positioned glyphs. It applies the font's substitutions
//! (joined Arabic forms, ligatures), its kerning and its mark positions, and puts
//! right-to-left runs in visual order. The result is a [`PlacedLine`]: the glyphs as
//! [`PlacedGlyph`]s and the width of the line. [`show_placed_glyphs`](crate::show_placed_glyphs)
//! draws them.
//!
//! The script and the direction are found from the text, as the layout of
//! `PdfDocument::from_html` finds them: the line reads right to
//! left when its first strong letter is Arabic or Hebrew, and every run inside it is shaped in
//! its own direction.
//!
//! This module needs the `text_layout` feature and not the `html` feature.
//!
//! # Examples
//!
//! Measure a line and draw it so that it ends at a given x:
//!
//! ```no_run
//! use printpdf::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let bytes = std::fs::read("font.ttf")?;
//! let font = ParsedFont::from_bytes(&bytes, 0, &mut Vec::new()).ok_or("font does not parse")?;
//! let mut doc = PdfDocument::new("Shaped line");
//! let font_id = doc.add_font(&font);
//!
//! let size = Pt(12.0);
//! let line = shape_line(&font, "Total: 42,00", size)?;
//! let right_edge = Pt(500.0);
//!
//! let mut ops = vec![
//!     Op::StartTextSection,
//!     Op::SetTextMatrix { matrix: TextMatrix::Translate(right_edge - line.width, Pt(700.0)) },
//! ];
//! ops.extend(show_placed_glyphs(&[(font_id, &*font)], size, &line.glyphs, None));
//! ops.push(Op::EndTextSection);
//! # Ok(())
//! # }
//! ```

use std::fmt;

use azul_layout::text3::api::{layout_text, TextLayoutDirection, TextLayoutOptions};

use crate::{ParsedFont, PlacedGlyph, Pt};

/// The longest text [`shape_line`] accepts, in bytes of UTF-8.
pub const MAX_LINE_BYTES: usize = 65_536;

/// The largest font size [`shape_line`] accepts, in pt.
pub const MAX_LINE_FONT_SIZE: f32 = 100_000.0;

/// A shaped line: its glyphs and its width.
///
/// The glyphs are in visual order, from left to right, with their `x` counted from the start of
/// the line. Every glyph has `font` 0, so the font is the only entry of the list given to
/// [`show_placed_glyphs`](crate::show_placed_glyphs).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLine {
    /// The glyphs with kerning and mark offsets applied. A glyph carries the text of its
    /// cluster in `text` and the other glyphs of the cluster carry an empty text, so that the
    /// PDF reads back as the original string.
    pub glyphs: Vec<PlacedGlyph>,
    /// The advance width of the whole line, from its start to where the next text would start.
    /// It includes spaces at either end.
    pub width: Pt,
}

impl PlacedLine {
    /// A line without glyphs and with width 0.
    pub fn empty() -> Self {
        Self { glyphs: Vec::new(), width: Pt(0.0) }
    }

    /// Whether the line has no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// Why [`shape_line`] returned no line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeLineError {
    /// The text is longer than [`MAX_LINE_BYTES`].
    TooLong {
        /// Length of the text in bytes.
        len: usize,
        /// The limit, [`MAX_LINE_BYTES`].
        max: usize,
    },
    /// The text has a line break (`\n`, `\r`, a vertical tab, a form feed, U+0085, U+2028 or
    /// U+2029). Shape each line on its own.
    LineBreak,
    /// The size is not a number from above 0 to [`MAX_LINE_FONT_SIZE`].
    InvalidSize,
    /// The font could not shape the text.
    Shaping,
}

impl fmt::Display for ShapeLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeLineError::TooLong { len, max } => {
                write!(f, "text of {len} bytes is longer than the limit of {max} bytes")
            }
            ShapeLineError::LineBreak => write!(f, "text contains a line break"),
            ShapeLineError::InvalidSize => {
                write!(f, "font size must be above 0 and at most {MAX_LINE_FONT_SIZE} pt")
            }
            ShapeLineError::Shaping => write!(f, "the font could not shape the text"),
        }
    }
}

impl std::error::Error for ShapeLineError {}

/// Shapes `text` as one line in `font` at `size`.
///
/// Returns the glyphs at the positions the font gives them, and the width of the line. An empty
/// string gives an empty line. A tab or another control character has no glyph; the glyphs
/// after it move by its advance. Characters the font does not have are shown with its `.notdef` glyph.
///
/// Every call copies the font's tables. To shape many lines in one font, make a [`LineShaper`]
/// once and call [`LineShaper::shape`].
///
/// # Errors
///
/// - [`ShapeLineError::TooLong`] when `text` is longer than [`MAX_LINE_BYTES`];
/// - [`ShapeLineError::LineBreak`] when `text` has a line break;
/// - [`ShapeLineError::InvalidSize`] when `size` is not above 0, not a number, or larger than
///   [`MAX_LINE_FONT_SIZE`];
/// - [`ShapeLineError::Shaping`] when the font fails to shape the text.
///
/// # Examples
///
/// ```
/// use printpdf::*;
///
/// let bytes = include_bytes!("../tests/assets/fonts/mock/mock_ttf.ttf");
/// let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).unwrap();
///
/// // The mock font has 1000 units per em; "A" is 300 units wide and "B" 350.
/// let line = shape_line(&font, "AB", Pt(10.0)).unwrap();
/// assert_eq!(line.glyphs.len(), 2);
/// assert_eq!(line.glyphs[1].x, 3.0);
/// assert_eq!(line.width, Pt(6.5));
///
/// assert!(shape_line(&font, "", Pt(10.0)).unwrap().is_empty());
/// assert!(shape_line(&font, "A\nB", Pt(10.0)).is_err());
/// ```
pub fn shape_line(font: &ParsedFont, text: &str, size: Pt) -> Result<PlacedLine, ShapeLineError> {
    LineShaper::new(font).shape(text, size)
}

/// A font prepared for shaping many lines.
///
/// [`shape_line`] copies the font on every call; a `LineShaper` copies it once.
///
/// # Examples
///
/// ```
/// use printpdf::*;
///
/// let bytes = include_bytes!("../tests/assets/fonts/mock/mock_ttf.ttf");
/// let font = ParsedFont::from_bytes(bytes, 0, &mut Vec::new()).unwrap();
/// let shaper = LineShaper::new(&font);
/// let widths: Vec<Pt> = ["A", "AB", "ABC"]
///     .iter()
///     .map(|t| shaper.shape(t, Pt(10.0)).unwrap().width)
///     .collect();
/// assert_eq!(widths, [Pt(3.0), Pt(6.5), Pt(10.5)]);
/// ```
#[derive(Debug, Clone)]
pub struct LineShaper {
    font: azul_css::props::basic::FontRef,
}

impl LineShaper {
    /// Prepares `font` for shaping.
    pub fn new(font: &ParsedFont) -> Self {
        Self { font: azul_layout::parsed_font_to_font_ref(font.as_azul().clone()) }
    }

    /// Shapes `text` as one line at `size`. See [`shape_line`].
    ///
    /// # Errors
    ///
    /// The same as [`shape_line`].
    pub fn shape(&self, text: &str, size: Pt) -> Result<PlacedLine, ShapeLineError> {
        if !(size.0 > 0.0 && size.0 <= MAX_LINE_FONT_SIZE) {
            return Err(ShapeLineError::InvalidSize);
        }
        if text.len() > MAX_LINE_BYTES {
            return Err(ShapeLineError::TooLong { len: text.len(), max: MAX_LINE_BYTES });
        }
        if text.chars().any(is_line_break) {
            return Err(ShapeLineError::LineBreak);
        }
        if text.is_empty() {
            return Ok(PlacedLine::empty());
        }

        let options = TextLayoutOptions {
            direction: TextLayoutDirection::Auto,
            ..TextLayoutOptions::create(size.0)
        };
        let layout = layout_text(text, std::slice::from_ref(&self.font), &options)
            .ok_or(ShapeLineError::Shaping)?;
        let lines = layout.lines.as_ref();
        if lines.len() > 1 {
            return Err(ShapeLineError::LineBreak);
        }
        let Some(line) = lines.first() else {
            return Ok(PlacedLine::empty());
        };

        let mut glyphs = Vec::new();
        let mut width = 0.0f32;
        for g in line.glyphs.as_ref() {
            width = width.max(g.x + g.advance);
            let cluster = text
                .get(g.cluster_start as usize..g.cluster_end as usize)
                .unwrap_or("");
            // a tab or another control character moves the pen and draws nothing
            if g.tab || (!cluster.is_empty() && cluster.chars().all(char::is_control)) {
                continue;
            }
            let cluster = if g.first_in_cluster { cluster } else { "" };
            glyphs.push(PlacedGlyph {
                font: 0,
                gid: g.glyph_id,
                x: g.x + g.offset_x,
                y: g.offset_y,
                text: cluster.to_string(),
            });
        }
        let width = if width.is_finite() { width.max(0.0) } else { 0.0 };
        Ok(PlacedLine { glyphs, width: Pt(width) })
    }
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}
