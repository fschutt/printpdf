use crate::{color::Color, graphics::{Line, Point, Polygon, Rect, LineDashPattern, LineJoinStyle, TextRenderingMode, LineCapStyle}, matrix::{CurTransMat, TextMatrix}, units::{Mm, Pt}, ExtendedGraphicsStateId, FontId, LayerInternalId, LinkAnnotId, PageAnnotId, XObjectId, XObjectTransform};

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
}

#[derive(Debug, PartialEq, Clone)]
pub enum LayerIntent {
    View,
    Design,
}

#[derive(Debug, PartialEq, Clone)]
pub enum LayerSubtype {
    Artwork,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Layer {
    pub name: String,
    pub creator: String, 
    pub intent: LayerIntent,
    pub usage: LayerSubtype,
}

/// Operations that can occur in a PDF page
#[derive(Debug, Clone)]
pub enum Op {
    BeginLayer { layer_id: LayerInternalId },
    EndLayer { layer_id: LayerInternalId },

    SaveGraphicsState,
    RestoreGraphicsState,
    LoadGraphicsState { gs: ExtendedGraphicsStateId },

    StartTextSection,
    EndTextSection,
    WriteText { text: String, font: FontId },
    AddLineBreak,
    SetLineHeight { lh: Pt },
    SetWordSpacing { percent: f32 },

    SetFont { font: FontId, size: Pt },
    SetTextCursor { pos: Point },
    SetFillColor { col: Color },
    SetOutlineColor { col: Color },
    SetOutlineThickness { pt: Pt },
    SetLineDashPattern { dash: LineDashPattern },
    SetLineJoinStyle { join: LineJoinStyle },
    SetLineCapStyle { cap: LineCapStyle },
    SetTextRenderingMode { mode: TextRenderingMode },
    SetCharacterSpacing { multiplier: f32 },
    SetLineOffset { multiplier: f32 },

    DrawLine { line: Line },
    DrawPolygon { polygon: Polygon },
    SetTransformationMatrix { matrix: CurTransMat },
    SetTextMatrix { matrix: TextMatrix },

    LinkAnnotation { link_ref: LinkAnnotId },
    UseXObject { id: XObjectId, transform: XObjectTransform },
    Unknown { key: String, value: lopdf::content::Operation },
}

impl PartialEq for Op {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BeginLayer { layer_id: l_layer_id }, Self::BeginLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::EndLayer { layer_id: l_layer_id }, Self::EndLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::LoadGraphicsState { gs: l_id }, Self::LoadGraphicsState { gs: r_id }) => l_id == r_id,
            (Self::WriteText { text: l_text, font: l_font }, Self::WriteText { text: r_text, font: r_font }) => l_text == r_text && l_font == r_font,
            (Self::SetFont { font: l_font, size: l_size }, Self::SetFont { font: r_font, size: r_size }) => l_font == r_font && l_size == r_size,
            (Self::SetTextCursor { pos: l_pos }, Self::SetTextCursor { pos: r_pos }) => l_pos == r_pos,
            (Self::SetFillColor { col: l_col }, Self::SetFillColor { col: r_col }) => l_col == r_col,
            (Self::SetOutlineColor { col: l_col }, Self::SetOutlineColor { col: r_col }) => l_col == r_col,
            (Self::SetOutlineThickness { pt: l_pt }, Self::SetOutlineThickness { pt: r_pt }) => l_pt == r_pt,
            (Self::SetLineDashPattern { dash: l_pattern }, Self::SetLineDashPattern { dash: r_pattern }) => l_pattern == r_pattern,
            (Self::SetLineJoinStyle { join: l_join }, Self::SetLineJoinStyle { join: r_join }) => l_join == r_join,
            (Self::SetLineCapStyle { cap: l_cap }, Self::SetLineCapStyle { cap: r_cap }) => l_cap == r_cap,
            (Self::SetTextRenderingMode { mode: l_mode }, Self::SetTextRenderingMode { mode: r_mode }) => l_mode == r_mode,
            (Self::SetCharacterSpacing { multiplier: l_multiplier }, Self::SetCharacterSpacing { multiplier: r_multiplier }) => l_multiplier == r_multiplier,
            (Self::SetLineOffset { multiplier: l_multiplier }, Self::SetLineOffset { multiplier: r_multiplier }) => l_multiplier == r_multiplier,
            (Self::DrawLine { line: l_line }, Self::DrawLine { line: r_line }) => l_line == r_line,
            (Self::DrawPolygon { polygon: l_polygon }, Self::DrawPolygon { polygon: r_polygon }) => l_polygon == r_polygon,
            (Self::SetTransformationMatrix { matrix: l_matrix }, Self::SetTransformationMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::SetTextMatrix { matrix: l_matrix }, Self::SetTextMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::LinkAnnotation { link_ref: l_link_ref }, Self::LinkAnnotation { link_ref: r_link_ref }) => l_link_ref == r_link_ref,
            (Self::UseXObject { id: l_xobj_id, transform: l_transform }, Self::UseXObject { id: r_xobj_id, transform: r_transform }) => l_xobj_id == r_xobj_id && l_transform == r_transform,
            (Self::Unknown { key: l_key, value: _ }, Self::Unknown { key: r_key, value: r_value }) => l_key == r_key,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}
