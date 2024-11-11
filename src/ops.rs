use crate::{color::Color, graphics::{Line, LineCapStyle, LineDashPattern, LineJoinStyle, Point, Polygon, Rect, TextRenderingMode}, matrix::{CurTransMat, TextMatrix}, units::{Mm, Pt}, BuiltinFont, ExtendedGraphicsStateId, FontId, LayerInternalId, LinkAnnotation, XObjectId, XObjectTransform};
use lopdf::Object as LoObject;

#[derive(Debug, PartialEq, Clone)]
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
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
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
#[derive(Debug, Clone)]
pub enum Op {
    /// Debugging or section marker (arbitrary id can mark a certain point in a stream of operations)
    Marker { id: String },
    /// Starts a layer
    BeginLayer { layer_id: LayerInternalId },
    /// Ends a layer (is inserted if missing at the page end)
    EndLayer { layer_id: LayerInternalId },
    /// Saves the graphics configuration on the stack (line thickness, colors, overprint, etc.)
    SaveGraphicsState,
    /// Pops the last graphics configuration state off the stack
    RestoreGraphicsState,
    /// Loads a specific graphics state (necessary for describing extended graphics)
    LoadGraphicsState { gs: ExtendedGraphicsStateId },
    /// Starts a section of text
    StartTextSection,
    /// Ends a text section (inserted by default at the page end)
    EndTextSection,
    /// Writes text, only valid between `StartTextSection` and `EndTextSection`
    WriteText { text: String, size: Pt, font: FontId },
    /// Writes text using a builtin font.
    WriteTextBuiltinFont { text: String, size: Pt, font: BuiltinFont },
    /// Add text to the file at the current position by specifying font codepoints for an ExternalFont
    /// 
    /// NOTE: the `char` defines which codepoint this value is being mapped to (otherwise the 
    /// user would not be able to copy-paste text from the PDF)
    WriteCodepoints { font: FontId, size: Pt, cp: Vec<(u16, char)> },
    /// Add text to the file at the current position by specifying font codepoints with additional kerning offset
    /// 
    /// NOTE: the `char` defines which codepoint this value is being mapped to (otherwise the 
    /// user would not be able to copy-paste text from the PDF)
    WriteCodepointsWithKerning { font: FontId, size: Pt, cpk: Vec<(i64, u16, char)> },
    /// Adds a line break to the text, depends on the line height
    AddLineBreak,
    /// Sets the line height for the text
    SetLineHeight { lh: Pt },
    /// Sets the word spacing in percent (default: 100.0)
    SetWordSpacing { percent: f32 },
    /// Sets the font size for a given font, only valid between `StartTextSection` and `EndTextSection`
    SetFontSize { size: Pt, font: FontId },
    /// Positions the text cursor in the page from the bottom left corner (can be manipulated further with `SetTextMatrix`)
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
    /// Sets the text rendering mode (fill, stroke, fill-stroke, clip, fill-clip)
    SetTextRenderingMode { mode: TextRenderingMode },
    /// Sets the character spacing (default: 1.0)
    SetCharacterSpacing { multiplier: f32 },
    /// Sets the line offset (default: 1.0)
    SetLineOffset { multiplier: f32 },
    /// Draw a line (colors, dashes configured earlier)
    DrawLine { line: Line },
    /// Draw a polygon 
    DrawPolygon { polygon: Polygon },
    /// Set the transformation matrix for this page. Make sure to save the old graphics state before invoking!
    SetTransformationMatrix { matrix: CurTransMat },
    /// Sets a matrix that only affects subsequent text objects.
    SetTextMatrix { matrix: TextMatrix },
    /// Adds a link annotation (use `PdfDocument::add_link` to register the `LinkAnnotation` on the document)
    LinkAnnotation { link: LinkAnnotation },
    /// Instantiates an XObject with a given transform (if the XObject has a width / height). 
    /// Use `PdfDocument::add_xobject` to register the object and get the ID.
    UseXObject { id: XObjectId, transform: XObjectTransform },
    /// Unknown, custom key / value operation
    Unknown { key: String, value: Vec<LoObject> },
}

impl PartialEq for Op {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Marker { id: l_id }, Self::Marker { id: r_id }) => l_id == r_id,
            (Self::BeginLayer { layer_id: l_layer_id }, Self::BeginLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::EndLayer { layer_id: l_layer_id }, Self::EndLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::LoadGraphicsState { gs: l_gs }, Self::LoadGraphicsState { gs: r_gs }) => l_gs == r_gs,
            (Self::WriteText { text: l_text, size: l_size, font: l_font }, Self::WriteText { text: r_text, size: r_size, font: r_font }) => l_text == r_text && l_size == r_size && l_font == r_font,
            (Self::WriteTextBuiltinFont { text: l_text, size: l_size, font: l_font }, Self::WriteTextBuiltinFont { text: r_text, size: r_size, font: r_font }) => l_text == r_text && l_size == r_size && l_font == r_font,
            (Self::WriteCodepoints { font: l_font, size: l_size, cp: l_cp }, Self::WriteCodepoints { font: r_font, size: r_size, cp: r_cp }) => l_font == r_font && l_size == r_size && l_cp == r_cp,
            (Self::WriteCodepointsWithKerning { font: l_font, size: l_size, cpk: l_cpk }, Self::WriteCodepointsWithKerning { font: r_font, size: r_size, cpk: r_cpk }) => l_font == r_font && l_size == r_size && l_cpk == r_cpk,
            (Self::SetLineHeight { lh: l_lh }, Self::SetLineHeight { lh: r_lh }) => l_lh == r_lh,
            (Self::SetWordSpacing { percent: l_percent }, Self::SetWordSpacing { percent: r_percent }) => l_percent == r_percent,
            (Self::SetFontSize { size: l_size, font: l_font }, Self::SetFontSize { size: r_size, font: r_font }) => l_size == r_size && l_font == r_font,
            (Self::SetTextCursor { pos: l_pos }, Self::SetTextCursor { pos: r_pos }) => l_pos == r_pos,
            (Self::SetFillColor { col: l_col }, Self::SetFillColor { col: r_col }) => l_col == r_col,
            (Self::SetOutlineColor { col: l_col }, Self::SetOutlineColor { col: r_col }) => l_col == r_col,
            (Self::SetOutlineThickness { pt: l_pt }, Self::SetOutlineThickness { pt: r_pt }) => l_pt == r_pt,
            (Self::SetLineDashPattern { dash: l_dash }, Self::SetLineDashPattern { dash: r_dash }) => l_dash == r_dash,
            (Self::SetLineJoinStyle { join: l_join }, Self::SetLineJoinStyle { join: r_join }) => l_join == r_join,
            (Self::SetLineCapStyle { cap: l_cap }, Self::SetLineCapStyle { cap: r_cap }) => l_cap == r_cap,
            (Self::SetTextRenderingMode { mode: l_mode }, Self::SetTextRenderingMode { mode: r_mode }) => l_mode == r_mode,
            (Self::SetCharacterSpacing { multiplier: l_multiplier }, Self::SetCharacterSpacing { multiplier: r_multiplier }) => l_multiplier == r_multiplier,
            (Self::SetLineOffset { multiplier: l_multiplier }, Self::SetLineOffset { multiplier: r_multiplier }) => l_multiplier == r_multiplier,
            (Self::DrawLine { line: l_line }, Self::DrawLine { line: r_line }) => l_line == r_line,
            (Self::DrawPolygon { polygon: l_polygon }, Self::DrawPolygon { polygon: r_polygon }) => l_polygon == r_polygon,
            (Self::SetTransformationMatrix { matrix: l_matrix }, Self::SetTransformationMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::SetTextMatrix { matrix: l_matrix }, Self::SetTextMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::LinkAnnotation { link: l_link }, Self::LinkAnnotation { link: r_link }) => l_link == r_link,
            (Self::UseXObject { id: l_id, transform: l_transform }, Self::UseXObject { id: r_id, transform: r_transform }) => l_id == r_id && l_transform == r_transform,
            (Self::Unknown { key: l_key, value: l_value }, Self::Unknown { key: r_key, value: r_value }) => l_key == r_key && l_value == r_value,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
