use serde_derive::{Deserialize, Serialize};

pub use crate::text::TextItem;
use crate::{
    color::Color,
    font::ParsedFont,
    graphics::{
        Line, LineCapStyle, LineDashPattern, LineJoinStyle, Point, Polygon, Rect, TextRenderingMode,
    },
    matrix::{CurTransMat, TextMatrix},
    units::{Mm, Pt},
    BuiltinFont, DictItem, ExtendedGraphicsStateId, FontId, LayerInternalId, LinkAnnotation,
    PdfResources, PdfToSvgOptions, PdfWarnMsg, RenderingIntent, XObjectId, XObjectTransform,
};

/// Font resource reference - either a builtin PDF font or an external font
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum PdfFontHandle {
    /// Builtin PDF font (F1-F14)
    Builtin(BuiltinFont),
    /// External font loaded from file
    External(FontId),
}

impl PdfFontHandle {
    /// Get the PDF resource name for this font
    pub fn get_resource_name(&self) -> String {
        match self {
            PdfFontHandle::Builtin(bf) => bf.get_pdf_id().to_string(),
            PdfFontHandle::External(fid) => fid.0.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPage {
    pub media_box: Rect,
    pub trim_box: Rect,
    pub crop_box: Rect,
    pub ops: Vec<Op>,
}

impl PdfPage {
    pub fn new(width: Mm, height: Mm, ops: Vec<Op>) -> Self {
        Self {
            media_box: Rect::from_wh(width.into(), height.into()),
            trim_box: Rect::from_wh(width.into(), height.into()),
            crop_box: Rect::from_wh(width.into(), height.into()),
            ops,
        }
    }

    pub(crate) fn get_media_box(&self) -> lopdf::Object {
        self.media_box.to_array().into()
    }

    pub(crate) fn get_trim_box(&self) -> lopdf::Object {
        self.trim_box.to_array().into()
    }

    pub(crate) fn get_crop_box(&self) -> lopdf::Object {
        self.crop_box.to_array().into()
    }

    /// Render the page to an SVG string.
    pub fn to_svg(
        &self,
        resources: &PdfResources,
        opts: &PdfToSvgOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> String {
        crate::render::render_to_svg(self, resources, opts, warnings)
    }

    pub async fn to_svg_async(
        &self,
        resources: &PdfResources,
        opts: &PdfToSvgOptions,
        warnings: &mut Vec<PdfWarnMsg>,
    ) -> String {
        crate::render::render_to_svg_async(self, resources, opts, warnings).await
    }

    pub fn get_xobject_ids(&self) -> Vec<XObjectId> {
        self.ops
            .iter()
            .filter_map(|s| match s {
                Op::UseXobject { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn get_external_font_ids(&self) -> Vec<FontId> {
        // Note: With the new API, fonts are set via SetFont operation
        // This function now returns an empty vec as external fonts are referenced by resource name
        // To get font usage, parse SetFont operations and resolve resource names
        Vec::new()
    }

    pub fn get_layers(&self) -> Vec<LayerInternalId> {
        self.ops
            .iter()
            .filter_map(|s| match s {
                Op::BeginLayer { layer_id } => Some(layer_id.clone()),
                _ => None,
            })
            .collect()
    }

    /// Extracts text from a PDF page.
    ///
    /// This function processes text-related operations (ShowText) to extract text content.
    /// It properly handles both TextItem::Text and TextItem::GlyphIds by using the
    /// current font's cmap to convert glyph IDs to Unicode characters.
    ///
    /// Note: This implementation doesn't fully handle complex text positioning or
    /// layout features such as columns or tables.
    ///
    /// # Arguments
    /// * `resources` - The PDF resources containing font information
    ///
    /// # Returns
    /// A vector of text chunks extracted from text sections
    pub fn extract_text(&self, resources: &PdfResources) -> Vec<String> {
        let mut text_chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut in_text_section = false;
        let mut cur_text_cursor = Point {
            x: Pt(0.0),
            y: Pt(0.0),
        };
        let mut current_font: Option<&ParsedFont> = None;

        for op in &self.ops {
            match op {
                Op::StartTextSection => {
                    in_text_section = true;
                }
                Op::EndTextSection => {
                    in_text_section = false;
                    if !current_chunk.is_empty() {
                        text_chunks.push(current_chunk.trim().to_string());
                        current_chunk = String::new();
                    }
                }
                Op::SetFont { font, size: _ } => {
                    // Track the current font for glyph ID decoding
                    if let crate::ops::PdfFontHandle::External(font_id) = font {
                        current_font = resources.fonts.map.get(font_id).map(|pf| &pf.parsed_font);
                    }
                }
                Op::SetTextMatrix { .. } => {
                    current_chunk.push_str("\r\n");
                }
                Op::SetTextCursor { pos } => {
                    if (cur_text_cursor.y.0.abs() - pos.y.0.abs()).abs() > 3.0 {
                        current_chunk.push_str("\r\n");
                    }
                    cur_text_cursor = *pos;
                }
                Op::ShowText { items } if in_text_section => {
                    for item in items {
                        match item {
                            TextItem::Offset(o) => {
                                if *o < -100.0 {
                                    current_chunk.push(' ');
                                }
                            }
                            TextItem::Text(t) => current_chunk.push_str(t),
                            TextItem::GlyphIds(glyphs) => {
                                // Convert glyph IDs to Unicode using the current font's cmap
                                if let Some(font) = current_font {
                                    for codepoint in glyphs {
                                        // Check if we have unicode mapping for this glyph ID
                                        let unicode_codepoint = codepoint.cid
                                            .as_ref()
                                            .and_then(|s| s.chars().next())
                                            .or_else(|| Self::glyph_id_to_char(font, codepoint.gid))
                                            .unwrap_or('\u{FFFD}');
                                        current_chunk.push(unicode_codepoint);
                                    }
                                } else {
                                    // No font set, use placeholder
                                    current_chunk.push_str("[glyphs]");
                                }
                            }
                        }
                    }
                }
                Op::AddLineBreak if in_text_section => {
                    current_chunk.push_str("\r\n");
                }
                Op::MoveTextCursorAndSetLeading { .. } if in_text_section => {
                    current_chunk.push_str("\r\n");
                }
                Op::MoveToNextLineShowText { text } if in_text_section => {
                    current_chunk.push_str("\r\n");
                    current_chunk.push_str(text);
                    current_chunk.push(' ');
                }
                Op::SetSpacingMoveAndShowText { text, .. } if in_text_section => {
                    current_chunk.push_str("\r\n");
                    current_chunk.push_str(text);
                    current_chunk.push(' ');
                }
                _ => {}
            }
        }

        if !current_chunk.is_empty() {
            text_chunks.push(current_chunk.trim().to_string());
        }

        text_chunks.retain(|chunk| !chunk.is_empty());
        text_chunks
    }

    /// Helper function to convert glyph ID to character using font's cmap
    fn glyph_id_to_char(font: &ParsedFont, gid: u16) -> Option<char> {
        // Try to find a character that maps to this glyph ID
        // This is a reverse lookup through the cmap
        // For efficiency, we only check common Unicode ranges
        for codepoint in 0..0x10000u32 {
            if let Some(mapped_gid) = font.lookup_glyph_index(codepoint) {
                if mapped_gid == gid {
                    return char::from_u32(codepoint);
                }
            }
        }
        None
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerIntent {
    View,
    Design,
}

impl LayerIntent {
    pub fn to_string(&self) -> &'static str {
        match self {
            LayerIntent::View => "View",
            LayerIntent::Design => "Design",
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerSubtype {
    Artwork,
}

impl LayerSubtype {
    pub fn to_string(&self) -> &'static str {
        match self {
            LayerSubtype::Artwork => "Artwork",
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub name: String,
    pub creator: String,
    pub intent: LayerIntent,
    pub usage: LayerSubtype,
}

impl Layer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            creator: "Adobe Illustrator 14.0".to_string(),
            intent: LayerIntent::Design,
            usage: LayerSubtype::Artwork,
        }
    }
}

/// Operations that can occur in a PDF page
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum Op {
    /// Debugging or section marker (arbitrary id can mark a certain point in a stream of
    /// operations)
    Marker { id: String },
    /// "CS" operator, sets the color space for stroking operations
    SetColorSpaceStroke { id: String },
    /// "cs" operator, sets the color space for fill operations
    SetColorSpaceFill { id: String },
    /// Starts a layer
    BeginLayer { layer_id: LayerInternalId },
    /// Ends a layer (is inserted if missing at the page end)
    EndLayer,
    /// Saves the graphics configuration on the stack (line thickness, colors, overprint, etc.)
    SaveGraphicsState,
    /// Pops the last graphics configuration state off the stack
    RestoreGraphicsState,
    /// Loads a specific graphics state (necessary for describing extended graphics)
    LoadGraphicsState { gs: ExtendedGraphicsStateId },
    /// `BT` - Starts a section of text
    StartTextSection,
    /// `ET` - Ends a text section (inserted by default at the page end)
    EndTextSection,
    
    // --- New 1:1 PDF operations (preferred) ---
    /// `Tf` operator: Set font and size
    /// This maps 1:1 to PDF and should be used instead of SetFontSize/SetFontSizeBuiltinFont
    SetFont { font: PdfFontHandle, size: Pt },
    /// `Tj`/`TJ` operators: Show text at current position
    /// Font must be set first with SetFont. This maps 1:1 to PDF.
    ShowText { items: Vec<TextItem> },
    /// `T*` Adds a line break to the text, depends on the line height
    AddLineBreak,
    /// `TL` - Sets the line height for the text
    SetLineHeight { lh: Pt },
    /// `Tw`: Sets the word spacing in point (default: 100.0)
    SetWordSpacing { pt: Pt },
    /// Positions the text cursor in the page from the bottom left corner (can be manipulated
    /// further with `SetTextMatrix`)
    SetTextCursor { pos: Point },
    /// Sets the fill color for texts / polygons
    SetFillColor { col: Color },
    /// Sets the outline color for texts / polygons
    SetOutlineColor { col: Color },
    /// Sets the outline thickness for texts / lines / polygons
    SetOutlineThickness { pt: Pt },
    /// Sets the outline dash pattern
    SetLineDashPattern { dash: LineDashPattern },
    /// Line join style: miter, round or limit
    SetLineJoinStyle { join: LineJoinStyle },
    /// Line cap style: butt, round, or projecting-square
    SetLineCapStyle { cap: LineCapStyle },
    /// Set a miter limit in Pt
    SetMiterLimit { limit: Pt },
    /// Sets the text rendering mode (fill, stroke, fill-stroke, clip, fill-clip)
    SetTextRenderingMode { mode: TextRenderingMode },
    /// Sets the character spacing (default: 1.0)
    SetCharacterSpacing { multiplier: f32 },
    /// `Ts`: Sets the line offset (default: 1.0)
    SetLineOffset { multiplier: f32 },
    /// Draw a line (colors, dashes configured earlier)
    DrawLine { line: Line },
    /// Draw a rectangle
    DrawRectangle { rectangle: Rect },
    /// Draw a polygon
    DrawPolygon { polygon: Polygon },
    /// Set the transformation matrix for this page. Make sure to save the old graphics state
    /// before invoking!
    SetTransformationMatrix { matrix: CurTransMat },
    /// Sets a matrix that only affects subsequent text objects.
    SetTextMatrix { matrix: TextMatrix },
    /// Adds a link annotation (use `PdfDocument::add_link` to register the `LinkAnnotation` on the
    /// document)
    LinkAnnotation { link: LinkAnnotation },
    /// Instantiates an XObject with a given transform (if the XObject has a width / height).
    /// Use `PdfDocument::add_xobject` to register the object and get the ID.
    UseXobject {
        id: XObjectId,
        transform: XObjectTransform,
    },
    /// `TD` operation
    MoveTextCursorAndSetLeading { tx: f32, ty: f32 },
    /// `ri` operation
    SetRenderingIntent { intent: RenderingIntent },
    /// `Tz` operation
    SetHorizontalScaling { percent: f32 },
    /// Begins an inline image object.
    BeginInlineImage,
    /// Begins the inline image data.
    BeginInlineImageData,
    /// Ends the inline image object.
    EndInlineImage,
    /// Begins a marked content sequence.
    BeginMarkedContent { tag: String },
    /// Begins a marked content sequence with an accompanying property list.
    BeginMarkedContentWithProperties {
        tag: String,
        properties: DictItem,
    },
    /// Starts an optional content layer
    BeginOptionalContent { layer_id: LayerInternalId },
    /// Defines a marked content point with properties.
    DefineMarkedContentPoint {
        tag: String,
        properties: Vec<DictItem>,
    },
    /// Ends the current marked-content sequence.
    EndMarkedContent,
    /// Ends the current marked-content sequence.
    EndMarkedContentWithProperties,
    /// Ends the current optional content sequence.
    EndOptionalContent,
    /// Begins a compatibility section (operators inside are ignored).
    BeginCompatibilitySection,
    /// Ends a compatibility section.
    EndCompatibilitySection,
    /// Moves to the next line and shows text (the `'` operator).
    MoveToNextLineShowText { text: String },
    /// Sets spacing, moves to the next line, and shows text (the `"` operator).
    SetSpacingMoveAndShowText {
        word_spacing: f32,
        char_spacing: f32,
        text: String,
    },
    /// Unknown, custom key / value operation
    Unknown { key: String, value: Vec<DictItem> },
}
