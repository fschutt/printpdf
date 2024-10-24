use crate::{color::Color, graphics::{Line, Point, Polygon, Rect}, matrix::{CurTransMat, TextMatrix}, units::{Mm, Pt}, ExtendedGraphicsStateId, FontId, LayerInternalId, LinkAnnotId, PageAnnotId, XObjectId};

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
    UseGraphicsState { id: ExtendedGraphicsStateId },

    BeginTextSection,
    EndTextSection,

    WriteText { text: String, font: FontId },
    SetFont { font: FontId },
    SetTextCursor { pos: Point },
    SetFillColor { col: Color },
    SetOutlineColor { col: Color },

    DrawLine { line: Line },
    DrawPolygon { polygon: Polygon },
    SetTransformationMatrix { matrix: CurTransMat },
    SetTextMatrix { matrix: TextMatrix },

    LinkAnnotation { link_ref: LinkAnnotId },
    InstantiateXObject { xobj_id: XObjectId, transformations: Vec<CurTransMat> },
    Unknown { key: String, value: lopdf::content::Operation },
}

impl PartialEq for Op {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BeginLayer { layer_id: l_layer_id }, Self::BeginLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::EndLayer { layer_id: l_layer_id }, Self::EndLayer { layer_id: r_layer_id }) => l_layer_id == r_layer_id,
            (Self::UseGraphicsState { id: l_id }, Self::UseGraphicsState { id: r_id }) => l_id == r_id,
            (Self::WriteText { text: l_text, font: l_font }, Self::WriteText { text: r_text, font: r_font }) => l_text == r_text && l_font == r_font,
            (Self::SetFont { font: l_font }, Self::SetFont { font: r_font }) => l_font == r_font,
            (Self::SetTextCursor { pos: l_pos }, Self::SetTextCursor { pos: r_pos }) => l_pos == r_pos,
            (Self::SetFillColor { col: l_col }, Self::SetFillColor { col: r_col }) => l_col == r_col,
            (Self::SetOutlineColor { col: l_col }, Self::SetOutlineColor { col: r_col }) => l_col == r_col,
            (Self::DrawLine { line: l_line }, Self::DrawLine { line: r_line }) => l_line == r_line,
            (Self::DrawPolygon { polygon: l_polygon }, Self::DrawPolygon { polygon: r_polygon }) => l_polygon == r_polygon,
            (Self::SetTransformationMatrix { matrix: l_matrix }, Self::SetTransformationMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::SetTextMatrix { matrix: l_matrix }, Self::SetTextMatrix { matrix: r_matrix }) => l_matrix == r_matrix,
            (Self::LinkAnnotation { link_ref: l_link_ref }, Self::LinkAnnotation { link_ref: r_link_ref }) => l_link_ref == r_link_ref,
            (Self::InstantiateXObject { xobj_id: l_xobj_id, transformations: l_transformations }, Self::InstantiateXObject { xobj_id: r_xobj_id, transformations: r_transformations }) => l_xobj_id == r_xobj_id && l_transformations == r_transformations,
            (Self::Unknown { key: l_key, value: l_value }, Self::Unknown { key: r_key, value: r_value }) => l_key == r_key && l_value.operator == r_value.operator && l_value.operands == l_value.operands,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}